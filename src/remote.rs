//! `tessera remote --provider MODULE pkcs11:...`: Tessera's own stand-in for
//! `p11-kit remote`, run by `p11-kit server` for each connection (through
//! P11_KIT_PRIVATEDIR, see server.rs). It does what `p11-kit remote` does,
//! through the same libp11-kit calls, on stdin/stdout.
//!
//! Why: some drivers identify the program that loaded them. SerproID sends
//! it with each signing request, and its server refuses
//! `/usr/libexec/p11-kit/p11-kit-remote`; with this, the program is Tessera.
//!
//! With TESSERA_SIGN_LENGTH_FIRST=1 (set by a driver's `[env]`), every
//! C_Sign with an output buffer is preceded by a length query, C_Sign with
//! no buffer. SerproID signs only when asked in those two steps: a single
//! C_Sign, which is what NSS (Firefox, Papers) sends, makes it push request
//! after request to the phone, and approving them fails. Inside Firefox the
//! driver is loaded directly and this never shows; through p11-kit it does.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::{fs, os::unix::fs::PermissionsExt};

use crate::paths;

type ModuleLoad = unsafe extern "C" fn(*const c_char, c_int) -> *mut c_void;
type ModuleRelease = unsafe extern "C" fn(*mut c_void);
type ServeTokens =
    unsafe extern "C" fn(*const *const c_char, usize, *mut c_void, c_int, c_int) -> c_int;

type CkUlong = std::ffi::c_ulong;
type SignFn = unsafe extern "C" fn(CkUlong, *mut u8, CkUlong, *mut u8, *mut CkUlong) -> CkUlong;

const CKR_OK: CkUlong = 0;
const CKR_BUFFER_TOO_SMALL: CkUlong = 0x150;
/// Position of C_Sign in CK_FUNCTION_LIST, after CK_VERSION (padded to a
/// pointer).
const C_SIGN_INDEX: usize = 43;
/// Number of functions in CK_FUNCTION_LIST (2.x) and CK_FUNCTION_LIST_3_0.
const FUNCTIONS_V2: usize = 68;
const FUNCTIONS_V3: usize = 92;

static DRIVER_SIGN: OnceLock<SignFn> = OnceLock::new();

/// C_Sign that asks the driver for the length first (see the module docs).
unsafe extern "C" fn sign_length_first(
    session: CkUlong,
    data: *mut u8,
    data_len: CkUlong,
    signature: *mut u8,
    signature_len: *mut CkUlong,
) -> CkUlong {
    let Some(sign) = DRIVER_SIGN.get() else {
        return 0x6; // CKR_FUNCTION_FAILED
    };
    if !signature.is_null() && !signature_len.is_null() {
        if std::env::var_os("TESSERA_DEBUG_SIGN").is_some() {
            eprintln!("tessera remote: length query before C_Sign");
        }
        let mut needed: CkUlong = 0;
        let rv = sign(session, data, data_len, std::ptr::null_mut(), &mut needed);
        if rv != CKR_OK {
            return rv;
        }
        if needed > *signature_len {
            *signature_len = needed;
            return CKR_BUFFER_TOO_SMALL;
        }
    }
    sign(session, data, data_len, signature, signature_len)
}

/// A copy of the driver's function list with C_Sign replaced. Leaked: it
/// lives as long as the process.
unsafe fn with_sign_length_first(module: *mut c_void) -> *mut c_void {
    let version = *(module as *const u8);
    let n = if version >= 3 {
        FUNCTIONS_V3
    } else {
        FUNCTIONS_V2
    };
    let words = 1 + n; // CK_VERSION padded to one pointer, then functions
    let mut copy = vec![0usize; words];
    std::ptr::copy_nonoverlapping(module as *const usize, copy.as_mut_ptr(), words);
    let slot = 1 + C_SIGN_INDEX;
    let original = copy[slot];
    if original == 0 {
        return module;
    }
    let _ = DRIVER_SIGN.set(std::mem::transmute::<usize, SignFn>(original));
    copy[slot] = sign_length_first as *const () as usize;
    Box::leak(copy.into_boxed_slice()).as_mut_ptr() as *mut c_void
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
        let module = load(c_provider.as_ptr(), 0);
        if module.is_null() {
            eprintln!("tessera remote: could not load {provider}");
            return 1;
        }
        let served = if std::env::var("TESSERA_SIGN_LENGTH_FIRST").as_deref() == Ok("1") {
            with_sign_length_first(module)
        } else {
            module
        };
        let c_tokens: Vec<CString> = tokens
            .iter()
            .filter_map(|t| CString::new(*t).ok())
            .collect();
        let ptrs: Vec<*const c_char> = c_tokens.iter().map(|c| c.as_ptr()).collect();
        let rv = serve(ptrs.as_ptr(), ptrs.len(), served, 0, 1);
        release(module);
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
    Ok(dir)
}
