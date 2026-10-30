//! TESSERA_REMOTE_TRACE=1: log every PKCS#11 call `tessera remote` passes to
//! the driver (to stderr, the driver's log), with its arguments, result and
//! duration. For comparing what a driver receives through p11-kit with what
//! an app sends it directly.
//!
//! Each function is replaced by a trampoline that takes six integer
//! arguments and forwards them: on x86-64 every PKCS#11 argument is an
//! integer or a pointer passed in a register, for the functions with at
//! most six. C_GenerateKeyPair and C_UnwrapKey take eight and are left as
//! they are.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::time::Instant;

type CkUlong = std::ffi::c_ulong;
type Fn6 = unsafe extern "C" fn(usize, usize, usize, usize, usize, usize) -> CkUlong;

const NAMES: [&str; 68] = [
    "C_Initialize",
    "C_Finalize",
    "C_GetInfo",
    "C_GetFunctionList",
    "C_GetSlotList",
    "C_GetSlotInfo",
    "C_GetTokenInfo",
    "C_GetMechanismList",
    "C_GetMechanismInfo",
    "C_InitToken",
    "C_InitPIN",
    "C_SetPIN",
    "C_OpenSession",
    "C_CloseSession",
    "C_CloseAllSessions",
    "C_GetSessionInfo",
    "C_GetOperationState",
    "C_SetOperationState",
    "C_Login",
    "C_Logout",
    "C_CreateObject",
    "C_CopyObject",
    "C_DestroyObject",
    "C_GetObjectSize",
    "C_GetAttributeValue",
    "C_SetAttributeValue",
    "C_FindObjectsInit",
    "C_FindObjects",
    "C_FindObjectsFinal",
    "C_EncryptInit",
    "C_Encrypt",
    "C_EncryptUpdate",
    "C_EncryptFinal",
    "C_DecryptInit",
    "C_Decrypt",
    "C_DecryptUpdate",
    "C_DecryptFinal",
    "C_DigestInit",
    "C_Digest",
    "C_DigestUpdate",
    "C_DigestKey",
    "C_DigestFinal",
    "C_SignInit",
    "C_Sign",
    "C_SignUpdate",
    "C_SignFinal",
    "C_SignRecoverInit",
    "C_SignRecover",
    "C_VerifyInit",
    "C_Verify",
    "C_VerifyUpdate",
    "C_VerifyFinal",
    "C_VerifyRecoverInit",
    "C_VerifyRecover",
    "C_DigestEncryptUpdate",
    "C_DecryptDigestUpdate",
    "C_SignEncryptUpdate",
    "C_DecryptVerifyUpdate",
    "C_GenerateKey",
    "C_GenerateKeyPair",
    "C_WrapKey",
    "C_UnwrapKey",
    "C_DeriveKey",
    "C_SeedRandom",
    "C_GenerateRandom",
    "C_GetFunctionStatus",
    "C_CancelFunction",
    "C_WaitForSlotEvent",
];
const EIGHT_ARGS: [usize; 2] = [59, 61];

static ORIGINAL: OnceLock<Vec<usize>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

extern "C" {
    fn gettid() -> i32;
}

fn hex(p: usize, n: usize) -> String {
    if p == 0 {
        return "NULL".into();
    }
    // SAFETY: the caller passed a buffer of n bytes at p.
    let b = unsafe { std::slice::from_raw_parts(p as *const u8, n.min(64)) };
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A readable account of the arguments of the calls worth decoding.
///
/// # Safety
///
/// `a` are the arguments of a call to function `i` of a CK_FUNCTION_LIST
/// that has just returned, as `tramp` passes them. Each decoded case reads
/// only what that function's PKCS#11 signature says is a valid pointer for
/// the call (CK_C_INITIALIZE_ARGS, a CK_ULONG count, a CK_ATTRIBUTE array of
/// the given length, a CK_MECHANISM, a found-object array of *count
/// entries), and only when the pointer is not null. At most 16 entries and
/// 64 bytes are read. This runs only with TESSERA_REMOTE_TRACE=1.
unsafe fn describe(i: usize, a: [usize; 6]) -> String {
    let ul = |p: usize| {
        if p == 0 {
            "NULL".to_string()
        } else {
            (*(p as *const CkUlong)).to_string()
        }
    };
    match i {
        0 => {
            if a[0] == 0 {
                "args=NULL".into()
            } else {
                let w = a[0] as *const usize;
                format!(
                    "flags={:#x} mutexfns={} pReserved={:#x}",
                    *w.add(4),
                    (0..4).filter(|k| *w.add(*k) != 0).count(),
                    *w.add(5)
                )
            }
        }
        4 => format!(
            "tokenPresent={} pList={} count={}",
            a[0] & 0xff,
            if a[1] == 0 { "NULL" } else { "buf" },
            ul(a[2])
        ),
        12 => format!(
            "slot={} flags={:#x} app={:#x} notify={:#x}",
            a[0], a[1], a[2], a[3]
        ),
        24 | 26 => {
            let (obj, attrs, n) = if i == 24 {
                (format!(" obj={}", a[1]), a[2], a[3])
            } else {
                (String::new(), a[1], a[2])
            };
            let attrs = attrs as *const [usize; 3];
            let list: Vec<String> = (0..n.min(16))
                .map(|k| {
                    let at = *attrs.add(k);
                    if at[1] == 0 {
                        format!("{:#x}(len)", at[0])
                    } else {
                        format!("{:#x}={}", at[0], hex(at[1], at[2]))
                    }
                })
                .collect();
            format!("session={}{obj} [{}]", a[0], list.join(" "))
        }
        42 | 29 | 33 | 37 | 48 => {
            let m = a[1] as *const usize;
            let (mech, param, plen) = if m.is_null() {
                (0, 0, 0)
            } else {
                (*m, *m.add(1), *m.add(2))
            };
            format!(
                "session={} mech={mech:#x} param={} key={}",
                a[0],
                hex(param, plen),
                a[2]
            )
        }
        43 => format!(
            "session={} data={} sig={} sigLen={}",
            a[0],
            hex(a[1], a[2]),
            if a[3] == 0 { "NULL" } else { "buf" },
            ul(a[4])
        ),
        18 => format!("session={} user={} pinLen={}", a[0], a[1], a[3]),
        27 => {
            let n = if a[3] == 0 {
                0
            } else {
                *(a[3] as *const CkUlong) as usize
            };
            let objs: Vec<String> = (0..n.min(16))
                .map(|k| (*(a[1] as *const CkUlong).add(k)).to_string())
                .collect();
            format!("session={} max={} found=[{}]", a[0], a[2], objs.join(" "))
        }
        _ => format!("{:#x} {:#x} {:#x} {:#x}", a[0], a[1], a[2], a[3]),
    }
}

/// Function I of the traced list: calls the driver's own and logs the call.
///
/// # Safety
///
/// Called by p11-kit with the arguments of function I. Every PKCS#11
/// function handed to a trampoline takes at most six arguments, each an
/// integer or a pointer (the two with eight are left out, see EIGHT_ARGS),
/// so on x86_64 they all arrive in registers and passing six usize values
/// forwards them unchanged; the extra ones are ignored by the callee.
/// ORIGINAL was set by `wrap` before the list was handed out.
unsafe extern "C" fn tramp<const I: usize>(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
) -> CkUlong {
    let orig: Fn6 = std::mem::transmute::<usize, Fn6>(ORIGINAL.get().unwrap()[I]);
    let args = [a, b, c, d, e, f];
    let t = Instant::now();
    let rv = orig(a, b, c, d, e, f);
    // Described after the call, so outputs (lengths, handles, values) show.
    let after = describe(I, args);
    let since = START.get().map(|s| s.elapsed().as_millis()).unwrap_or(0);
    eprintln!(
        "trace {since:>7}ms tid={} {}({after}) = {rv:#x} [{} ms]",
        gettid(),
        NAMES[I],
        t.elapsed().as_millis()
    );
    rv
}

macro_rules! table {
    ($($i:literal)*) => { [$(tramp::<$i> as *const () as usize),*] };
}

/// A copy of `module` (a CK_FUNCTION_LIST) whose functions log each call.
///
/// # Safety
///
/// As stable::wrap: `module` points to a valid CK_FUNCTION_LIST (2.x, 68
/// functions) or CK_FUNCTION_LIST_3_0 (92), whose first word holds the
/// version and the others function pointers. Only the first 68 are traced.
/// The copy is leaked on purpose, and wrap is called once per process.
pub unsafe fn wrap(module: *mut c_void) -> *mut c_void {
    let _ = START.set(Instant::now());
    let version = *(module as *const u8);
    let n = if version >= 3 { 92 } else { 68 };
    let words = 1 + n;
    let mut copy = vec![0usize; words];
    std::ptr::copy_nonoverlapping(module as *const usize, copy.as_mut_ptr(), words);
    let _ = ORIGINAL.set(copy[1..69].to_vec());
    let tramps: [usize; 68] = table!(
        0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29
        30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56
        57 58 59 60 61 62 63 64 65 66 67
    );
    for i in 0..68 {
        if copy[1 + i] != 0 && !EIGHT_ARGS.contains(&i) && i != 3 {
            copy[1 + i] = tramps[i];
        }
    }
    eprintln!("trace: wrapping {n} functions (PKCS#11 {version}.x list)");
    Box::leak(copy.into_boxed_slice()).as_mut_ptr() as *mut c_void
}
