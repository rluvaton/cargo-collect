use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use itertools::Itertools;
use tracing::subscriber::set_global_default as set_global_subscriber;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::FmtSubscriber;

const EXAMPLES: &str = r#"
Examples:
    # Collect the dependencies of the crate `serde` with version requirement `^1.0.0`
    # Save the crates files in "./deps" folder
    ./cargo-collect --crate-name serde --crate-version-req "^1.0.0"

    # Collect the dependencies of the crate `serde` with version requirement `=1.0.0`
    # and put them in the folder `output`
    ./cargo-collect --crate-name serde --crate-version-req "=1.0.0" --output output

    # Collect all dependencies used by the local Cargo.toml file that match the versions specified.
    # This support workspaces as well
    # Useful for example when some python library (e.g. cryptography) have Rust implementation
    # that is not published to crates.io and it's required in order to install the library
    ./cargo-collect --cargo-file Cargo.toml

    # Collect all dependencies used by the local Cargo.lock file that match the EXACT
    # versions specified.
    # Useful for example when some python library (e.g. cryptography) have Rust implementation
    # that is not published to crates.io and it's required in order to install the library
    ./cargo-collect --cargo-lock-file Cargo.lock
"#;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, after_help = EXAMPLES)]
pub struct Cli {
    /// The crate name.
    #[arg(
        short = 'n',
        long,
        required_unless_present_any(["crate_name", "cargo_file"])
    )]
    pub(crate) crate_name: Option<String>,

    /// The version requirement of the crate can be =1.0.0 or ^1.0 (see semver.org).
    #[arg(
        short = 'v',
        long,
        conflicts_with_all(["cargo_lock_file", "cargo_file"])
    )]
    pub(crate) crate_version_req: Option<String>,

    /// The output folder to put all crate files.
    #[arg(
        short = 'o',
        long,
        default_value = "deps"
    )]
    pub(crate) output: PathBuf,

    /// The Cargo.toml file to take dependencies from.
    /// This will take the latest version that the version requirement
    /// (This should be used when the crate is not published)
    ///
    /// Support workspaces
    #[arg(
        long,
        required_unless_present_any(["crate_name", "cargo_lock_file"])
    )]
    pub(crate) cargo_file: Option<String>,

    /// The Cargo.lock file to take dependencies from.
    /// This will take exact versions of the dependencies.
    /// (This should be used when the crate is not published)
    #[arg(
        long,
        required_unless_present_any(["crate_name", "cargo_file"])
    )]
    pub(crate) cargo_lock_file: Option<String>,

    /// Whether to update the local index of crates.io.
    ///
    /// Use this when cant find crate version that you know exists
    #[arg(
        short = 'u',
        default_value = "false"
    )]
    pub(crate) update_index: bool,
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

    return args;
}
