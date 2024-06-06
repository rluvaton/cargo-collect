mod cli;
mod collect_packages;
mod download_packages;


use anyhow::{anyhow, Result};
use crates_index::Index;

use crate::cli::Cli;
use crate::collect_packages::collect_packages;
use crate::download_packages::download_packages;

async fn run(args: Cli) -> Result<()> {
    let index = Index::new_cargo_default()?;

    // Take the version requirement from args if exists,
    // otherwise define the highest normal version as the version req.
    let version_req = if let Some(version_req) = args.crate_version_req {
        version_req
    } else {
        let krate = index
            .crate_(&args.crate_name)
            .ok_or_else(|| anyhow!(format!("Crate {} not found", args.crate_name)))?;
        krate
            .highest_normal_version()
            .unwrap_or(krate.highest_version())
            .version()
            .to_owned()
    };

    // Collect the dependencies recursively.
    let packages = collect_packages(
        &index,
        args.crate_name.to_owned(),
        version_req,
        &args.output,
    )
        .await?;

    // Download all crates in parallel.
    download_packages(packages).await?;

    Ok(())
}



#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = cli::get_options();

    run(args).await?;

    Ok(())
}
