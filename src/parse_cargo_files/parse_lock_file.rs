use std::fs;
use std::hash::Hash;

use derive_builder::Builder;
use itertools::Itertools;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Clone, Builder)]
pub struct CargoLockToml {
    #[allow(dead_code)] // Disable dead code warning for the entire struct
    #[builder(default = "3")]
    pub version: u8,
    #[allow(dead_code)] // Disable dead code warning for the entire struct
    pub package: Option<Vec<Package>>,
}


#[derive(Debug, Deserialize, PartialEq, Clone, Builder)]
pub struct Package {
    #[allow(dead_code)]
    #[builder(setter(into, strip_option))]
    pub name: String,
    #[allow(dead_code)]
    #[builder(setter(into, strip_option))]
    pub version: String,
    #[allow(dead_code)]
    #[builder(setter(into, strip_option), default)]
    pub source: Option<String>,
    #[allow(dead_code)]
    #[builder(setter(into, strip_option), default)]
    pub checksum: Option<String>,
    #[allow(dead_code)]
    #[builder(setter(into, strip_option), default)]
    pub dependencies: Option<Vec<String>>,
}

pub fn parse_cargo_lock_file(content: String) -> CargoLockToml {
    let cargo_toml: CargoLockToml = toml::from_str(&content).expect("Failed to deserialize Cargo.lock");

    if cargo_toml.version != 3 {
        panic!("Cargo.lock version must be 3");
    }

    return cargo_toml;
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;

    fn create_cargo_lock_file(add: &str) -> String {
        // language=toml
        let base = r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3
        "#;

        return format!("{}\n{}", base, add);
    }


    // #####################################################################
    // Single parsing file tests
    // #####################################################################

    #[test]
    fn read_cargo_lock_file() {
        let content = fs::read_to_string("Cargo.lock").expect("Failed to read Cargo.lock file");

        // Testing not crash
        let result = parse_cargo_lock_file(content);
    }

    #[test]
    fn no_packages() {
        let cargo_lock = create_cargo_lock_file("");

        let cargo = parse_cargo_lock_file(cargo_lock);

        assert_eq!(cargo.package, None);
    }

    #[test]
    fn single_remote_package() {
        // language=toml
        let cargo_lock = create_cargo_lock_file(r#"
[[package]]
name = "anyhow"
version = "1.0.70"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "7de8ce5e0f9f8d88245311066a578d72b7af3e7088f32783804676302df237e4"
        "#);

        let cargo = parse_cargo_lock_file(cargo_lock);

        let expected_packages = vec![
            PackageBuilder::default()
                .name("anyhow")
                .version("1.0.70")
                .source("registry+https://github.com/rust-lang/crates.io-index")
                .checksum("7de8ce5e0f9f8d88245311066a578d72b7af3e7088f32783804676302df237e4")
                .build().unwrap()
        ];

        assert_eq!(cargo.package.unwrap(), expected_packages);

    }

    #[test]
    fn multiple_remote_packages() {
        // language=toml
        let cargo_lock = create_cargo_lock_file(r#"
[[package]]
name = "anyhow"
version = "1.0.70"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "7de8ce5e0f9f8d88245311066a578d72b7af3e7088f32783804676302df237e4"

[[package]]
name = "autocfg"
version = "1.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "d468802bab17cbc0cc575e9b053f41e72aa36bfa6b7f55e3529ffa43161b97fa"

        "#);

        let cargo = parse_cargo_lock_file(cargo_lock);

        let expected_packages = vec![
            PackageBuilder::default()
                .name("anyhow")
                .version("1.0.70")
                .source("registry+https://github.com/rust-lang/crates.io-index")
                .checksum("7de8ce5e0f9f8d88245311066a578d72b7af3e7088f32783804676302df237e4")
                .build().unwrap(),

            PackageBuilder::default()
                .name("autocfg")
                .version("1.1.0")
                .source("registry+https://github.com/rust-lang/crates.io-index")
                .checksum("d468802bab17cbc0cc575e9b053f41e72aa36bfa6b7f55e3529ffa43161b97fa")
                .build().unwrap()
        ];

        assert_eq!(cargo.package.unwrap(), expected_packages);
    }

    #[test]
    fn remote_package_with_deps() {
        // language=toml
        let cargo_lock = create_cargo_lock_file(r#"

[[package]]
name = "aho-corasick"
version = "0.7.20"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "cc936419f96fa211c1b9166887b38e5e40b19958e5b895be7c1f93adec7071ac"
dependencies = [
 "memchr",
]

[[package]]
name = "memchr"
version = "2.5.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "2dffe52ecf27772e601905b7522cb4ef790d2cc203488bbd0e2fe85fcb74566d"

        "#);

        let cargo = parse_cargo_lock_file(cargo_lock);

        let expected_packages = vec![
            PackageBuilder::default()
                .name("aho-corasick")
                .version("0.7.20")
                .source("registry+https://github.com/rust-lang/crates.io-index")
                .checksum("cc936419f96fa211c1b9166887b38e5e40b19958e5b895be7c1f93adec7071ac")
                .dependencies(vec!["memchr".to_string()])
                .build().unwrap(),

            PackageBuilder::default()
                .name("memchr")
                .version("2.5.0")
                .source("registry+https://github.com/rust-lang/crates.io-index")
                .checksum("2dffe52ecf27772e601905b7522cb4ef790d2cc203488bbd0e2fe85fcb74566d")
                .build().unwrap()
        ];

        assert_eq!(cargo.package.unwrap(), expected_packages);
    }

    #[test]
    fn local_package() {
        // language=toml
        let cargo_lock = create_cargo_lock_file(r#"
[[package]]
name = "cryptography-x509"
version = "0.1.0"
        "#);

        let cargo = parse_cargo_lock_file(cargo_lock);

        let expected_packages = vec![
            PackageBuilder::default()
                .name("cryptography-x509")
                .version("0.1.0")
                .build().unwrap(),
        ];

        assert_eq!(cargo.package.unwrap(), expected_packages);
    }

    #[test]
    fn local_package_with_deps() {
        // language=toml
        let cargo_lock = create_cargo_lock_file(r#"
[[package]]
name = "asn1"
version = "0.16.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "532ceda058281b62096b2add4ab00ab3a453d30dee28b8890f62461a0109ebbd"

[[package]]
name = "cryptography-x509"
version = "0.1.0"
        "#);

        let cargo = parse_cargo_lock_file(cargo_lock);

        let expected_packages = vec![

            PackageBuilder::default()
                .name("asn1")
                .version("0.16.2")
                .source("registry+https://github.com/rust-lang/crates.io-index")
                .checksum("532ceda058281b62096b2add4ab00ab3a453d30dee28b8890f62461a0109ebbd")
                .build().unwrap(),

            PackageBuilder::default()
                .name("cryptography-x509")
                .version("0.1.0")
                .build().unwrap(),
        ];

        assert_eq!(cargo.package.unwrap(), expected_packages);
    }
}



