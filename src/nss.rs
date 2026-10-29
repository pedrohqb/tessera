//! Registering p11-kit-client.so in an NSS database (a Firefox profile, or
//! the small database Tessera makes for Papers).
//!
//! NSS is loaded at run time from the runtime's libnss3.so, so Tessera needs
//! neither NSS headers nor the modutil/certutil tools. The work runs in a
//! child process (`tessera nss-add-module <dir>`), keeping NSS's global state
//! and the modules it loads out of the GUI process.

use anyhow::{anyhow, bail, Context, Result};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_ulong, CString};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::paths;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Added,
    AlreadyPresent,
}

/// Is the client module already listed in this database's pkcs11.txt?
pub fn has_module(dir: &Path) -> bool {
    fs::read_to_string(dir.join("pkcs11.txt"))
        .map(|t| {
            t.lines()
                .any(|l| l.trim() == format!("library={}", paths::CLIENT_MODULE))
        })
        .unwrap_or(false)
}

/// Firefox holds its profile locked while running; adding a module then would
/// be lost or corrupt the database.
pub fn profile_in_use(dir: &Path) -> bool {
    // Firefox keeps a "lock" symlink in the profile while it runs and
    // removes it on exit (the .parentlock file stays).
    fs::symlink_metadata(dir.join("lock")).is_ok()
}

type NssInitialize =
    unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char, u32) -> c_int;
type NssShutdown = unsafe extern "C" fn() -> c_int;
type AddNewModule = unsafe extern "C" fn(*const c_char, *const c_char, c_ulong, c_ulong) -> c_int;
type PortGetError = unsafe extern "C" fn() -> c_int;

/// Runs in the child process.
pub fn add_module_in_process(dir: &Path) -> Result<Outcome> {
    if has_module(dir) {
        return Ok(Outcome::AlreadyPresent);
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    // SAFETY: libnss3 is a well-behaved C library; the symbols below have the
    // signatures declared above (nss.h, secmod.h, secerr.h).
    unsafe {
        let lib = Library::new("libnss3.so").context("loading libnss3.so")?;
        let init: Symbol<NssInitialize> = lib.get(b"NSS_Initialize\0")?;
        let shutdown: Symbol<NssShutdown> = lib.get(b"NSS_Shutdown\0")?;
        let add: Symbol<AddNewModule> = lib.get(b"SECMOD_AddNewModule\0")?;
        let err: Symbol<PortGetError> = lib.get(b"PORT_GetError\0")?;

        let config = CString::new(format!("sql:{}", dir.display()))?;
        let empty = CString::new("")?;
        let secmod = CString::new("secmod.db")?;
        if init(
            config.as_ptr(),
            empty.as_ptr(),
            empty.as_ptr(),
            secmod.as_ptr(),
            0,
        ) != 0
        {
            bail!("NSS_Initialize failed (NSS error {})", err());
        }
        let name = CString::new(paths::NSS_MODULE_NAME)?;
        let module = CString::new(paths::CLIENT_MODULE)?;
        let rv = add(name.as_ptr(), module.as_ptr(), 0, 0);
        let code = err();
        shutdown();
        if rv != 0 {
            bail!("SECMOD_AddNewModule failed (NSS error {code})");
        }
    }
    Ok(Outcome::Added)
}

/// Called from the GUI: runs `tessera nss-add-module <dir>`.
pub fn add_module(dir: &Path) -> Result<Outcome> {
    if has_module(dir) {
        return Ok(Outcome::AlreadyPresent);
    }
    let exe = std::env::current_exe().context("finding our own executable")?;
    let out = Command::new(exe)
        .arg("nss-add-module")
        .arg(dir)
        .output()
        .context("running nss-add-module")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if out.status.success() {
        return Ok(if stdout.trim() == "present" {
            Outcome::AlreadyPresent
        } else {
            Outcome::Added
        });
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(anyhow!("{}", stderr.trim()))
}
