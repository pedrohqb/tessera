//! `tessera remote --provider MODULE pkcs11:...`: Tessera's own stand-in for
//! `p11-kit remote`, run by `p11-kit server` for each connection (through
//! P11_KIT_PRIVATEDIR, see server.rs). It does what `p11-kit remote` does,
//! through the same libp11-kit calls, on stdin/stdout.
//!
//! Why: some drivers identify the program that loaded them. SerproID sends
//! it with each signing request, and its server refuses
//! `/usr/libexec/p11-kit/p11-kit-remote`; with this, the program is Tessera.
//!
//! Before loading the driver, it gives the driver an ordinary process:
//! signals back to their defaults (p11-kit server ignores some, and ignored
//! signals survive exec), and the p11-kit channel moved off stdin/stdout,
//! which become /dev/null and stderr. A driver that runs helper programs or
//! writes to stdout then behaves as it does inside an app.
//!
//! The driver always gets mechanism parameters that stay valid after the
//! *Init call that passed them (see stable.rs): SerproID needs that.
//!
//! TESSERA_REMOTE_LOAD=raw serves the driver's own function list (dlopen +
//! C_GetFunctionList) instead of p11-kit's managed wrapper around it.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;
use std::{fs, os::unix::fs::PermissionsExt};

use crate::paths;

type ModuleLoad = unsafe extern "C" fn(*const c_char, c_int) -> *mut c_void;
type GetFunctionList = unsafe extern "C" fn(*mut *mut c_void) -> std::ffi::c_ulong;
type ModuleRelease = unsafe extern "C" fn(*mut c_void);
type ServeTokens =
    unsafe extern "C" fn(*const *const c_char, usize, *mut c_void, c_int, c_int) -> c_int;

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
    fn sigprocmask(how: i32, set: *const u64, old: *mut u64) -> i32;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn dup2(old: i32, new: i32) -> i32;
}

const SIG_DFL: usize = 0;
const SIG_SETMASK: i32 = 2;
const F_DUPFD_CLOEXEC: i32 = 1030;

/// Signals back to their default action, none blocked.
unsafe fn reset_signals() {
    for s in 1..65 {
        if s != 9 && s != 19 {
            signal(s, SIG_DFL);
        }
    }
    let empty = [0u64; 16]; // sigset_t
    sigprocmask(SIG_SETMASK, empty.as_ptr(), std::ptr::null_mut());
}

/// Move the p11-kit channel (stdin, stdout) to new descriptors closed on
/// exec, put /dev/null on stdin and stderr on stdout. Returns (in, out).
unsafe fn move_channel() -> Option<(c_int, c_int)> {
    let input = fcntl(0, F_DUPFD_CLOEXEC, 3);
    let output = fcntl(1, F_DUPFD_CLOEXEC, 3);
    if input < 0 || output < 0 {
        return None;
    }
    let null = std::fs::File::open("/dev/null").ok()?;
    use std::os::fd::AsRawFd;
    if dup2(null.as_raw_fd(), 0) < 0 || dup2(2, 1) < 0 {
        return None;
    }
    Some((input, output))
}

pub fn run(args: &[String]) -> i32 {
    let mut provider: Option<&str> = None;
    let mut tokens: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-p" | "--provider" => provider = it.next().map(String::as_str),
            s if s.starts_with("--provider=") => provider = Some(&s[11..]),
            s => tokens.push(s),
        }
    }
    let Some(provider) = provider else {
        eprintln!("tessera remote: --provider is required");
        return 2;
    };
    if tokens.is_empty() {
        tokens.push("pkcs11:");
    }
    // SAFETY: libp11-kit's public API (p11-kit/p11-kit.h, p11-kit/remote.h);
    // the pointers passed stay alive for the calls.
    unsafe {
        reset_signals();
        let (fd_in, fd_out) = move_channel().unwrap_or((0, 1));
        let lib = match Library::new("libp11-kit.so.0") {
            Ok(l) => l,
            Err(e) => {
                eprintln!("tessera remote: {e}");
                return 1;
            }
        };
        let (Ok(load), Ok(release), Ok(serve)) = (
            lib.get::<ModuleLoad>(b"p11_kit_module_load\0"),
            lib.get::<ModuleRelease>(b"p11_kit_module_release\0"),
            lib.get::<ServeTokens>(b"p11_kit_remote_serve_tokens\0"),
        ) else {
            eprintln!("tessera remote: libp11-kit lacks the remote API");
            return 1;
        };
        let (load, release, serve): (
            Symbol<ModuleLoad>,
            Symbol<ModuleRelease>,
            Symbol<ServeTokens>,
        ) = (load, release, serve);
        let Ok(c_provider) = CString::new(provider) else {
            return 2;
        };
        let raw = std::env::var("TESSERA_REMOTE_LOAD").as_deref() == Ok("raw");
        let mut driver: Option<Library> = None;
        let module = if raw {
            let lib = match Library::new(provider) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("tessera remote: {e}");
                    return 1;
                }
            };
            let mut list: *mut c_void = std::ptr::null_mut();
            let rv = match lib.get::<GetFunctionList>(b"C_GetFunctionList\0") {
                Ok(f) => f(&mut list),
                Err(_) => 1,
            };
            driver = Some(lib);
            if rv != 0 {
                list = std::ptr::null_mut();
            }
            list
        } else {
            load(c_provider.as_ptr(), 0)
        };
        if module.is_null() {
            eprintln!("tessera remote: could not load {provider}");
            return 1;
        }
        eprintln!(
            "tessera remote: serving {provider} ({}, channel on fds {fd_in}/{fd_out})",
            if raw { "raw" } else { "managed" }
        );
        // Always: the driver gets mechanism parameters that stay valid
        // after the *Init call (see stable.rs).
        let served = crate::stable::wrap(module);
        let served = if std::env::var("TESSERA_REMOTE_TRACE").as_deref() == Ok("1") {
            crate::trace::wrap(served)
        } else {
            served
        };
        let c_tokens: Vec<CString> = tokens
            .iter()
            .filter_map(|t| CString::new(*t).ok())
            .collect();
        let ptrs: Vec<*const c_char> = c_tokens.iter().map(|c| c.as_ptr()).collect();
        let rv = serve(ptrs.as_ptr(), ptrs.len(), served, fd_in, fd_out);
        if driver.is_none() {
            release(module);
        }
        drop(driver);
        rv
    }
}

/// The directory given to `p11-kit server` as P11_KIT_PRIVATEDIR: it holds a
/// `p11-kit-remote` that runs `tessera remote`.
pub fn private_dir() -> std::io::Result<PathBuf> {
    let dir = paths::log_dir().with_file_name("libexec");
    fs::create_dir_all(&dir)?;
    let exe = std::env::current_exe()?;
    let script = dir.join("p11-kit-remote");
    let body = format!("#!/bin/sh\nexec '{}' remote \"$@\"\n", exe.display());
    if fs::read_to_string(&script).ok().as_deref() != Some(body.as_str()) {
        fs::write(&script, body)?;
    }
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    let bin = dir.join("bin");
    fs::create_dir_all(&bin)?;
    let notify = bin.join("notify-send");
    if fs::read_to_string(&notify).ok().as_deref() != Some(NOTIFY_SEND) {
        fs::write(&notify, NOTIFY_SEND)?;
    }
    fs::set_permissions(&notify, fs::Permissions::from_mode(0o755))?;
    Ok(dir)
}

/// Put first in the drivers' PATH (see server.rs). SerproID announces each
/// request with `notify-send`, naming an image from its own folder, which
/// the desktop outside the sandbox cannot open: the notification showed a
/// broken picture. This drops the icon and image hints and shows Tessera's
/// icon.
const NOTIFY_SEND: &str = r#"#!/bin/sh
# Written by Tessera: notify-send for the drivers, with Tessera's icon.
hint=0 skip=0
for a do
    shift
    if [ $skip = 1 ]; then skip=0; continue; fi
    if [ $hint = 1 ]; then
        hint=0
        case $a in *image-path*|*image_path*|*image-data*|*image_data*|*icon_data*) continue ;; esac
        set -- "$@" --hint="$a"
        continue
    fi
    case $a in
        -i|--icon) skip=1; continue ;;
        -i*|--icon=*) continue ;;
        -h|--hint) hint=1; continue ;;
        --hint=*image-path*|--hint=*image_path*|--hint=*image-data*|--hint=*image_data*|--hint=*icon_data*) continue ;;
    esac
    set -- "$@" "$a"
done
real=$(PATH=${PATH#*:} command -v notify-send) || exit 0
exec "$real" --icon=io.github.pedrohqb.Tessera "$@"
"#;
