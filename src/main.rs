#[macro_use]
extern crate derive_builder;

mod cli;
mod collect_packages;
mod download_packages;
mod spinners;
mod parse_lock_file;
mod parse_cargo_files;

use std::fs;
use anyhow::{anyhow, Result};
use crates_index::Index;

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
    let index = Index::new_cargo_default()?;

    let mut crates_to_download: CratesToDownload;

    let output_path = args.output.clone();

    if args.crate_name.is_some() {
        crates_to_download = get_crate_names_and_versions_from_cli_arg(&index, args)?;
    } else if args.cargo_file.is_some() {
        crates_to_download = get_crate_names_and_versions_from_cargo_file(args);
    } else if args.cargo_lock_file.is_some() {
        crates_to_download = get_crate_names_and_versions_from_cargo_lock_file(args);
    } else {
        panic!("Should not reach here");
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

fn get_crate_names_and_versions_from_cli_arg(index: &Index, args: Cli) -> Result<CratesToDownload> {
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

fn get_version_requirements_for_crate(index: &Index, crate_name: String) -> Result<String> {

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

    return deps
        .package
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

#[cfg(test)]
mod tests_mod {
    use crates_index::Crate;

    #[test]
    fn it_works() {
        let base64 = r#"
        {"name":"base64","vers":"0.1.0","deps":[],"cksum":"0fcf1cc12085a0cb60a051f2ba4a027b3e84d4265d8831794147f58c584e3301","features":{},"yanked":false}
{"name":"base64","vers":"0.1.1","deps":[],"cksum":"a51012ca17f843e723dedc71fdd7feac9d8b53be85492aa9232b2da59ce6bb3b","features":{},"yanked":false}
{"name":"base64","vers":"0.2.0","deps":[],"cksum":"3ce110e5c96df1817009271c910626fa4b79c2f178d70f9857d768c3886ba6a0","features":{},"yanked":false}
{"name":"base64","vers":"0.2.1","deps":[],"cksum":"2015e3793554aa5b6007e3a72959e84c1070039e74f13dde08fa64afe1ddd892","features":{},"yanked":false}
{"name":"base64","vers":"0.3.0","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"1d156a04ec694d726e92ea3c13e4a62949b4f0488a9344f04341d679ec6b127b","features":{},"yanked":false}
{"name":"base64","vers":"0.4.0","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"065a0ce220ab84d0b6d5ae3e7bb77232209519c366f51f946fe28c19e84989d0","features":{},"yanked":false}
{"name":"base64","vers":"0.4.1","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"9892882c3bd89ed02dec391c128984c772b663a29700c32b5de0b33861cdf2bd","features":{},"yanked":false}
{"name":"base64","vers":"0.5.0","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"6c902f607515b17ee069f2757c58a6d4b2afa7411b8995f96c4a3c19247b5fcf","features":{},"yanked":false}
{"name":"base64","vers":"0.5.1","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"124e5332dfc4e387b4ca058909aa175c0c3eccf03846b7c1a969b9ad067b8df2","features":{},"yanked":false}
{"name":"base64","vers":"0.5.2","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"30e93c03064e7590d0466209155251b90c22e37fab1daf2771582598b5827557","features":{},"yanked":false}
{"name":"base64","vers":"0.4.2","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"979d348dc50dfcd050a87df408ec61f01a0a27ee9b4ebdc6085baba8275b2c7f","features":{},"yanked":false}
{"name":"base64","vers":"0.3.1","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"33678aec1839cd59fbb29a90950baa95fc4a5464711a7858f5ba68dbc3d3eab2","features":{},"yanked":false}
{"name":"base64","vers":"0.6.0","deps":[{"name":"byteorder","req":"^1.0.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"= 0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.2.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"96434f987501f0ed4eb336a411e0631ecd1afa11574fe148587adc4ff96143c9","features":{},"yanked":false}
{"name":"base64","vers":"0.7.0","deps":[{"name":"byteorder","req":"^1.1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.2.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"5032d51da2741729bfdaeb2664d9b8c6d9fd1e2b90715c660b6def36628499c2","features":{},"yanked":false}
{"name":"base64","vers":"0.8.0","deps":[{"name":"byteorder","req":"^1.1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.2.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"7c4a342b450b268e1be8036311e2c613d7f8a7ed31214dff1cc3b60852a3168d","features":{},"yanked":false}
{"name":"base64","vers":"0.9.0","deps":[{"name":"byteorder","req":"^1.1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.3.15","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.2.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"229d032f1a99302697f10b27167ae6d03d49d032e6a8e2550e8d3fc13356d2b4","features":{},"yanked":false}
{"name":"base64","vers":"0.9.1","deps":[{"name":"byteorder","req":"^1.1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.4","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.2.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"9263aa6a38da271eec5c91a83ce1e800f093c8535788d403d626d8d5c3f8f007","features":{},"yanked":false}
{"name":"base64","vers":"0.9.2","deps":[{"name":"byteorder","req":"^1.1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.4","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.2.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"85415d2594767338a74a30c1d370b2f3262ec1b4ed2d7bba5b3faf4de40467d9","features":{},"yanked":false}
{"name":"base64","vers":"0.9.3","deps":[{"name":"byteorder","req":"^1.1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.4","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"safemem","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"}],"cksum":"489d6c0ed21b11d038c31b6ceccca973e65d73ba3bd8ecb9a2babf5546164643","features":{},"yanked":false}
{"name":"base64","vers":"0.10.0","deps":[{"name":"byteorder","req":"^1.2.6","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"rand","req":"^0.5.5","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"621fc7ecb8008f86d7fb9b95356cd692ce9514b80a86d85b397f32a22da7b9e2","features":{},"yanked":false}
{"name":"base64","vers":"0.10.1","deps":[{"name":"byteorder","req":"^1.2.6","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"criterion","req":"^0.2","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"0b25d992356d2eb0ed82172f5248873db5560c4721f564b13cb5193bda5e668e","features":{},"yanked":false}
{"name":"base64","vers":"0.11.0","deps":[{"name":"criterion","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"doc-comment","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"b41b7ea54a0c9d92199de89e20e58d49f02f8e699814ef3fdf266f6f748d15c7","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.12.0","deps":[{"name":"criterion","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"doc-comment","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"7d5ca2cd0adc3f48f9e9ea5a6bbdf9ccc0bfade884847e484d452414c7ccffb3","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.12.1","deps":[{"name":"criterion","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"doc-comment","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"53d1ccbaf7d9ec9537465a97bf19edc1a4e158ecb49fc16178202238c569cc42","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.12.2","deps":[{"name":"criterion","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"doc-comment","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"e223af0dc48c96d4f8342ec01a4974f139df863896b316681efd36742f22cc67","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.12.3","deps":[{"name":"criterion","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"3441f0f7b02788e948e47f457ca01f1d7e6d92c693bc132c22b087d3141c03ff","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.13.0","deps":[{"name":"criterion","req":"=0.3.2","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"904dfeac50f3cdaba28fc6f57fdcddb75f49ed61346676a78c4ffe55877802fd","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.20.0-alpha.1","deps":[{"name":"criterion","req":"^0.3.4","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.11.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.1.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.21","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"149ea5dc24cb11513350770afebba32b68e3d2e356f9221351a2a1ee89112a82","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.47.0"}
{"name":"base64","vers":"0.13.1","deps":[{"name":"criterion","req":"=0.3.2","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.6.1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"9e1b586273c5702936fe7b7d6896644d8be71e6314cfe09d3167c95f712589e8","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false}
{"name":"base64","vers":"0.20.0","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"0ea22880d78093b0cbe17c89f64a7d457941e65759157ec6cb31a31d652b05e5","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.57.0"}
{"name":"base64","vers":"0.21.0-beta.1","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"64e0a6b26c7b009f913ee6af053ce7cde07f295881d8ec808f813d267be68183","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.57.0"}
{"name":"base64","vers":"0.21.0-beta.2","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"0547ed923c8857baee67fa8d9f2b4de4ad63863bd4b774689af8987cc9bfc8c5","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.57.0"}
{"name":"base64","vers":"0.21.0-rc.1","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"339eb223c8f495cef5b03f1727754538a468edaeba28bab0886ecadca774a3b7","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.57.0"}
{"name":"base64","vers":"0.21.0","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"a4a4ddaa51a5bc52a6948f74c06d20aaaddb71924eab79b8c97a8c556e942d6a","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.57.0"}
{"name":"base64","vers":"0.21.1","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"3f1e31e207a6b8fb791a38ea3105e6cb541f55e4d029902d3039a4ad07cc4105","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.60.0"}
{"name":"base64","vers":"0.21.2","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"604178f6c5c21f02dc555784810edfb88d34ac2c73b2eae109655649ee73ce3d","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.57.0"}
{"name":"base64","vers":"0.21.3","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"lazy_static","req":"^1.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"414dcefbc63d77c526a76b3afcf6fbb9b5e2791c19c3aa2297733208750c6e53","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.48.0"}
{"name":"base64","vers":"0.21.4","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"lazy_static","req":"^1.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"9ba43ea6f343b788c8764558649e08df62f86c6ef251fdaeb1ffd010a9ae50a2","features":{"alloc":[],"default":["std"],"std":[]},"yanked":false,"rust_version":"1.48.0"}
{"name":"base64","vers":"0.21.5","deps":[{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"lazy_static","req":"^1.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.12.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.3.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"structopt","req":"^0.3.26","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"35636a1494ede3b646cc98f74f8e62c773a38a659ebc777a2cf26b9b74171df9","features":{"alloc":[],"default":["std"],"std":["alloc"]},"yanked":false,"rust_version":"1.48.0"}
{"name":"base64","vers":"0.21.6","deps":[{"name":"clap","req":"^3.2.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"once_cell","req":"^1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.13.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.6.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"strum","req":"^0.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"c79fed4cdb43e993fcdadc7e58a09fd0e3e649c4436fa11da71c9f1f3ee7feb9","features":{"alloc":[],"default":["std"],"std":["alloc"]},"yanked":false,"rust_version":"1.48.0"}
{"name":"base64","vers":"0.21.7","deps":[{"name":"clap","req":"^3.2.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"once_cell","req":"^1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.13.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.6.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"strum","req":"^0.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"9d297deb1925b89f2ccc13d7635fa0714f12c87adce1c75356b39ca9b7178567","features":{"alloc":[],"default":["std"],"std":["alloc"]},"yanked":false,"rust_version":"1.48.0"}
{"name":"base64","vers":"0.22.0","deps":[{"name":"clap","req":"^3.2.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"once_cell","req":"^1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.13.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.6.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"strum","req":"^0.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"9475866fec1451be56a3c2400fd081ff546538961565ccb5b7142cbd22bc7a51","features":{"alloc":[],"default":["std"],"std":["alloc"]},"yanked":false,"rust_version":"1.48.0"}
{"name":"base64","vers":"0.22.1","deps":[{"name":"clap","req":"^3.2.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"criterion","req":"^0.4.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"once_cell","req":"^1","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8.5","features":["small_rng"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest","req":"^0.13.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rstest_reuse","req":"^0.6.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"strum","req":"^0.25","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"72b3254f16251a8381aa12e40e3c4d2f0199f8c6508fbecb9d91f575e0fbb8c6","features":{"alloc":[],"default":["std"],"std":["alloc"]},"yanked":false,"rust_version":"1.48.0"}
        "#.trim();

        let base64_bytes = r#"
        {"name":"base64-bytes","vers":"0.1.0","deps":[{"name":"base64","req":"^0.22","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"bincode","req":"^1.3","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"rand","req":"^0.8","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"serde","req":"^1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"normal"},{"name":"serde","req":"^1.0","features":["derive"],"optional":false,"default_features":true,"target":null,"kind":"dev"},{"name":"serde_json","req":"^1.0","features":[],"optional":false,"default_features":true,"target":null,"kind":"dev"}],"cksum":"7ce54e4e485fa0eed9c3aa5348162be09168f75bb5be7bc6587bcf2a65ee1386","features":{},"yanked":false}
        "#.trim();

        let result = Crate::from_slice(base64.as_bytes()).expect("Exists");

        println!("{:?}", result);
    }
}
