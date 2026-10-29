//! `tessera remote --provider MODULE pkcs11:...`: Tessera's own stand-in for
//! `p11-kit remote`, run by `p11-kit server` for each connection (through
//! P11_KIT_PRIVATEDIR, see server.rs). It does what `p11-kit remote` does,
//! through the same libp11-kit calls, on stdin/stdout.
//!
//! Why: some drivers identify the program that loaded them. SerproID sends
//! it with each signing request, and its server refuses
//! `/usr/libexec/p11-kit/p11-kit-remote`; with this, the program is Tessera.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;
use std::{fs, os::unix::fs::PermissionsExt};

use crate::paths;

type ModuleLoad = unsafe extern "C" fn(*const c_char, c_int) -> *mut c_void;
type ModuleRelease = unsafe extern "C" fn(*mut c_void);
type ServeTokens =
    unsafe extern "C" fn(*const *const c_char, usize, *mut c_void, c_int, c_int) -> c_int;

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
        let c_tokens: Vec<CString> = tokens
            .iter()
            .filter_map(|t| CString::new(*t).ok())
            .collect();
        let ptrs: Vec<*const c_char> = c_tokens.iter().map(|c| c.as_ptr()).collect();
        let rv = serve(ptrs.as_ptr(), ptrs.len(), module, 0, 1);
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
