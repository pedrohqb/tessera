//! Registering p11-kit-client.so in an NSS database (a Firefox profile), and
//! the small database Tessera makes for Papers.
//!
//! Papers needs no module registered: when NSS opens a database it loads
//! the root-certificates module by itself, and in the Flatpak runtime that
//! is p11-kit, which reaches the system's p11-kit server once Papers may use
//! its socket. Registering p11-kit-client.so too made every certificate show
//! twice. Papers only needs a database of its own, so that poppler does not
//! fall back to ~/.pki/nssdb (which lists the roots module again).
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
///
/// While it runs, Firefox holds a POSIX record lock (fcntl) on the profile's
/// .parentlock. That lock goes away with the process, even when it is
/// killed, unlike the "lock" symlink, which a killed Firefox leaves behind
/// (and which made "Set up" refuse forever). Locks work across sandboxes:
/// they belong to the file, not to a PID namespace.
pub fn profile_in_use(dir: &Path) -> bool {
    #[repr(C)]
    struct Flock {
        l_type: i16,
        l_whence: i16,
        l_start: i64,
        l_len: i64,
        l_pid: i32,
    }
    extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    const F_GETLK: i32 = 5;
    const F_WRLCK: i16 = 1;
    const F_UNLCK: i16 = 2;
    use std::os::fd::AsRawFd;
    let Ok(file) = fs::File::open(dir.join(".parentlock")) else {
        return false;
    };
    let mut lock = Flock {
        l_type: F_WRLCK,
        l_whence: 0,
        l_start: 0,
        l_len: 0,
        l_pid: 0,
    };
    // SAFETY: F_GETLK reads and fills the struct flock passed by pointer.
    // Flock matches glibc's struct flock on x86_64 (short, short, off_t,
    // off_t, pid_t, with the C padding repr(C) gives), `lock` is alive and
    // writable for the call, and the descriptor belongs to `file`, open
    // until the end of the function.
    let rv = unsafe { fcntl(file.as_raw_fd(), F_GETLK, &mut lock as *mut Flock) };
    rv == 0 && lock.l_type != F_UNLCK
}

type NssInitialize =
    unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char, u32) -> c_int;
type NssShutdown = unsafe extern "C" fn() -> c_int;
type AddNewModule = unsafe extern "C" fn(*const c_char, *const c_char, c_ulong, c_ulong) -> c_int;
type PortGetError = unsafe extern "C" fn() -> c_int;
type DeleteModule = unsafe extern "C" fn(*const c_char, *mut c_int) -> c_int;

/// Is the Papers database there, without the client module?
pub fn poppler_db_ready(dir: &Path) -> bool {
    dir.join("cert9.db").exists() && !has_module(dir)
}

/// Runs in the child process: create the database (or open it) and remove
/// the client module an older Tessera registered there.
pub fn set_up_poppler_db_in_process(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    // SAFETY: as in add_module_in_process (nss.h, secmod.h): the symbols are
    // called with their declared signatures, the CStrings outlive the calls
    // that take their pointers, and NSS is shut down on every path after a
    // successful NSS_Initialize. This runs in a child process of its own,
    // so NSS's global state is not shared with anything else.
    unsafe {
        let lib = Library::new("libnss3.so").context("loading libnss3.so")?;
        let init: Symbol<NssInitialize> = lib.get(b"NSS_Initialize\0")?;
        let shutdown: Symbol<NssShutdown> = lib.get(b"NSS_Shutdown\0")?;
        let delete: Symbol<DeleteModule> = lib.get(b"SECMOD_DeleteModule\0")?;
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
        if has_module(dir) {
            let name = CString::new(paths::NSS_MODULE_NAME)?;
            let mut kind: c_int = 0;
            if delete(name.as_ptr(), &mut kind) != 0 {
                let code = err();
                shutdown();
                bail!("SECMOD_DeleteModule failed (NSS error {code})");
            }
        }
        shutdown();
    }
    Ok(())
}

/// Called from the GUI: runs `tessera nss-poppler <dir>`.
pub fn set_up_poppler_db(dir: &Path) -> Result<Outcome> {
    if poppler_db_ready(dir) {
        return Ok(Outcome::AlreadyPresent);
    }
    let exe = std::env::current_exe().context("finding our own executable")?;
    let out = Command::new(exe)
        .arg("nss-poppler")
        .arg(dir)
        .output()
        .context("running nss-poppler")?;
    if out.status.success() {
        return Ok(Outcome::Added);
    }
    Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr).trim()))
}

/// Runs in the child process.
pub fn add_module_in_process(dir: &Path) -> Result<Outcome> {
    if has_module(dir) {
        return Ok(Outcome::AlreadyPresent);
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    // SAFETY: the symbols are looked up by name in libnss3 and called with
    // the signatures declared above, which are those of nss.h, secmod.h and
    // secerr.h (NSS_Initialize, NSS_Shutdown, SECMOD_AddNewModule,
    // PORT_GetError); `lib` outlives every call. The CStrings outlive the
    // calls that take their pointers. NSS is shut down on every path after a
    // successful NSS_Initialize. This runs in a child process of its own
    // (`tessera nss-add-module`), so NSS's global state is not shared.
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

#[cfg(test)]
mod tests {
    use super::profile_in_use;
    use std::process::{Command, Stdio};
    use std::{fs, thread, time::Duration};

    #[test]
    fn firefox_lock_is_the_fcntl_lock_on_parentlock() {
        let dir = std::env::temp_dir().join(format!("tessera-lock-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // A stale "lock" symlink, as a killed Firefox leaves it, is ignored.
        let _ = std::os::unix::fs::symlink("127.0.0.1:+1", dir.join("lock"));
        fs::write(dir.join(".parentlock"), b"").unwrap();
        assert!(!profile_in_use(&dir));
        // Another process holding the lock, as a running Firefox does.
        let script = format!(
            "import fcntl,time; f=open('{}','r+'); fcntl.lockf(f, fcntl.LOCK_EX); print('locked', flush=True); time.sleep(5)",
            dir.join(".parentlock").display()
        );
        let mut child = Command::new("python3")
            .args(["-c", &script])
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3");
        let mut out = child.stdout.take().unwrap();
        let mut buf = [0u8; 7];
        std::io::Read::read_exact(&mut out, &mut buf).unwrap();
        thread::sleep(Duration::from_millis(50));
        assert!(profile_in_use(&dir));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!profile_in_use(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }
}
