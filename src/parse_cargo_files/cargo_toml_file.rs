use std::collections::HashMap;
use std::fs;
use std::hash::Hash;
use std::path::PathBuf;

use derive_builder::Builder;
use itertools::Either::{Left, Right};
use itertools::Itertools;
use serde::Deserialize;

type DependencyMap = HashMap<String, Dependency>;
type SpecificVersionDependencyMap = HashMap<SpecificDependencyVersion, Dependency>;


#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct SpecificDependencyVersion {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct CargoToml {
    #[allow(dead_code)]
    pub package: Package,

    #[allow(dead_code)]
    pub dependencies: Option<DependencyMap>,

    #[allow(dead_code)]
    #[serde(rename = "dev-dependencies")]
    pub dev_dependencies: Option<DependencyMap>,

    #[allow(dead_code)]
    #[serde(rename = "build-dependencies")]
    pub build_dependencies: Option<DependencyMap>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Package {
    #[allow(dead_code)]
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Detailed(DependencyDetail),
}


#[derive(Debug, Deserialize, PartialEq, Clone, Builder)]
pub struct DependencyDetail {
    #[allow(dead_code)]
    #[builder(setter(into, strip_option), default)]
    pub version: Option<String>,

    #[allow(dead_code)]
    #[builder(setter(into, strip_option), default)]
    pub features: Option<Vec<String>>,

    #[allow(dead_code)]
    #[serde(rename = "default-features")]
    #[builder(setter(into, strip_option), default)]
    pub default_features: Option<bool>,

    #[allow(dead_code)]
    #[builder(setter(into, strip_option), default)]
    pub path: Option<String>,
}

fn parse_cargo_file(content: String) -> CargoToml {
    let cargo_toml: CargoToml = toml::from_str(&content).expect("Failed to deserialize Cargo.toml");

    return cargo_toml;
}

// Parse cargo file from path with support for local dependencies
pub fn parse_cargo_file_from_path(cargo_file_path: String) -> SpecificVersionDependencyMap {
    let cargo_file_path_buf = PathBuf::from(cargo_file_path.clone());
    let cargo_file_folder = cargo_file_path_buf.parent().expect("cargo file path must be inside a directory");
    let cargo_file_content = fs::read_to_string(cargo_file_path.clone()).expect(format!("Failed to read Cargo.toml file at {}", cargo_file_path).as_str());

    let parsed_cargo = parse_cargo_file(cargo_file_content);

    let (mut all_deps_specific, path_deps) = get_deps_maps_from_cargo(parsed_cargo);

    path_deps.iter().for_each(|(key, val)| {
        match val {
            Dependency::Version(_) => unreachable!("Should not reach here"),
            Dependency::Detailed(detail) => {
                let path = detail.clone().path.expect("Must have path");

                let path = cargo_file_folder.join(path);

                let file_path = path.join("Cargo.toml");

                // Add nested paths deps
                all_deps_specific.extend(parse_cargo_file_from_path(file_path.to_str().expect("Failed to convert path to string").to_string()))
            }
        }
    });


    return all_deps_specific;
}

// Return tuple of (merged dependencies with specific version and without local paths , local dependencies)
fn get_deps_maps_from_cargo(cargo: CargoToml) -> (SpecificVersionDependencyMap, DependencyMap) {
    let mut dependencies = cargo.dependencies.unwrap_or_default();
    let dev_dependencies = cargo.dev_dependencies.unwrap_or_default();
    let build_dependencies = cargo.build_dependencies.unwrap_or_default();

    let mut all_deps = dependencies.clone();
    all_deps.extend(dev_dependencies);
    all_deps.extend(build_dependencies);

    let (remote_deps, local_deps): (Vec<(SpecificDependencyVersion, Dependency)>, Vec<(String, Dependency)>) = all_deps
        .into_iter()
        .partition_map(|(k, v)| {
            let value = v.clone();
            return match v {
                Dependency::Version(version) => {
                    Left((
                        // Key
                        SpecificDependencyVersion {
                            name: k.clone(),
                            version: version.clone(),
                        },
                        // Value
                        value
                    ))
                }
                Dependency::Detailed(details) => {
                    if details.path.is_some() {
                        return Right((
                            // Key
                            k.clone(),
                            // Value
                            value
                        ));
                    }
                    let version = details.clone().version.expect("Must have version");
                    Left((
                        // Key
                        SpecificDependencyVersion {
                            name: k.clone(),
                            version,
                        },
                        // Value
                        value
                    ))
                }
            };
        });

    return (
        remote_deps.iter().cloned().collect(),
        local_deps.iter().cloned().collect()
    );
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;

    fn create_cargo_file(add: &str) -> String {
        // language=toml
        let base = r#"
[package]
name = "cargo-collect"
version = "0.1.1"
edition = "2021"
readme = "README.md"
        "#;

        return format!("{}\n{}", base, add);
    }

    fn create_dependency_map<const SIZE: usize>(deps_array: [(&str, Dependency); SIZE]) -> DependencyMap {
        return deps_array.iter()
            .cloned()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    }

    // #####################################################################
    // Single parsing file tests
    // #####################################################################

    #[test]
    fn read_cargo_file() {
        let content = fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml file");

        // Testing not crash
        let result = parse_cargo_file(content);
    }

    #[test]
    fn combined_parse_single_file() {
        // language=toml
        let cargo_toml = create_cargo_file(
            r#"
[dependencies]
dep1 = "0.1"
dep2 = { version = "0.2" }
dep3 = { version = "0.3", features = ["derive"] }
dep4 = { version = "0.4", default-features = false }
dep5 = { version = "0.5", default-features = false, features = ["std"] }

[dev-dependencies]
dev1 = "0.6"
dev2 = { version = "0.7" }
dev3 = { version = "0.8", features = ["full"] }
dev4 = { version = "0.9", default-features = true }
dev5 = { version = "0.10", default-features = false, features = ["env"] }

[build-dependencies]
build1 = "0.11"
build2 = { version = "0.12" }
build3 = { version = "0.13", features = ["something"] }
build4 = { version = "0.14", default-features = true }
build5 = { version = "0.15", default-features = true, features = ["else"] }
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("dep1", Dependency::Version("0.1".to_string())),
            ("dep2", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.2".to_string())
                    .build().unwrap()
            )),
            ("dep3", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.3".to_string())
                    .features(vec![
                        "derive".to_string()
                    ])
                    .build().unwrap()
            )),
            ("dep4", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.4".to_string())
                    .default_features(false)
                    .build().unwrap()
            )),
            ("dep5", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.5".to_string())
                    .default_features(false)
                    .features(vec![
                        "std".to_string()
                    ])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);

        let dev_deps = cargo.dev_dependencies.expect("Must have dev-dependencies");

        let expected_dev_deps = create_dependency_map([
            ("dev1", Dependency::Version("0.6".to_string())),
            ("dev2", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.7".to_string())
                    .build().unwrap()
            )),
            ("dev3", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.8".to_string())
                    .features(vec![
                        "full".to_string()
                    ])
                    .build().unwrap()
            )),
            ("dev4", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.9".to_string())
                    .default_features(true)
                    .build().unwrap()
            )),
            ("dev5", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.10".to_string())
                    .default_features(false)
                    .features(vec![
                        "env".to_string()
                    ])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(dev_deps, expected_dev_deps);

        let build_deps = cargo.build_dependencies.expect("Must have build-dependencies");

        let expected_build_deps = create_dependency_map([
            ("build1", Dependency::Version("0.11".to_string())),
            ("build2", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.12".to_string())
                    .build().unwrap()
            )),
            ("build3", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.13".to_string())
                    .features(vec![
                        "something".to_string()
                    ])
                    .build().unwrap()
            )),
            ("build4", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.14".to_string())
                    .default_features(true)
                    .build().unwrap()
            )),
            ("build5", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.15".to_string())
                    .default_features(true)
                    .features(vec![
                        "else".to_string()
                    ])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(build_deps, expected_build_deps);
    }

    #[test]
    fn no_deps() {
        let no_deps_cargo_toml = create_cargo_file("");

        let cargo = parse_cargo_file(no_deps_cargo_toml);

        assert_eq!(cargo.dependencies.is_none(), true);
    }

    #[test]
    fn empty_deps() {
        // language=toml
        let no_deps_cargo_toml = create_cargo_file("[dependencies]");

        let cargo = parse_cargo_file(no_deps_cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies map");

        let expected_deps = create_dependency_map([]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn single_dep_with_just_version_string() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        anyhow = "1.0"
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("anyhow", Dependency::Version("1.0".to_string()))
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn multiple_deps_with_just_version_string() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        anyhow = "1.0"
        reqwest = "0.11"
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("anyhow", Dependency::Version("1.0".to_string())),
            ("reqwest", Dependency::Version("0.11".to_string())),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn single_dep_with_just_version_object() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        anyhow = { version = "1.0" }
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("anyhow", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("1.0".to_string())
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn multiple_deps_with_just_version_object() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        anyhow = { version = "1.0" }
        clap = { version = "4.1" }
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("anyhow", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("1.0".to_string())
                    .build().unwrap()
            )),
            ("clap", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("4.1".to_string())
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn multiple_deps_with_just_version_string_and_object() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        crates-index = "0.19"
        clap = { version = "4.1" }
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("crates-index", Dependency::Version("0.19".to_string())),
            ("clap", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("4.1".to_string())
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn single_dep_with_version_and_features() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        tokio = { version = "1.26", features = ["full"] }
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("tokio", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("1.26".to_string())
                    .features(vec!["full".to_string()])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn multiple_deps_with_version_and_features() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        clap = { version = "4.1", features = ["derive"] }
        tokio = { version = "1.26", features = ["full"] }
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("clap", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("4.1".to_string())
                    .features(vec!["derive".to_string()])
                    .build().unwrap()
            )),
            ("tokio", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("1.26".to_string())
                    .features(vec!["full".to_string()])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn single_dep_with_version_and_features_and_default_features() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        tracing = {version = "0.1", default-features = false, features = ["std"]}
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("tracing", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.1".to_string())
                    .default_features(false)
                    .features(vec!["std".to_string()])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn multiple_deps_with_version_and_features_and_default_features() {
        // language=toml
        let cargo_toml = create_cargo_file(r#"
        [dependencies]
        tracing = {version = "0.1", default-features = false, features = ["std"]}
        tracing-subscriber = {version = "0.3", default-features = false, features = ["ansi", "env-filter", "fmt"]}
        "#.trim());

        let cargo = parse_cargo_file(cargo_toml);

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("tracing-subscriber", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.3".to_string())
                    .default_features(false)
                    .features(vec!["ansi".to_string(), "env-filter".to_string(), "fmt".to_string()])
                    .build().unwrap()
            )),
            ("tracing", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .version("0.1".to_string())
                    .default_features(false)
                    .features(vec!["std".to_string()])
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn parse_root_workspace_cargo_file() {
        // language=toml
        let root_cargo_toml = r#"
[workspace.package]
version = "0.1.0"
edition = "2021"

[package]
name = "cryptography-rust"
version.workspace = true
authors.workspace = true
edition.workspace = true
publish.workspace = true
rust-version.workspace = true

[workspace]
members = [
    "crates/some_local",
]

[dependencies]
dep1 = "0.1"
some_local = { path = "crates/some_local" }
        "#.trim();

        let cargo = parse_cargo_file(root_cargo_toml.to_string());

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("dep1", Dependency::Version("0.1".to_string())),
            ("some_local", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .path("crates/some_local".to_string())
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    #[test]
    fn parse_workspace_lib_cargo_file() {
        // language=toml
        let root_cargo_toml = r#"
[package]
name = "cryptography-key-parsing"
version.workspace = true
authors.workspace = true
edition.workspace = true
publish.workspace = true
rust-version.workspace = true

[dependencies]
openssl-sys = "0.9.102"
cryptography-x509 = { path = "../cryptography-x509" }

        "#.trim();

        let cargo = parse_cargo_file(root_cargo_toml.to_string());

        let deps = cargo.dependencies.expect("Must have dependencies");

        let expected_deps = create_dependency_map([
            ("openssl-sys", Dependency::Version("0.9.102".to_string())),
            ("cryptography-x509", Dependency::Detailed(
                DependencyDetailBuilder::default()
                    .path("../cryptography-x509".to_string())
                    .build().unwrap()
            )),
        ]);

        assert_eq!(deps, expected_deps);
    }

    // #####################################################################
    // Combined parsing file tests (support for local paths)
    // #####################################################################

    // Save map as each key is path inside tmp dir and value is the file content, returning the root folder path
    fn save_map_as_files_in_tmp_dir<const SIZE: usize>(file_map: [(&str, &str); SIZE]) -> String {
        let tmp_dir = tempdir::TempDir::new("cargo_toml_test").expect("Failed to create temp dir");
        let tmp_dir = tmp_dir.into_path();

        file_map.iter().for_each(|(path, content)| {
            let file_path = tmp_dir.join(path);

            let parent = file_path.parent().expect("Must have parent");

            // If not in the root tmp dir, then create all directories
            if parent != tmp_dir {
                fs::create_dir_all(parent).expect("Failed to create dir")
            }

            fs::write(file_path, content).expect(format!("Failed to write file \"{}\"", path).as_str());
        });

        return tmp_dir.to_str().expect("Failed to convert path to string").to_string();
    }

    fn create_specific_version_dependency_map<const SIZE: usize>(deps_array: [(SpecificDependencyVersion, Dependency); SIZE]) -> SpecificVersionDependencyMap {
        return deps_array.iter()
            .cloned()
            .collect();
    }

    #[test]
    fn read_cargo_file_with_local() {
        // Testing not crash
        parse_cargo_file_from_path("Cargo.toml".to_string());
    }

    #[test]
    fn support_local_deps() {
        // language=toml
        let root_cargo_toml = r#"
[workspace.package]
version = "0.1.0"
edition = "2021"

[package]
name = "cryptography-rust"
version.workspace = true
authors.workspace = true
edition.workspace = true
publish.workspace = true
rust-version.workspace = true

[workspace]
members = [
    "crates/some_local_1",
    "crates/some_local_2",
]

[dependencies]
dep1 = "0.1"
some_local_1 = { path = "crates/some_local_1" }
some_local_2 = { path = "crates/some_local_2" }
        "#.trim();

        // language=toml
        let l1_cargo_toml = r#"
[package]
name = "some_local_1"
version.workspace = true
authors.workspace = true
edition.workspace = true
publish.workspace = true
rust-version.workspace = true

[dependencies]
pyo3 = { version = "0.22.1", features = ["abi3"] }
openssl-sys = "0.9.102"
some_local_2 = { path = "../some_local_2" }
        "#.trim();

        // language=toml
        let l2_cargo_toml = r#"
[package]
name = "some_local_2"

[dependencies]
# Different version of openssl-sys
openssl-sys = "0.9.103"
mydep = "1.0.0"
        "#.trim();

        let root_dir = save_map_as_files_in_tmp_dir([
            ("Cargo.toml", root_cargo_toml),
            ("crates/some_local_1/Cargo.toml", l1_cargo_toml),
            ("crates/some_local_2/Cargo.toml", l2_cargo_toml),
        ]);

        let root_cargo_file_path = PathBuf::from(root_dir).join("Cargo.toml").to_str().expect("Failed to convert path to string").to_string();

        let deps = parse_cargo_file_from_path(root_cargo_file_path);

        let expected_deps = create_specific_version_dependency_map([
            (
                SpecificDependencyVersion {
                    name: "dep1".to_string(),
                    version: "0.1".to_string(),
                },
                Dependency::Version("0.1".to_string())
            ),
            (
                SpecificDependencyVersion {
                    name: "openssl-sys".to_string(),
                    version: "0.9.102".to_string(),
                },
                Dependency::Version("0.9.102".to_string())
            ),
            (
                SpecificDependencyVersion {
                    name: "openssl-sys".to_string(),
                    version: "0.9.103".to_string(),
                },
                Dependency::Version("0.9.103".to_string())
            ),
            (
                SpecificDependencyVersion {
                    name: "pyo3".to_string(),
                    version: "0.22.1".to_string(),
                },
                Dependency::Detailed(
                    DependencyDetailBuilder::default()
                        .version("0.22.1".to_string())
                        .features(vec!["abi3".to_string()])
                        .build().unwrap()
                )
            ),
            (
                SpecificDependencyVersion {
                    name: "mydep".to_string(),
                    version: "1.0.0".to_string(),
                },
                Dependency::Version("1.0.0".to_string())
            ),
        ]);

        assert_eq!(deps, expected_deps);
    }
}



