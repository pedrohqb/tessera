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

// SAFETY: Kept only owns its boxes; the raw pointer points into its own box.
unsafe impl Send for Kept {}

static ORIGINAL: OnceLock<Vec<usize>> = OnceLock::new();
static KEPT: OnceLock<Mutex<HashMap<(usize, c_ulong), Kept>>> = OnceLock::new();

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
