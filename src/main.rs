use anyhow::{anyhow, Context, Result};
use clap::Parser;
use crates_index::{Index, IndexConfig};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressFinish, ProgressStyle};
use itertools::Itertools;
use reqwest::header::{HeaderValue, USER_AGENT};
use reqwest::Client;
use semver::{Version as SemVersion, VersionReq};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs::create_dir_all;
use tracing::subscriber::set_global_default as set_global_subscriber;
use tracing::{info, warn};
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The crate name.
    #[arg(short = 'n', long)]
    crate_name: String,
    /// The version requirement of the crate can be =1.0.0 or ^1.0 (see semver.org).
    #[arg(short = 'v', long)]
    crate_version_req: Option<String>,
    /// The output folder to put all crate files.
    #[arg(short = 'o', long, default_value = "deps")]
    output: PathBuf,
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct Package {
    path: PathBuf,
    url: String,
    checksum: Vec<u8>,
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

fn append_to_path(path: &Path, suffix: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension(suffix);
    path
}

pub fn move_if_exists(from: &Path, to: &Path) -> Result<()> {
    if from.exists() {
        std::fs::rename(from, to)?;
    }
    Ok(())
}

async fn download_crate(
    client: &Client,
    url: &str,
    path: &Path,
    hash: &[u8],
    user_agent: &HeaderValue,
    pb: &ProgressBar,
) -> Result<()> {
    pb.set_message(format!(
        "Downloading {}",
        path.file_name().unwrap().to_str().unwrap()
    ));
    let mut http_res = client
        .get(url)
        .header(USER_AGENT, user_agent)
        .send()
        .await?;
    create_dir_all(path.parent().unwrap()).await?;
    let part_path = append_to_path(path, ".part");

    let mut hasher = Sha256::new();
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&part_path)?;
        let status = http_res.status();
        if status == 403 || status == 404 {
            let forbidden_path = append_to_path(path, ".notfound");
            let text = http_res.text().await?;
            std::fs::write(
                forbidden_path,
                format!("Server returned {}: {}", status, &text),
            )?;
            return Err(anyhow!(
                "Crate not found: {}, {}, {}",
                status.as_u16(),
                url.to_string(),
                text
            ));
        }

        while let Some(chunk) = http_res.chunk().await? {
            hasher.update(&chunk);
            f.write_all(&chunk)?;
        }
    }

    let f_hash = hasher.finalize();

    if f_hash.as_slice() == hash {
        move_if_exists(&part_path, path)?;
        Ok(())
    } else {
        let badsha_path = append_to_path(path, ".badsha256");
        std::fs::write(badsha_path, &f_hash)?;
        Err(anyhow!(
            "Mismatched Hash: expected: {:x?} actual: {:x}",
            hash,
            f_hash
        ))
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

        // If the package already processed skip thier dependencies.
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

fn progress_spinner() -> Result<ProgressBar> {
    Ok(
        ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr()).with_style(
            ProgressStyle::with_template("{spinner:.green} {pos} - {msg}")?,
        ),
    )
}

fn progress_bar(size: usize) -> ProgressBar {
    ProgressBar::new(size as u64)
        .with_style(
            ProgressStyle::with_template(
                "{spinner:.green} {msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
            )
                .expect("template is correct")
                .progress_chars("#>-"),
        )
        .with_finish(ProgressFinish::AndLeave)
}

async fn download_packages(packages: HashSet<Package>) -> Result<()> {
    info!("Downloading {} crates", packages.len());
    let client = Client::new();
    let user_agent = HeaderValue::from_str(&format!("CargoCollect/{}", env!("CARGO_PKG_VERSION")))?;
    let pb = progress_bar(packages.len());

    let tasks = futures::stream::iter(packages.into_iter())
        .map(|pkg| {
            let pb = pb.clone();
            let client = client.clone();
            let user_agent = user_agent.clone();
            tokio::spawn(async move {
                download_crate(
                    &client,
                    &pkg.url,
                    &pkg.path,
                    &pkg.checksum,
                    &user_agent,
                    &pb,
                )
                    .await?;
                pb.inc(1);
                Ok::<_, anyhow::Error>(())
            })
        })
        .buffer_unordered(16)
        .collect::<Vec<_>>()
        .await;

    for t in tasks {
        match t.unwrap() {
            Ok(_) => {}
            Err(err) => {
                warn!("Can't download crate: {}", err)
            }
        }
    }
    Ok(())
}

async fn collect_packages(
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
        if (already_downloaded.contains_key(&crate_name)) {
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
            let mut versions = already_downloaded.get_mut(&crate_name).unwrap();
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

fn parse_crate_name_and_version_from_file_name(file_name: &str) -> Option<(&str, &str)> {
    let crate_and_version = file_name.replace(".crate", "");

    let position = crate_and_version.rfind("-");

    if position.is_none() {
        return None;
    }

    let position = position.unwrap();

    let crate_name = &crate_and_version[0..position];
    let version = &crate_and_version[position + 1..crate_and_version.len()];

    return Some((crate_name, version));
}

fn build_hashset_from_local_deps(folder_with_already_download: String) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();

    std::fs::read_dir(folder_with_already_download)
        .unwrap()
        .filter_map(|entry| entry.ok())

        .map(|entry| entry.path().file_name())
        .filter_map(|file_name| file_name)

        .map(|file_name| file_name.to_str())
        .filter_map(|file_name| file_name)

        .filter(|file_name| file_name.ends_with(".crate"))
        .filter_map(|file_name| parse_crate_name_and_version_from_file_name(file_name))

        .for_each(|(crate_name, version)| {
            if map.contains_key(crate_name) {
                let mut versions: &mut HashSet<String> = map.get_mut(&crate_name).unwrap();
                if !versions.contains(version) {
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
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    let subscriber = FmtSubscriber::builder().with_timer(SystemTime).finish();
    set_global_subscriber(subscriber).context("failed to set tracing subscriber")?;

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

    run(args).await?;

    Ok(())
}
