//! Reads `opt-level` and `codegen-units` from the workspace's `[profile.*]`
//! tables, so the generated build can honour them per crate.
//!
//! `buildRustCrate` compiles every crate with `-C opt-level=3` (release) and
//! a fixed number of codegen units, whatever the `Cargo.toml` says. We
//! resolve the settings cargo would use for each crate (the profile itself,
//! then `package."*"` for non-members, then `package.<name>` /
//! `package."<name>@<version>"`) and emit them into `Cargo.nix`. Only values
//! the user set explicitly are emitted; cargo's own defaults are not filled
//! in, so crates without settings keep `buildRustCrate`'s behaviour.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{format_err, Error};
use semver::Version;
use serde::{Deserialize, Serialize};

/// The settings for one crate in one profile.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProfileSettings {
    /// `-C opt-level`: "0".."3", "s" or "z".
    pub opt_level: Option<String>,
    /// `-C codegen-units`.
    pub codegen_units: Option<u64>,
}

impl ProfileSettings {
    fn from_toml(value: &toml::Value) -> ProfileSettings {
        let opt_level = match value.get("opt-level") {
            Some(toml::Value::Integer(i)) => Some(i.to_string()),
            Some(toml::Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        let codegen_units = value
            .get("codegen-units")
            .and_then(toml::Value::as_integer)
            .and_then(|i| u64::try_from(i).ok());
        ProfileSettings {
            opt_level,
            codegen_units,
        }
    }

    fn merge(&mut self, other: &ProfileSettings) {
        if other.opt_level.is_some() {
            self.opt_level.clone_from(&other.opt_level);
        }
        if other.codegen_units.is_some() {
            self.codegen_units = other.codegen_units;
        }
    }

    fn is_empty(&self) -> bool {
        self.opt_level.is_none() && self.codegen_units.is_none()
    }
}

/// One `[profile.<name>]` table.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Profile {
    base: ProfileSettings,
    /// `[profile.<name>.package.<spec>]`, keyed by spec (`*`, `name` or `name@version`).
    packages: BTreeMap<String, ProfileSettings>,
}

impl Profile {
    fn from_toml(value: Option<&toml::Value>) -> Profile {
        let Some(value) = value else {
            return Profile::default();
        };
        let packages = value
            .get("package")
            .and_then(toml::Value::as_table)
            .map(|table| {
                table
                    .iter()
                    .map(|(spec, v)| (spec.clone(), ProfileSettings::from_toml(v)))
                    .collect()
            })
            .unwrap_or_default();
        Profile {
            base: ProfileSettings::from_toml(value),
            packages,
        }
    }

    fn for_crate(&self, name: &str, version: &Version, is_member: bool) -> Option<ProfileSettings> {
        let mut settings = self.base.clone();
        if !is_member {
            if let Some(all) = self.packages.get("*") {
                settings.merge(all);
            }
        }
        for spec in [name.to_string(), format!("{}@{}", name, version)] {
            if let Some(s) = self.packages.get(&spec) {
                settings.merge(s);
            }
        }
        if settings.is_empty() {
            None
        } else {
            Some(settings)
        }
    }
}

/// The `release` and `dev` profiles of a workspace.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CargoProfiles {
    release: Profile,
    dev: Profile,
}

/// The settings that apply to one crate.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CrateProfiles {
    /// Used when building with `release = true`.
    pub release: Option<ProfileSettings>,
    /// Used when building with `release = false`.
    pub dev: Option<ProfileSettings>,
}

impl CrateProfiles {
    /// True if neither profile sets anything for this crate.
    pub fn is_empty(&self) -> bool {
        self.release.is_none() && self.dev.is_none()
    }
}

impl CargoProfiles {
    /// Parses the `[profile]` table of a `Cargo.toml`'s contents.
    pub fn parse(contents: &str) -> Result<CargoProfiles, Error> {
        let manifest: toml::Value = toml::from_str(contents)?;
        let profile = manifest.get("profile");
        Ok(CargoProfiles {
            release: Profile::from_toml(profile.and_then(|p| p.get("release"))),
            dev: Profile::from_toml(profile.and_then(|p| p.get("dev"))),
        })
    }

    /// Reads the profiles of the workspace's root `Cargo.toml`; empty if there is none.
    pub fn read_workspace(workspace_root: Option<&str>) -> Result<CargoProfiles, Error> {
        let Some(root) = workspace_root else {
            return Ok(CargoProfiles::default());
        };
        let path = Path::new(root).join("Cargo.toml");
        if !path.exists() {
            return Ok(CargoProfiles::default());
        }
        let contents = std::fs::read_to_string(&path)
            .map_err(|e| format_err!("while reading {}: {}", path.display(), e))?;
        CargoProfiles::parse(&contents)
            .map_err(|e| format_err!("while parsing profiles of {}: {}", path.display(), e))
    }

    /// The settings cargo would use for this crate.
    pub fn for_crate(&self, name: &str, version: &Version, is_member: bool) -> CrateProfiles {
        CrateProfiles {
            release: self.release.for_crate(name, version, is_member),
            dev: self.dev.for_crate(name, version, is_member),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
[workspace]
members = ["a"]

[profile.release]
codegen-units = 16

[profile.release.package."*"]
codegen-units = 1

[profile.release.package.big]
opt-level = 1

[profile.release.package."pinned@1.0.0"]
opt-level = "s"

[profile.dev]
opt-level = 1
"#;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn member_gets_profile_base() {
        let p = CargoProfiles::parse(MANIFEST).unwrap();
        let c = p.for_crate("a", &v("0.1.0"), true);
        assert_eq!(
            c.release,
            Some(ProfileSettings {
                opt_level: None,
                codegen_units: Some(16)
            })
        );
        assert_eq!(
            c.dev,
            Some(ProfileSettings {
                opt_level: Some("1".into()),
                codegen_units: None
            })
        );
    }

    #[test]
    fn dependency_gets_wildcard_then_name() {
        let p = CargoProfiles::parse(MANIFEST).unwrap();
        let dep = p.for_crate("serde", &v("1.0.0"), false);
        assert_eq!(dep.release.unwrap().codegen_units, Some(1));
        let big = p.for_crate("big", &v("2.0.0"), false).release.unwrap();
        assert_eq!(big.codegen_units, Some(1));
        assert_eq!(big.opt_level.as_deref(), Some("1"));
        assert_eq!(
            p.for_crate("pinned", &v("1.0.0"), false)
                .release
                .unwrap()
                .opt_level
                .as_deref(),
            Some("s")
        );
        assert_eq!(
            p.for_crate("pinned", &v("1.0.1"), false)
                .release
                .unwrap()
                .opt_level,
            None
        );
    }

    #[test]
    fn no_profiles_means_nothing_emitted() {
        let p = CargoProfiles::parse("[package]\nname = \"x\"\n").unwrap();
        assert!(p.for_crate("x", &v("0.1.0"), true).is_empty());
    }
}
