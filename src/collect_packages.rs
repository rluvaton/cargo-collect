
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use crates_index::{Index, IndexConfig};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use itertools::Itertools;
use tracing::{info, warn};
use semver::{Version as SemVersion, VersionReq};
use crate::spinners::progress_spinner;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Package {
    pub(crate) path: PathBuf,
    pub(crate)url: String,
    pub(crate)checksum: Vec<u8>,
}

impl Package {
    pub fn new(path: PathBuf, url: String, checksum: Vec<u8>) -> Self {
        Self {
            path,
            url,
            checksum,
        }
    }
}

async fn find_highest_requirement_version(
    index: &Index,
    index_config: &IndexConfig,
    packages: &mut HashSet<Package>,
    folder_path: &Path,
    crate_name: &str,
    crate_version_req: &str,
    pb: &ProgressBar,
) -> Result<(Option<String>, Vec<(String, String)>)> {
    pb.set_message(crate_name.to_owned());
    let krate = index
        .crate_(crate_name);

    if krate.is_none() {
        warn!("Crate {} not found, skipping", crate_name);
        return Ok((None, vec![]));
    }

    let krate = krate.unwrap();

    let version_req = VersionReq::parse(crate_version_req)?;
    let versions = krate
        .versions()
        .iter()
        .filter_map(|version| {
            let semversion = SemVersion::parse(version.version()).unwrap_or_else(|e| {
                warn!(
                    "Skipped, Can't parse the crate version: {}-{}, {e}",
                    crate_name,
                    version.version()
                );
                SemVersion::new(0, 0, 0)
            });
            if version_req.matches(&semversion) {
                Some((version, semversion))
            } else {
                None
            }
        })
        .sorted_unstable_by_key(|(_, semversion)| semversion.clone())
        .rev()
        .collect_vec();

    // Take the highest matched version that not yanked if it's exists. otherwise take the highest yanked version.
    let version = versions
        .iter()
        .find(|(v, _)| !v.is_yanked())
        .or(versions.get(0));

    if let Some((version, _)) = version {
        let url = version
            .download_url(index_config)
            .ok_or_else(|| anyhow!("Can't generate download url for crate: {}", crate_name))?;
        let pkg = Package::new(
            folder_path.join(format!("{}-{}.crate", crate_name, version.version())),
            url,
            version.checksum().to_vec(),
        );

        // If the package already processed skip their dependencies.
        if packages.insert(pkg) {
            pb.inc(1);
            Ok((Some(version.version().to_string()), version
                .dependencies()
                .into_iter()
                .map(|dep| (dep.crate_name().to_owned(), dep.requirement().to_owned()))
                .collect_vec()))
        } else {
            Ok((None, vec![]))
        }
    } else {
        Err(anyhow!(
            "Relevant version for crate {} was not found. version_req: {}, versions: {:?}",
            crate_name,
            version_req,
            krate
                .versions()
                .iter()
                .filter(|v| !v.is_yanked())
                .map(|v| {
                    let semv = SemVersion::parse(v.version()).unwrap();
                    format!("{}: {}", v.version(), version_req.matches(&semv))
                })
                .collect_vec()
        ))
    }
}

pub async fn collect_packages(
    index: &Index,
    crate_name: String,
    crate_version_req: String,
    output: &Path,
) -> Result<HashSet<Package>> {
    // Collect all dependencies recursively.
    let mut worklist = vec![(crate_name, crate_version_req)];
    let mut packages = HashSet::new();
    let index_config = index.index_config()?;
    let pb = progress_spinner()?;
    info!("Collect dependencies recursively...");

    let mut already_downloaded = build_hashset_from_local_deps(output.to_str().unwrap().to_string());
    while let Some((crate_name, crate_version_req)) = worklist.pop() {
        if already_downloaded.contains_key(&crate_name) {
            let versions = already_downloaded.get(&crate_name).unwrap();
            let matched = versions.iter().find(|v| is_version_match_the_range(v.as_str().to_string(), crate_version_req.clone()));
            if matched.is_some() {
                continue;
            }
        }

        let (version, deps) = find_highest_requirement_version(
            &index,
            &index_config,
            &mut packages,
            output,
            &crate_name,
            &crate_version_req,
            &pb,
        )
            .await?;

        worklist.extend(deps);

        if version.is_none() {
            continue;
        }

        let version = version.unwrap();

        if already_downloaded.contains_key(&crate_name) {
            let versions = already_downloaded.get_mut(&crate_name).unwrap();
            if !versions.contains(&version) {
                versions.insert(version);
            }
        } else {
            let mut versions = HashSet::new();
            versions.insert(version);
            already_downloaded.insert(crate_name, versions);
        }


    }
    Ok(packages)
}

fn is_version_match_the_range(version: String, range: String) -> bool {
    let version_req = VersionReq::parse(range.as_str());

    if version_req.is_err() {
        return false;
    }

    let version_req = version_req.unwrap();

    let semversion = SemVersion::parse(version.as_str()).unwrap_or_else(|e| {
        warn!(
                    "Skipped, Can't parse the crate version: {}, {e}",
                    version
                );
        SemVersion::new(0, 0, 0)
    });

    return version_req.matches(&semversion)
}

fn create_file_name_from_crate_name_and_version(crate_name: String, version: String) -> String {
    return format!("{}-{}.crate", crate_name, version);
}

fn parse_crate_name_and_version_from_file_name(file_name: &str) -> Option<(String, String)> {
    let crate_and_version = file_name.replace(".crate", "");

    let position = crate_and_version.rfind("-");

    if position.is_none() {
        return None;
    }

    let position = position.unwrap();

    let crate_name = &crate_and_version[0..position];
    let version = &crate_and_version[position + 1..crate_and_version.len()];

    return Some((crate_name.to_string(), version.to_string()));
}

fn build_hashset_from_local_deps(folder_with_already_download: String) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();

    std::fs::read_dir(folder_with_already_download)
        .unwrap()
        .filter_map(|entry| entry.ok())

        .filter_map(|entry| {
            let path = entry.path();
            let file_name = path.file_name();

            if file_name.is_none() {
                return None;
            }

            let file_name = file_name.unwrap().to_str();

            if file_name.is_none() {
                return None;
            }

            let file_name = file_name.unwrap().to_string();

            if !file_name.ends_with(".crate") {
                return None;
            }

            return parse_crate_name_and_version_from_file_name(file_name.as_str())

        })

        .for_each(|(crate_name, version)| {
            if map.contains_key(&crate_name) {
                let versions: &mut HashSet<String> = map.get_mut(&crate_name).unwrap();
                if !versions.contains(&version) {
                    (versions).insert(version.to_string());
                }
            } else {
                let mut versions = HashSet::new();
                versions.insert(version.to_string());
                map.insert(crate_name.to_string(), versions);
            }
        });

    return map;
}
