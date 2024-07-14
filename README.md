# cargo-collect

Program for recursive download of crates and their dependencies from crates.io.

This is a fork of [cargo-collect](https://gitlab.com/TalRoni/cargo-collect) by TalRoni

## Description

`cargo-collect` can be used to download a gzipped archive of given crate, in the exact form that it was uploaded to crates.io.

This can be useful for a variety of things, such as:
 - download the crates to upload to third party registry with [cargo-upload](https://gitlab.com/TalRoni/cargo-upload).
 - security auditing of crates (esp. when a crate repository is missing).
 - reproducing a bug that only occurs in uploaded versions of your crate.

## Installation

### Pre-Built Binaries
Download the binary from the [releases page](https://github.com/rluvaton/cargo-collect/releases/latest).

### From Source
```bash
$ git clone
$ cd cargo-collect
$ cargo build --release
$ ./target/release/cargo-collect --help
```

## Usage
To download the newest version of foo crate and its dependencies, do this:

> Change `./cargo-collect` to the path of the binary you downloaded and add extension (.exe) if you are on windows.
```bash
$ ./cargo-collect foo --output /path/to/optput/folder
```
For more detailed usage instructions, run `./cargo-collect --help`.

### Help

```bash
$ ./cargo-collect --help

Cargo tool for download crate file and its dependencies recursively.

Usage: cargo-collect [OPTIONS]

Options:
  -n, --crate-name <CRATE_NAME>
          The crate name

  -v, --crate-version-req <CRATE_VERSION_REQ>
          The version requirement of the crate can be =1.0.0 or ^1.0 (see semver.org)

  -o, --output <OUTPUT>
          The output folder to put all crate files
          
          [default: deps]

      --cargo-file <CARGO_FILE>
          The Cargo.toml file to take dependencies from. This will take the latest version that the version requirement (This should be used when the crate is not published)
          
          Support workspaces

      --cargo-lock-file <CARGO_LOCK_FILE>
          The Cargo.lock file to take dependencies from. This will take exact versions of the dependencies. (This should be used when the crate is not published)
          
  -u, --update-index
          Whether to update the local index of crates.io.
          
          Use this when cant find crate version that you know exists


  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version


Examples:
    # Collect the dependencies of the crate `serde` with version requirement `^1.0.0`
    # Save the crates files in "./deps" folder
    ./cargo-collect --crate-name serde --crate-version-req "^1.0.0"

    # Collect the dependencies of the crate `serde` with version requirement `=1.0.0`
    # and put them in the folder `output`
    ./cargo-collect --crate-name serde --crate-version-req "=1.0.0" --output output

    # Collect all dependencies used by the local Cargo.toml file that match the versions specified.
    # This support workspaces as well
    # Useful for example when some python library (e.g. cryptography) have Rust implementation
    # that is not published to crates.io and it's required in order to install the library
    ./cargo-collect --cargo-file Cargo.toml

    # Collect all dependencies used by the local Cargo.lock file that match the EXACT
    # versions specified.
    # Useful for example when some python library (e.g. cryptography) have Rust implementation
    # that is not published to crates.io and it's required in order to install the library
    ./cargo-collect --cargo-lock-file Cargo.lock

```

## License
cargo-collect is licensed under the terms of the GNU GENERAL PUBLIC LICENSE Version 3


