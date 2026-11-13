//! TESSERA_TOKEN_SERIAL: the serial number `tessera remote` reports for the
//! driver's tokens, in place of the driver's own.
//!
//! SerproID reports "1", the same serial as the system's p11-kit trust
//! store ("System Trust", "Default Trust"). Apps that tell tokens apart by
//! serial then take SerproID for one of those: PJeOffice (signer4j) keeps
//! one device per serial, and on the system's p11-kit server the trust
//! store comes first, so SerproID was dropped there. Tessera sets
//! TESSERA_TOKEN_SERIAL=SERPROID for it (drivers.rs); a driver.toml can set
//! it in `[env]` for another driver.
//!
//! `wrap` returns a copy of the function list whose C_GetTokenInfo writes
//! that serial, padded with spaces to 16 bytes as PKCS#11 wants, over the
//! driver's.

use std::ffi::{c_ulong, c_void};
use std::sync::OnceLock;

type GetTokenInfo = unsafe extern "C" fn(c_ulong, *mut u8) -> c_ulong;

/// Index of C_GetTokenInfo among the functions of CK_FUNCTION_LIST.
const GET_TOKEN_INFO: usize = 6;
/// Where serialNumber (16 bytes) sits in CK_TOKEN_INFO: after label (32),
/// manufacturerID (32) and model (16).
const SERIAL_OFFSET: usize = 80;

static ORIGINAL: OnceLock<usize> = OnceLock::new();
static SERIAL: OnceLock<[u8; 16]> = OnceLock::new();

/// `serial` as a CK_TOKEN_INFO serialNumber: at most 16 bytes, padded with
/// spaces. None when it is empty or not printable ASCII.
pub fn field(serial: &str) -> Option<[u8; 16]> {
    let s = serial.trim();
    if s.is_empty() || !s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return None;
    }
    let mut out = [b' '; 16];
    for (o, b) in out.iter_mut().zip(s.bytes()) {
        *o = b;
    }
    Some(out)
}

/// C_GetTokenInfo, then the serial over the driver's.
///
/// # Safety
///
/// Called by p11-kit as the driver's C_GetTokenInfo, with its arguments:
/// `info` is null or points to a CK_TOKEN_INFO the caller owns. ORIGINAL
/// and SERIAL were set by `wrap` before this list was handed out; ORIGINAL
/// is the driver's own C_GetTokenInfo. The serial is written only when the
/// driver returned CKR_OK, so it filled the structure.
unsafe extern "C" fn get_token_info(slot: c_ulong, info: *mut u8) -> c_ulong {
    let orig: GetTokenInfo = std::mem::transmute::<usize, GetTokenInfo>(*ORIGINAL.get().unwrap());
    let rv = orig(slot, info);
    if rv == 0 && !info.is_null() {
        if let Some(serial) = SERIAL.get() {
            std::ptr::copy_nonoverlapping(serial.as_ptr(), info.add(SERIAL_OFFSET), 16);
        }
    }
    rv
}

/// A copy of `module` whose tokens report `serial`.
///
/// # Safety
///
/// As stable::wrap: `module` points to a valid CK_FUNCTION_LIST (2.x) or
/// CK_FUNCTION_LIST_3_0, whose first word holds the CK_VERSION and each of
/// the following words a function pointer. The copy is leaked on purpose:
/// p11-kit uses it until the process ends. Call it once per process.
pub unsafe fn wrap(module: *mut c_void, serial: [u8; 16]) -> *mut c_void {
    let version = *(module as *const u8);
    let words = 1 + if version >= 3 { 92 } else { 68 };
    let mut copy = vec![0usize; words];
    std::ptr::copy_nonoverlapping(module as *const usize, copy.as_mut_ptr(), words);
    if copy[1 + GET_TOKEN_INFO] == 0 {
        return module;
    }
    let _ = ORIGINAL.set(copy[1 + GET_TOKEN_INFO]);
    let _ = SERIAL.set(serial);
    copy[1 + GET_TOKEN_INFO] = get_token_info as *const () as usize;
    Box::leak(copy.into_boxed_slice()).as_mut_ptr() as *mut c_void
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_serial_is_padded_printable_ascii() {
        assert_eq!(&field("SERPROID").unwrap(), b"SERPROID        ");
        assert_eq!(&field("0123456789abcdefXYZ").unwrap(), b"0123456789abcdef");
        assert_eq!(field("  "), None);
        assert_eq!(field("série"), None);
    }

    /// p11-kit's trust module, whose tokens report serial "1", reports the
    /// new one once wrapped.
    #[test]
    fn the_tokens_report_the_new_serial() {
        let module = "/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-trust.so";
        if !std::path::Path::new(module).exists() {
            if std::env::var("TESSERA_TEST_REQUIRE").as_deref() == Ok("1") {
                panic!("no p11-kit trust module (TESSERA_TEST_REQUIRE=1)");
            }
            return;
        }
        type Fl = unsafe extern "C" fn(*mut *mut c_void) -> c_ulong;
        type Init = unsafe extern "C" fn(*mut c_void) -> c_ulong;
        type Slots = unsafe extern "C" fn(u8, *mut c_ulong, *mut c_ulong) -> c_ulong;
        // SAFETY: the trust module is a PKCS#11 library; its function list
        // is used through the PKCS#11 signatures above, with buffers sized
        // as the calls require (CK_TOKEN_INFO is 208 bytes on x86-64).
        unsafe {
            let lib = libloading::Library::new(module).unwrap();
            let fl: libloading::Symbol<Fl> = lib.get(b"C_GetFunctionList\0").unwrap();
            let mut list: *mut c_void = std::ptr::null_mut();
            assert_eq!(fl(&mut list), 0);
            let wrapped = wrap(list, field("SERPROID").unwrap()) as *const usize;
            let f = |i: usize| *wrapped.add(1 + i);
            let init: Init = std::mem::transmute::<usize, Init>(f(0));
            let slots: Slots = std::mem::transmute::<usize, Slots>(f(4));
            let info: GetTokenInfo = std::mem::transmute::<usize, GetTokenInfo>(f(6));
            assert_eq!(init(std::ptr::null_mut()), 0);
            let mut n: c_ulong = 0;
            assert_eq!(slots(1, std::ptr::null_mut(), &mut n), 0);
            let mut ids = vec![0 as c_ulong; n as usize];
            assert_eq!(slots(1, ids.as_mut_ptr(), &mut n), 0);
            assert!(n > 0, "the trust module has no token");
            let mut buf = [0u8; 256];
            assert_eq!(info(ids[0], buf.as_mut_ptr()), 0);
            assert_eq!(&buf[80..96], b"SERPROID        ");
            assert_eq!(&buf[64..77], b"p11-kit-trust", "the rest is the driver's");
        }
    }
}
