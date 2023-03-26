use anyhow::{anyhow, Context, Result};
use clap::Parser;
use crates_index::Index;
use futures::StreamExt;
use itertools::Itertools;
use reqwest::header::{HeaderValue, USER_AGENT};
use reqwest::Client;
use semver::{Version as SemVersion, VersionReq};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
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
    #[arg(short = 'o', long)]
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
) -> Result<()> {
    info!("Downloading {}", url);
    let mut http_res = client
        .get(url)
        .header(USER_AGENT, user_agent)
        .send()
        .await?;
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
    packages: &mut HashSet<Package>,
    folder_path: &Path,
    crate_name: &str,
    crate_version_req: &str,
) -> Result<Vec<(String, String)>> {
    let krate = index
        .crate_(crate_name)
        .ok_or_else(|| anyhow!("Crate not found"))?;
    let version_req = VersionReq::parse(crate_version_req)?;
    let version = krate
        .versions()
        .into_iter()
        .filter_map(|version| {
            if version.is_yanked() {
                None
            } else {
                Some((
                    version.clone(),
                    SemVersion::parse(version.version()).unwrap_or_else(|e| {
                        warn!(
                            "Skipped, Can't parse the crate version: {}-{}, {e}",
                            crate_name,
                            version.version()
                        );
                        SemVersion::new(0, 0, 0)
                    }),
                ))
            }
        })
        .rev()
        .sorted_unstable_by_key(|(_, semversion)| semversion.clone())
        .find(|(_, semversion)| version_req.matches(semversion));

    if let Some((version, _)) = version {
        let url = version
            .download_url(&index.index_config()?)
            .ok_or_else(|| anyhow!("Can't generate download url for crate: {}", crate_name))?;
        let pkg = Package::new(
            folder_path.join(format!("{}-{}.crate", crate_name, version.version())),
            url,
            version.checksum().to_vec(),
        );

        // If the package already processed skip thier dependencies.
        if packages.insert(pkg) {
            Ok(version
                .dependencies()
                .into_iter()
                .map(|dep| (dep.crate_name().to_owned(), dep.requirement().to_owned()))
                .collect_vec())
        } else {
            Ok(vec![])
        }
    } else {
        Err(anyhow!(
            "Relevant version for crate {} not found",
            crate_name
        ))
    }
}

async fn download_packages(
    client: &Client,
    user_agent: &HeaderValue,
    packages: HashSet<Package>,
) -> Result<()> {
    let tasks = futures::stream::iter(packages.into_iter())
        .map(|pkg| {
            let client = client.clone();
            let user_agent = user_agent.clone();
            tokio::spawn(async move {
                download_crate(&client, &pkg.url, &pkg.path, &pkg.checksum, &user_agent).await
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

async fn collect_packages(index: &Index, crate_name: &str, crate_version_req: &str) -> Result<HashSet<Package>> {
    // Collect all dependencies recursively.
    let mut worklist = vec![(args.crate_name, version_req)];
    let mut packages = HashSet::new();
    info!("Collect dependencies recursively.");
    while let Some((crate_name, crate_version_req)) = worklist.pop() {
        let deps = find_highest_requirement_version(
            &index,
            &mut packages,
            &args.output,
            &crate_name,
            &crate_version_req,
        )
        .await?;
        worklist.extend(deps);
    }
    Ok(packages)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    env_logger::init();
    let subscriber = FmtSubscriber::builder().with_timer(SystemTime).finish();
    set_global_subscriber(subscriber).context("failed to set tracing subscriber")?;

    let args = Cli::parse();
    let index = Index::new_cargo_default()?;

    let version_req = if let Some(version_req) = args.crate_version_req {
        version_req
    } else {
        let krate = index.crate_(&args.crate_name).ok_or_else(|| anyhow!("Crate not found"))?;
        krate.highest_normal_version().unwrap_or(krate.highest_version()).version().to_owned()
    };

    let packages = collect_packages(&index, &args.crate_name, &version_req).await?;

    // Download all crates in parallel.
    let client = Client::new();
    let user_agent = HeaderValue::from_str(&format!("CargoCollect/{}", env!("CARGO_PKG_VERSION")))?;
    download_packages(&client, &user_agent, packages).await?;
    Ok(())
}
