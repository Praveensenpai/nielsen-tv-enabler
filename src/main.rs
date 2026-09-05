//! Main entrypoint for nielsen-tv-enabler.

use clap::Parser;
use nielsen_tv_enabler::app;
use nielsen_tv_enabler::cli::Args;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_logger(args.verbose);
    app::run(&args)
}

/// Initializes environment logger with appropriate filter level.
fn init_logger(verbose: bool) {
    let default_level = if verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
        .format_timestamp_secs()
        .init();
}
