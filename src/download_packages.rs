use std::collections::HashSet;
use std::path::{Path, PathBuf};
use anyhow::{anyhow,  Result};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressFinish, ProgressStyle};
use reqwest::Client;
use reqwest::header::{HeaderValue, USER_AGENT};
use sha2::{Digest, Sha256};
use tokio::fs::{create_dir_all};
use std::fs::OpenOptions;
use std::io::Write;
use tracing::{info, warn};
use crate::collect_packages::{Package};
use crate::spinners::progress_bar;

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

pub async fn download_packages(packages: HashSet<Package>) -> Result<()> {
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


