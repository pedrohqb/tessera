//! Restarting Tessera from its window. A Flatpak sandbox mounts the folders
//! it is granted only if they exist when it starts: an app opened for the
//! first time after Tessera started (its profile or settings folder made
//! then), or a driver extension installed since, stays out of sight until
//! Tessera starts again.
//!
//! In the Flatpak, a new instance is started through the Flatpak portal
//! (Spawn), which runs the app again in a sandbox of its own, mounted
//! afresh (and with the latest installed version), outside this one: it
//! goes on after this instance quits. It starts with --restarted, which
//! makes it wait until this instance has left the session bus (see
//! wait_for_previous), so it does not just hand its window back to it.

use adw::prelude::*;
use gtk::{gio, glib};
use std::time::{Duration, Instant};

use crate::paths;

pub const RESTARTED: &str = "--restarted";

/// FLATPAK_SPAWN_FLAGS_LATEST_VERSION: an update installed meanwhile is
/// the one that starts.
const LATEST_VERSION: u32 = 2;

/// A GVariant bytestring literal (the portal takes NUL-terminated byte
/// arrays), escaped for the text format.
fn bytestring(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
    format!("b'{escaped}'")
}

/// The parameters of org.freedesktop.portal.Flatpak.Spawn: cwd, argv, fds,
/// envs, flags, options.
fn spawn_params(cwd: &str) -> String {
    format!(
        "({}, [{}, {}], @a{{uh}} {{}}, @a{{ss}} {{}}, uint32 {LATEST_VERSION}, @a{{sv}} {{}})",
        bytestring(cwd),
        bytestring("tessera"),
        bytestring(RESTARTED)
    )
}

/// Start the new instance; Ok once it is on its way.
async fn spawn_new() -> Result<(), String> {
    if !paths::in_flatpak() {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(exe)
            .arg(RESTARTED)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let home = paths::home();
    let params = glib::Variant::parse(None, &spawn_params(&home.to_string_lossy()))
        .map_err(|e| e.to_string())?;
    let bus = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|e| e.to_string())?;
    bus.call_future(
        Some("org.freedesktop.portal.Flatpak"),
        "/org/freedesktop/portal/Flatpak",
        "org.freedesktop.portal.Flatpak",
        "Spawn",
        Some(&params),
        Some(glib::VariantTy::new("(u)").expect("valid type")),
        gio::DBusCallFlags::NONE,
        10_000,
    )
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Start a new instance, then quit this one (the "quit" action stops the
/// servers and takes Tessera's modules off the host first). On failure,
/// `failed` gets the reason and this instance stays.
pub fn restart(app: &adw::Application, failed: impl FnOnce(String) + 'static) {
    let app = app.clone();
    glib::spawn_future_local(async move {
        match spawn_new().await {
            Ok(()) => app.activate_action("quit", None),
            Err(e) => failed(e),
        }
    });
}

/// In the instance started by `restart`: wait (up to 20 s) until the one
/// that started it has left the session bus, where it owns the app's name.
pub fn wait_for_previous() {
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
        return;
    };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        let owned = bus
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                Some(&(paths::APP_ID,).to_variant()),
                Some(glib::VariantTy::new("(b)").expect("valid type")),
                gio::DBusCallFlags::NONE,
                2_000,
                None::<&gio::Cancellable>,
            )
            .ok()
            .and_then(|v| v.get::<(bool,)>())
            .map(|(b,)| b);
        if owned != Some(true) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spawn_parameters_parse() {
        let v = glib::Variant::parse(None, &spawn_params("/home/a 'b'")).unwrap();
        assert_eq!(v.type_().as_str(), "(ayaaya{uh}a{ss}ua{sv})");
        let cwd: Vec<u8> = v.child_value(0).fixed_array::<u8>().unwrap().to_vec();
        assert_eq!(cwd, b"/home/a 'b'\0");
        let argv = v.child_value(1);
        assert_eq!(argv.n_children(), 2);
        assert_eq!(
            argv.child_value(1).fixed_array::<u8>().unwrap(),
            b"--restarted\0"
        );
        assert_eq!(v.child_value(4).get::<u32>(), Some(LATEST_VERSION));
    }
}
