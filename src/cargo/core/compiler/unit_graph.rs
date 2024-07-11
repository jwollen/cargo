//! Serialization of [`UnitGraph`] for unstable option [`--unit-graph`].
//!
//! [`--unit-graph`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#unit-graph

use crate::core::compiler::Unit;
use crate::core::compiler::{BuildContext, CompileKind, CompileMode, CompileTarget};
use crate::core::profiles::{Profile, UnitFor};
use crate::core::{PackageId, Target};
use crate::util::interning::InternedString;
use crate::util::CargoResult;
use std::collections::HashMap;
use std::hash::Hash;
use std::io::Write;
use std::path::Path;

/// The dependency graph of Units.
pub type UnitGraph = HashMap<Unit, Vec<UnitDep>>;

/// A unit dependency.
#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct UnitDep {
    /// The dependency unit.
    pub unit: Unit,
    /// The purpose of this dependency (a dependency for a test, or a build
    /// script, etc.). Do not use this after the unit graph has been built.
    pub unit_for: UnitFor,
    /// The name the parent uses to refer to this dependency.
    pub extern_crate_name: InternedString,
    /// If `Some`, the name of the dependency if renamed in toml.
    /// It's particularly interesting to artifact dependencies which rely on it
    /// for naming their environment variables. Note that the `extern_crate_name`
    /// cannot be used for this as it also may be the build target itself,
    /// which isn't always the renamed dependency name.
    pub dep_name: Option<InternedString>,
    /// Whether or not this is a public dependency.
    pub public: bool,
    /// If `true`, the dependency should not be added to Rust's prelude.
    pub noprelude: bool,
}

const VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SerializedUnitGraph {
    pub version: u32,
    pub units: Vec<SerializedUnit>,
    pub roots: Vec<usize>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializedUnit {
    pub pkg_id: PackageId,
    pub target: Target,
    pub profile: Profile,
    pub platform: CompileKind,
    pub mode: CompileMode,
    pub features: Vec<InternedString>,
    pub rustflags: Vec<String>,
    pub rustdocflags: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)] // hide for unstable build-std
    pub is_std: bool,
    pub dep_hash: u64,
    pub artifact: bool,
    pub artifact_target_for_features: Option<CompileTarget>,
    pub extra_compiler_args: Vec<String>,
    pub dependencies: Vec<SerializedUnitDep>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializedUnitDep {
    pub index: usize,
    pub extern_crate_name: InternedString,
    // This is only set on nightly since it is unstable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    // This is only set on nightly since it is unstable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noprelude: Option<bool>,
    pub dep_name: Option<InternedString>,
    pub unit_for: UnitFor,
}

pub fn load_serialized_unit_graph(path: &Path) -> CargoResult<SerializedUnitGraph> {
    let data = cargo_util::paths::read(path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Outputs a JSON serialization of [`UnitGraph`] for given `root_units`
/// to the standard output.
pub fn emit_serialized_unit_graph(
    root_units: &[Unit],
    unit_graph: &UnitGraph,
    bcx: &BuildContext<'_, '_>,
) -> CargoResult<()> {
    let mut units: Vec<(&Unit, &Vec<UnitDep>)> = unit_graph.iter().collect();
    units.sort_unstable();
    // Create a map for quick lookup for dependencies.
    let indices: HashMap<&Unit, usize> = units
        .iter()
        .enumerate()
        .map(|(i, val)| (val.0, i))
        .collect();
    let roots = root_units.iter().map(|root| indices[root]).collect();
    let ser_units = units
        .iter()
        .map(|(unit, unit_deps)| {
            let dependencies: Vec<SerializedUnitDep> = unit_deps
                .iter()
                .map(|unit_dep| {
                    // https://github.com/rust-lang/rust/issues/64260 when stabilized.
                    let (public, noprelude) = if bcx.gctx.nightly_features_allowed {
                        (Some(unit_dep.public), Some(unit_dep.noprelude))
                    } else {
                        (None, None)
                    };
                    SerializedUnitDep {
                        index: indices[&unit_dep.unit],
                        extern_crate_name: unit_dep.extern_crate_name,
                        public,
                        noprelude,
                        dep_name: unit_dep.dep_name,
                        unit_for: unit_dep.unit_for,
                    }
                })
                .collect();

            let extra_compiler_args = bcx
                .extra_compiler_args
                .get(unit)
                .cloned()
                .unwrap_or_default();

            SerializedUnit {
                pkg_id: unit.pkg.package_id(),
                target: unit.target.clone(),
                profile: unit.profile.clone(),
                platform: unit.kind,
                mode: unit.mode,
                features: unit.features.clone(),
                rustflags: unit.rustflags.to_vec(),
                rustdocflags: unit.rustdocflags.to_vec(),
                is_std: unit.is_std,
                dep_hash: unit.dep_hash,
                artifact: unit.artifact.is_true(),
                artifact_target_for_features: unit.artifact_target_for_features,
                extra_compiler_args,
                dependencies,
            }
        })
        .collect();
    let s = SerializedUnitGraph {
        version: VERSION,
        units: ser_units,
        roots,
    };

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, &s)?;
    drop(writeln!(lock));
    Ok(())
}
