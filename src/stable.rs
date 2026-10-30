//! Mechanism parameters that outlive the call that passed them.
//!
//! PKCS#11 lets a driver read the CK_MECHANISM it gets in C_SignInit (and
//! the other *Init calls) only during that call. SerproID keeps the pointer
//! and reads the parameters again in C_Sign. An app keeps its parameters
//! alive, so this never shows when the app loads the driver itself; through
//! p11-kit they live in the RPC message, which is reused as soon as
//! C_SignInit returns, so SerproID signs with whatever is there by then.
//! For RSA-PSS (TLS 1.3), the request it sends to the cloud is wrong and
//! approving it on the phone fails ("Ops!").
//!
//! `wrap` returns a copy of the driver's function list whose *Init calls
//! hand the driver a copy of the mechanism kept until the next *Init of the
//! same kind on the same session.

use std::collections::HashMap;
use std::ffi::{c_ulong, c_void};
use std::sync::{Mutex, OnceLock};

type Init = unsafe extern "C" fn(c_ulong, *const c_void, c_ulong) -> c_ulong;

/// Index in CK_FUNCTION_LIST of C_EncryptInit, C_DecryptInit, C_DigestInit,
/// C_SignInit, C_SignRecoverInit, C_VerifyInit, C_VerifyRecoverInit.
const INITS: [usize; 7] = [29, 33, 37, 42, 46, 48, 52];

#[repr(C)]
struct Mechanism {
    mechanism: c_ulong,
    parameter: *const u8,
    parameter_len: c_ulong,
}

struct Kept {
    mechanism: Box<Mechanism>,
    _parameter: Box<[u8]>,
}

// SAFETY: Kept only owns its two boxes. The raw pointer inside `mechanism`
// points into `_parameter`, which Kept also owns and never changes, so moving
// a Kept to another thread moves what the pointer points to with it. Kept is
// only reached through the Mutex in KEPT.
unsafe impl Send for Kept {}

static ORIGINAL: OnceLock<Vec<usize>> = OnceLock::new();
static KEPT: OnceLock<Mutex<HashMap<(usize, c_ulong), Kept>>> = OnceLock::new();

/// A copy of the mechanism `m` and its parameter, owned by the Kept.
///
/// # Safety
///
/// `m` is null or points to a CK_MECHANISM valid for this call, whose
/// `parameter` is null or points to `parameter_len` readable bytes: what
/// PKCS#11 requires of the caller of every *Init function.
unsafe fn keep(m: *const Mechanism) -> Option<Kept> {
    if m.is_null() {
        return None;
    }
    let m = &*m;
    let len = m.parameter_len as usize;
    let parameter: Box<[u8]> = if m.parameter.is_null() || len == 0 {
        Box::new([])
    } else {
        std::slice::from_raw_parts(m.parameter, len).into()
    };
    let mechanism = Box::new(Mechanism {
        mechanism: m.mechanism,
        parameter: if m.parameter.is_null() {
            std::ptr::null()
        } else {
            parameter.as_ptr()
        },
        parameter_len: m.parameter_len,
    });
    Some(Kept {
        mechanism,
        _parameter: parameter,
    })
}

/// The *Init function at index I of the function list, handing the driver a
/// kept copy of the mechanism.
///
/// # Safety
///
/// Called by p11-kit as the driver's *Init function, with that function's
/// arguments. ORIGINAL was set by `wrap` before this list was handed out, and
/// its entry I is the driver's own function, which has the *Init signature
/// (CK_SESSION_HANDLE, CK_MECHANISM_PTR, CK_OBJECT_HANDLE) declared as Init.
/// The copy handed to the driver stays in KEPT until the next *Init of the
/// same kind on the same session, so it is alive whenever the driver reads it.
unsafe extern "C" fn init<const I: usize>(
    session: c_ulong,
    mechanism: *const c_void,
    key: c_ulong,
) -> c_ulong {
    let orig: Init = std::mem::transmute::<usize, Init>(ORIGINAL.get().unwrap()[I]);
    let Some(kept) = keep(mechanism as *const Mechanism) else {
        return orig(session, mechanism, key);
    };
    let ptr = &*kept.mechanism as *const Mechanism as *const c_void;
    let rv = orig(session, ptr, key);
    // Replace the previous copy only now: until this call the driver may
    // still have been using it.
    if let Ok(mut map) = KEPT.get_or_init(Default::default).lock() {
        map.insert((I, session), kept);
    }
    rv
}

macro_rules! init_fn {
    ($i:literal) => {
        init::<$i> as *const () as usize
    };
}

/// A copy of `module` (a CK_FUNCTION_LIST) with stable mechanism copies.
///
/// # Safety
///
/// `module` points to a valid CK_FUNCTION_LIST (version 2.x, 68 functions)
/// or CK_FUNCTION_LIST_3_0 (92 functions), as C_GetFunctionList or p11-kit
/// return. Its first word holds the CK_VERSION, whose first byte is the
/// major version; each of the following words is a function pointer. The
/// copy is leaked on purpose: p11-kit uses it until the process ends. Call
/// it once per process (ORIGINAL keeps only the first module's functions).
pub unsafe fn wrap(module: *mut c_void) -> *mut c_void {
    let version = *(module as *const u8);
    let n = if version >= 3 { 92 } else { 68 };
    let words = 1 + n;
    let mut copy = vec![0usize; words];
    std::ptr::copy_nonoverlapping(module as *const usize, copy.as_mut_ptr(), words);
    let _ = ORIGINAL.set(copy[1..69].to_vec());
    let fns = [
        init_fn!(29),
        init_fn!(33),
        init_fn!(37),
        init_fn!(42),
        init_fn!(46),
        init_fn!(48),
        init_fn!(52),
    ];
    for (k, i) in INITS.iter().enumerate() {
        if copy[1 + i] != 0 {
            copy[1 + i] = fns[k];
        }
    }
    Box::leak(copy.into_boxed_slice()).as_mut_ptr() as *mut c_void
}
