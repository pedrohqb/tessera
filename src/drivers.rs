//! Token drivers: PKCS#11 modules described by a `driver.toml`.
//!
//! Each driver lives in its own directory, `<drivers dir>/<id>/driver.toml`:
//! OpenSC, bundled with Tessera, and the drivers installed as Flatpak
//! extensions (flatpak/extensions), each mounted in its own folder. Paths in
//! the file are absolute, or relative to that directory. The id is the
//! folder's name, lowercased.
//!
//! ```toml
//! name = "SafeSign IC"
//! vendor = "A.E.T. Europe"
//! description = "GD Burti and other SafeSign tokens"
//! module = "extra/libaetpkss.so"
//!
//! library_path = ["extra/usr/lib"]   # optional
//!
//! [[tools]]
//! name = "Token Administration"
//! exec = ["extra/tokenadmin"]
//! ```
//!
//! `library_path` is put in LD_LIBRARY_PATH for the process that loads the
//! module (for libraries the runtime doesn't have). `env` sets other
//! variables for that process; a leading `~/` in a value is the home folder,
//! and a leading `./` the driver's directory.
//!
//! `[when_on.<other driver id>.env]` changes some of those variables while
//! that other driver is switched on too. OpenSC uses it to leave to
//! SafeNet's driver the cards both know (see flatpak/drivers/opensc).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;

#[derive(Debug, Clone, Deserialize)]
pub struct Tool {
    pub name: String,
    pub exec: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DriverFile {
    name: String,
    #[serde(default)]
    vendor: String,
    #[serde(default)]
    description: String,
    module: String,
    #[serde(default)]
    tools: Vec<Tool>,
    /// Directories to create in $HOME before the driver is used (some vendor
    /// tools expect them and fail if they are missing).
    #[serde(default)]
    create_dirs: Vec<String>,
    #[serde(default)]
    library_path: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    when_on: BTreeMap<String, WhenOn>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WhenOn {
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Driver {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub description: String,
    pub module: PathBuf,
    pub tools: Vec<Tool>,
    pub create_dirs: Vec<String>,
    pub library_path: Vec<PathBuf>,
    /// Environment for the process that loads the module.
    pub env: BTreeMap<String, String>,
    /// Changes to `env` while another driver (the key) is on as well.
    pub when_on: BTreeMap<String, BTreeMap<String, String>>,
}

impl Driver {
    /// The module file is there (an extra-data driver whose download failed,
    /// or a broken extension, is listed but not loaded).
    pub fn available(&self) -> bool {
        self.module.is_file()
    }

    /// The module's environment when the drivers `on` are switched on.
    pub fn env_with(&self, on: &[String]) -> BTreeMap<String, String> {
        let mut env = self.env.clone();
        for (other, changes) in &self.when_on {
            if other != &self.id && on.contains(other) {
                env.extend(changes.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        env
    }
}

/// A value of `env`: `~/` is the home folder, `./` the driver's directory.
fn env_value(dir: &Path, v: String) -> String {
    if let Some(rest) = v.strip_prefix("~/") {
        paths::home().join(rest).to_string_lossy().into_owned()
    } else if let Some(rest) = v.strip_prefix("./") {
        dir.join(rest).to_string_lossy().into_owned()
    } else {
        v
    }
}

fn env_map(dir: &Path, env: BTreeMap<String, String>) -> BTreeMap<String, String> {
    env.into_iter()
        .map(|(k, v)| (k, env_value(dir, v)))
        .collect()
}

/// A path in driver.toml: absolute, or relative to the driver's directory
/// (`..` taken out, so OpenSC's module, built into Tessera, reads as
/// /app/lib/opensc-pkcs11.so).
fn resolve(dir: &Path, p: &str) -> PathBuf {
    use std::path::Component;
    let p = Path::new(p);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let mut out = dir.to_path_buf();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn load_one(dir: &Path) -> Result<Driver> {
    let file = dir.join("driver.toml");
    let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
    let d: DriverFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", file.display()))?;
    // Lowercased: an extension's folder is named after its id
    // (io.github.pedrohqb.Tessera.Driver.SafeNet → SafeNet), the built-in
    // drivers' are lowercase, and settings and sockets use the id.
    let id = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let tools = d
        .tools
        .into_iter()
        .map(|mut t| {
            if let Some(first) = t.exec.first_mut() {
                *first = resolve(dir, first).to_string_lossy().into_owned();
            }
            t
        })
        .collect();
    Ok(Driver {
        id,
        name: d.name,
        vendor: d.vendor,
        description: d.description,
        module: resolve(dir, &d.module),
        tools,
        create_dirs: d.create_dirs,
        library_path: d.library_path.iter().map(|p| resolve(dir, p)).collect(),
        env: env_map(dir, d.env),
        when_on: d
            .when_on
            .into_iter()
            .map(|(other, w)| (other.to_lowercase(), env_map(dir, w.env)))
            .collect(),
    })
}

/// All drivers found, sorted by name. A driver id found in more than one
/// directory is taken from the first.
pub fn discover() -> Vec<Driver> {
    let mut found: BTreeMap<String, Driver> = BTreeMap::new();
    for root in paths::driver_dirs() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            match load_one(&dir) {
                Ok(d) => {
                    found.entry(d.id.clone()).or_insert(d);
                }
                Err(e) if dir.join("driver.toml").exists() => eprintln!("tessera: {e:#}"),
                Err(_) => {}
            }
        }
    }
    let mut v: Vec<Driver> = found.into_values().collect();
    v.sort_by_key(|d| d.name.to_lowercase());
    v
}

/// Create the directories drivers ask for (e.g. SerproID's certificate
/// folder).
pub fn prepare_dirs(drivers: &[Driver]) {
    for d in drivers.iter().filter(|d| d.available()) {
        for rel in &d.create_dirs {
            let p = paths::home().join(rel.trim_start_matches("~/"));
            if let Err(e) = fs::create_dir_all(&p) {
                eprintln!("tessera: cannot create {}: {e}", p.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_resolves_relative_paths() {
        let tmp = std::env::temp_dir().join(format!("tessera-test-{}", std::process::id()));
        let dir = tmp.join("demo");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("driver.toml"),
            "name = \"Demo\"\nmodule = \"lib/demo.so\"\n[[tools]]\nname = \"Admin\"\nexec = [\"bin/admin\", \"--x\"]\n",
        )
        .unwrap();
        let d = load_one(&dir).unwrap();
        assert_eq!(d.id, "demo");
        assert_eq!(d.module, dir.join("lib/demo.so"));
        assert_eq!(d.tools[0].exec[0], dir.join("bin/admin").to_string_lossy());
        assert_eq!(d.tools[0].exec[1], "--x");
        assert!(!d.available());
        fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn env_follows_the_other_drivers() {
        let tmp = std::env::temp_dir().join(format!("tessera-test-env-{}", std::process::id()));
        let dir = tmp.join("opensc");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("driver.toml"),
            "name = \"OpenSC\"\nmodule = \"../../../opensc-pkcs11.so\"\n\
             [env]\nOPENSC_CONF = \"./opensc.conf\"\nKEEP = \"x\"\n\
             [when_on.safenet.env]\nOPENSC_CONF = \"./other.conf\"\n",
        )
        .unwrap();
        let d = load_one(&dir).unwrap();
        assert_eq!(
            d.module,
            dir.ancestors().nth(3).unwrap().join("opensc-pkcs11.so")
        );
        let conf = |env: &BTreeMap<String, String>| env["OPENSC_CONF"].clone();
        let alone = d.env_with(&["opensc".to_string()]);
        assert_eq!(conf(&alone), dir.join("opensc.conf").to_string_lossy());
        assert_eq!(alone["KEEP"], "x");
        let both = d.env_with(&["opensc".to_string(), "safenet".to_string()]);
        assert_eq!(conf(&both), dir.join("other.conf").to_string_lossy());
        assert_eq!(both["KEEP"], "x");
        fs::remove_dir_all(tmp).unwrap();
    }
}
