use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use itertools::Itertools;
use tracing::subscriber::set_global_default as set_global_subscriber;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The crate name.
    #[arg(short = 'n', long)]
    pub(crate) crate_name: String,
    /// The version requirement of the crate can be =1.0.0 or ^1.0 (see semver.org).
    #[arg(short = 'v', long)]
    pub(crate) crate_version_req: Option<String>,
    /// The output folder to put all crate files.
    #[arg(short = 'o', long, default_value = "deps")]
    pub(crate) output: PathBuf,
}

pub fn get_options() -> Cli {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    let subscriber = FmtSubscriber::builder().with_timer(SystemTime).finish();
    set_global_subscriber(subscriber).context("failed to set tracing subscriber").expect("failed to set tracing subscriber");

    // Skip collect subcommand keyword for using with cargo.
    let args = std::env::args().collect_vec();
    let args = if args
        .get(1)
        .and_then(|a| Some(a == "collect"))
        .unwrap_or(false)
    {
        Cli::parse_from(&args[1..])
    } else {
        Cli::parse()
    };

    return args
}
