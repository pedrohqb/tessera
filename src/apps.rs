//! Pointing client apps at Tessera.
//!
//! Each Flatpak app has its own p11-kit configuration in
//! ~/.var/app/<id>/config/pkcs11. A module file there with
//!
//!     remote: unix:path=/run/user/<uid>/tessera/pkcs11
//!
//! makes p11-kit (its proxy module, and anything that loads modules through
//! p11-kit, such as GnuTLS) serve Tessera's tokens inside that app. The app
//! also needs access to the socket: `flatpak override --user
//! --filesystem=xdg-run/tessera <id>`.

use anyhow::{Context, Result};
use std::fs;

use crate::paths;

pub struct ClientApp {
    pub id: &'static str,
    pub name: &'static str,
}

pub const FIREFOX: ClientApp = ClientApp {
    id: "org.mozilla.firefox",
    name: "Firefox",
};
pub const PAPERS: ClientApp = ClientApp {
    id: "org.gnome.Papers",
    name: "Papers",
};
pub const PJEOFFICE: ClientApp = ClientApp {
    id: "br.jus.cnj.PJeOfficePro",
    name: "PJeOffice Pro",
};

fn module_contents() -> String {
    format!(
        "# Written by Tessera ({}): the tokens it serves.\nremote: {}\n",
        paths::APP_ID,
        paths::server_address()
    )
}

/// The command that lets an app reach Tessera's socket.
pub fn override_command(app_id: &str) -> String {
    format!("flatpak override --user --filesystem=xdg-run/tessera {app_id}")
}

pub fn module_file_present(app_id: &str) -> bool {
    fs::read_to_string(paths::app_module_file(app_id))
        .map(|t| t == module_contents())
        .unwrap_or(false)
}

/// Write the module file into the app's p11-kit configuration.
pub fn write_module_file(app_id: &str) -> Result<()> {
    let file = paths::app_module_file(app_id);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(&file, module_contents()).with_context(|| format!("writing {}", file.display()))
}
