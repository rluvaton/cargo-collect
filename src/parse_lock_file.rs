use std::fs;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CargoToml {
    #[allow(dead_code)] // Disable dead code warning for the entire struct
    package: Package,
    #[allow(dead_code)]
    dependencies: Dependencies,
}

#[derive(Debug, Deserialize)]
struct CargoLockToml {
    #[allow(dead_code)] // Disable dead code warning for the entire struct
    package: Package,
    #[allow(dead_code)]
    dependencies: Dependencies,
}

#[derive(Debug, Deserialize)]
struct Package {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    edition: String,
}

#[derive(Debug, Deserialize)]
struct Dependencies {
    #[allow(dead_code)]
    serde: SerdeDependency,
    #[allow(dead_code)]
    toml: String,
}

#[derive(Debug, Deserialize)]
struct SerdeDependency {
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    features: Vec<String>,
}


fn parse_cargo_file(content: String) -> CargoToml {
    let cargo_toml: CargoToml = toml::from_str(&content).expect("Failed to deserialize Cargo.toml");

    return cargo_toml;
}

fn parse_cargo_lock_file(content: String) -> CargoLockToml {
    let cargo_toml: CargoLockToml = toml::from_str(&content).expect("Failed to deserialize Cargo.toml");

    return cargo_toml;
}


#[cfg(test)]
mod tests_mod {
    use std::fs;
    use super::*;

    #[test]
    fn read_cargo_file() {
        let content = fs::read_to_string("Cargo.toml").expect("Failed to read Cargo.toml file");

        let cargo = parse_cargo_file(content);

        println!("{:#?}", cargo);
    }

    #[test]
    fn read_lock_file() {
        let content = fs::read_to_string("Cargo.lock").expect("Failed to read Cargo.lock file");

        let cargo = parse_cargo_lock_file(content);

        println!("{:#?}", cargo);
    }
}



