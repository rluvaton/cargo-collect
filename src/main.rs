#[macro_use]
extern crate derive_builder;

mod cli;
mod collect_packages;
mod download_packages;
mod spinners;
mod parse_cargo_files;

use std::fs;
use anyhow::{anyhow, Result};
use crates_index::GitIndex;

use crate::cli::Cli;
use crate::collect_packages::collect_packages;
use crate::download_packages::download_packages;
use crate::parse_cargo_files::cargo_toml_file::{parse_cargo_file_from_path};
use crate::parse_cargo_files::parse_lock_file::parse_cargo_lock_file;

pub type CratesToDownload = Vec<(
    String, /* Crate name */
    String /* Crate version requirement */
)>;

async fn run(args: Cli) -> Result<()> {
    let mut index = GitIndex::new_cargo_default()?;

    if args.update_index {
        println!("Updating index...");
        index.update().await?;
        println!("Index updated.");
    }

    let mut crates_to_download: CratesToDownload;

    let output_path = args.output.clone();

    if args.crate_name.is_some() {
        crates_to_download = get_crate_names_and_versions_from_cli_arg(&index, args)?;
    } else if args.cargo_file.is_some() {
        crates_to_download = get_crate_names_and_versions_from_cargo_file(args);
    } else if args.cargo_lock_file.is_some() {
        crates_to_download = get_crate_names_and_versions_from_cargo_lock_file(args);
    } else {
        unreachable!("Should not reach here");
    }

    if !output_path.try_exists().expect("Failed to check directory creation") {
        fs::create_dir(&output_path)
            .expect(format!("Failed to create output directory at {:?}", output_path.as_path()).as_str());
    }

    // Collect the dependencies recursively.
    let packages = collect_packages(
        &index,
        &mut crates_to_download,
        &output_path,
    )
        .await?;

    // Download all crates in parallel.
    download_packages(packages).await?;

    Ok(())
}

fn get_crate_names_and_versions_from_cli_arg(index: &GitIndex, args: Cli) -> Result<CratesToDownload> {
    let crate_name = args.crate_name.expect("Must have crate name");

    // Take the version requirement from args if exists,
    // otherwise define the highest normal version as the version req.
    let version_req = if let Some(version_req) = args.crate_version_req {
        version_req
    } else {
        get_version_requirements_for_crate(index, crate_name.clone())?
    };

    return Ok(vec![(crate_name.clone(), version_req)]);
}

fn get_version_requirements_for_crate(index: &GitIndex, crate_name: String) -> Result<String> {

    // Take the version requirement from args if exists,
    // otherwise define the highest normal version as the version req.

    let krate = index
        .crate_(&crate_name)
        .ok_or_else(|| anyhow!(format!("Crate {} not found", crate_name)))?;

    return Ok(krate
        .highest_normal_version()
        .unwrap_or(krate.highest_version())
        .version()
        .to_owned());
}


fn get_crate_names_and_versions_from_cargo_file(args: Cli) -> CratesToDownload {
    let cargo_file_path = args.cargo_file.expect("Must exists");

    let deps = parse_cargo_file_from_path(cargo_file_path);

    return deps.iter()
        .map(|(key, _)| (key.name.clone(), key.version.clone()))
        .collect();
}

fn get_crate_names_and_versions_from_cargo_lock_file(args: Cli) -> CratesToDownload {
    let cargo_lock_file_path = args.cargo_lock_file.expect("Must exists");

    let cargo_file_content = fs::read_to_string(cargo_lock_file_path.clone()).expect(format!("Failed to read Cargo.lock file at {}", cargo_lock_file_path).as_str());

    let deps = parse_cargo_lock_file(cargo_file_content);

    if deps.package.is_none() {
        return vec![];
    }

    return deps
        .package
        .unwrap()
        .iter()

        // Only take the packages that are not local packages (local packages does not have source
        .filter(|package| package.source.is_some())
        // In lock file we want exact version
        .map(|package| (package.name.clone(), "=".to_owned() + package.version.clone().as_str()))
        .collect();
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = cli::get_options();

    run(args).await?;

    Ok(())
}

