//! The CLI's own build identity, reported read-only.
//!
//! `forge build-info` answers what the running `cli` binary was built from: the package version, the
//! target it was compiled for and whether it was a debug or release build. Every field is a
//! compile-time constant, so the command reads no environment, touches no database, calls no network
//! and leaves no filesystem state behind — which is what makes it the safe question to ask any Forge
//! host, even one with no `.env.local` and no credentials.
//!
//!   cargo run -p cli -- forge build-info
//!   cargo run -p cli -- forge build-info --format json

use crate::forge::Failure;
use serde::Serialize;

/// The identity of the running CLI binary, every field resolved at compile time.
#[derive(Debug, Clone, Serialize)]
pub struct BuildInfo {
    /// `CARGO_PKG_VERSION` of the `cli` crate (inherited from the workspace version).
    pub cli_version: &'static str,
    /// The OS the binary was compiled for (`std::env::consts::OS`), not the host it happens to run on.
    pub os: &'static str,
    /// The architecture the binary was compiled for (`std::env::consts::ARCH`).
    pub arch: &'static str,
    /// `debug` for a debug build, `release` for an optimised one.
    pub profile: &'static str,
}

impl BuildInfo {
    /// Read the compile-time identity. No I/O, no environment, no providers.
    pub fn current() -> Self {
        Self {
            cli_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        }
    }
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut as_json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                if args.get(index + 1).map(String::as_str) != Some("json") {
                    return Err(Failure::usage("usage: forge build-info [--format json]"));
                }
                as_json = true;
                index += 2;
            }
            "--format=json" => {
                as_json = true;
                index += 1;
            }
            other => {
                return Err(Failure::usage(format!(
                    "unknown argument `{other}`; usage: forge build-info [--format json]"
                )));
            }
        }
    }

    let info = BuildInfo::current();
    if as_json {
        let rendered = serde_json::to_string_pretty(&info).map_err(|error| {
            Failure::failed(format!("cannot render build info as JSON: {error}"))
        })?;
        println!("{rendered}");
    } else {
        println!("cli_version: {}", info.cli_version);
        println!("os: {}", info.os);
        println!("arch: {}", info.arch);
        println!("profile: {}", info.profile);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_reports_the_compiled_identity() {
        let info = BuildInfo::current();
        assert_eq!(info.cli_version, env!("CARGO_PKG_VERSION"));
        assert!(!info.cli_version.is_empty(), "the version is never empty");
        assert_eq!(info.os, std::env::consts::OS);
        assert_eq!(info.arch, std::env::consts::ARCH);
        assert_eq!(
            info.profile,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
    }

    /// The JSON answer carries the same four fields under their documented names.
    #[test]
    fn the_json_shape_carries_all_four_fields() {
        let value = serde_json::to_value(BuildInfo::current()).expect("the build info serialises");
        for field in ["cli_version", "os", "arch", "profile"] {
            assert!(value.get(field).is_some(), "missing {field}");
        }
    }
}
