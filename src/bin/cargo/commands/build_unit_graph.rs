use cargo::ops;

use crate::command_prelude::*;

pub fn cli() -> Command {
    subcommand("build-unit-graph")
        .about("Build a unit graph")
        .arg(
            Arg::new("path")
                .value_name("PATH")
                .action(ArgAction::Set)
                .required(true)
                .help("Path to a serialized unit graph"),
        )
        .arg_future_incompat_report()
        .arg_message_format()
        .arg_silent_suggestion()
        .arg_parallel()
        .arg_target_dir()
        .arg_timings()
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let compile_opts =
        args.compile_options(gctx, CompileMode::Build, None, ProfileChecking::Custom)?;

    let path = args.value_of_path("path", gctx).unwrap();
    ops::compile_unit_graph(gctx, compile_opts, &path)?;
    Ok(())
}
