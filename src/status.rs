//! What the user sees: card readers (through pcscd) and tokens with their
//! certificates, read through each driver's server socket by a child process
//! (`tessera list-tokens <socket>`), the way client apps read them.

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::object::{Attribute, AttributeType, ObjectClass};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long one driver may take to list its tokens.
const LIST_TIMEOUT: Duration = Duration::from_secs(10);

use crate::paths;

#[derive(Debug, Clone)]
pub struct Reader {
    pub name: String,
    pub card_present: bool,
    /// The card's ATR, when there is a card.
    pub atr: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum Readers {
    Ok(Vec<Reader>),
    /// pcscd is not reachable from the sandbox.
    NoService(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    /// "YYYY-MM-DD"
    pub not_after: String,
    pub expired: bool,
    /// The certificate's serial number (hex), which with the issuer tells
    /// it apart from any other.
    #[serde(default)]
    pub serial: String,
    /// A CA's certificate (the chain some tokens carry), shared by many
    /// tokens.
    #[serde(default)]
    pub ca: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Name of the driver that serves it.
    #[serde(default)]
    pub driver: String,
    pub label: String,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub certificates: Vec<Certificate>,
    /// The slot's position among all slots, with or without a token
    /// (C_GetSlotList(CK_FALSE)), which is what Java's SunPKCS11 counts in
    /// `slotListIndex` (see jsignpdf.rs).
    #[serde(default)]
    pub slot_index: usize,
    /// The slot's description, which for most drivers (OpenSC, SafeNet)
    /// names the card reader the token is in.
    #[serde(default)]
    pub slot: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TokenList {
    tokens: Vec<Token>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub readers: Readers,
    pub tokens: Vec<Token>,
    /// Drivers whose server could not be read: "driver: error".
    pub errors: Vec<String>,
    /// Ids of the drivers that answered.
    pub ok: Vec<String>,
    /// Ids of the drivers that did not answer in time.
    pub timed_out: Vec<String>,
}

enum ReadError {
    Timeout,
    Failed(String),
}
use ReadError::{Failed, Timeout};

pub fn readers() -> Readers {
    let ctx = match pcsc::Context::establish(pcsc::Scope::User) {
        Ok(c) => c,
        Err(e) => return Readers::NoService(e.to_string()),
    };
    let names = match ctx.list_readers_owned() {
        Ok(n) => n,
        Err(pcsc::Error::NoReadersAvailable) => return Readers::Ok(vec![]),
        Err(e) => return Readers::NoService(e.to_string()),
    };
    let mut states: Vec<pcsc::ReaderState> = names
        .iter()
        .map(|n| pcsc::ReaderState::new(n.clone(), pcsc::State::UNAWARE))
        .collect();
    let present: Vec<(bool, Vec<u8>)> = if ctx
        .get_status_change(std::time::Duration::from_millis(0), &mut states)
        .is_ok()
    {
        states
            .iter()
            .map(|s| {
                let card = s.event_state().contains(pcsc::State::PRESENT);
                (card, if card { s.atr().to_vec() } else { vec![] })
            })
            .collect()
    } else {
        vec![(false, vec![]); names.len()]
    };
    let list = names
        .iter()
        .zip(present)
        .map(|(n, (card_present, atr))| Reader {
            name: n.to_string_lossy().into_owned(),
            card_present,
            atr,
        })
        .collect();
    Readers::Ok(list)
}

fn client_module() -> String {
    std::env::var("TESSERA_CLIENT_MODULE").unwrap_or_else(|_| paths::CLIENT_MODULE.to_string())
}

fn name_field(name: &x509_parser::x509::X509Name) -> String {
    name.iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok().map(str::to_string))
        .unwrap_or_else(|| name.to_string())
}

fn parse_cert(label: String, der: &[u8]) -> Option<Certificate> {
    let (_, cert) = x509_parser::parse_x509_certificate(der).ok()?;
    let subject = name_field(cert.subject());
    let not_after = cert.validity().not_after;
    let date = not_after.to_datetime().date();
    Some(Certificate {
        subject: if subject.is_empty() { label } else { subject },
        issuer: name_field(cert.issuer()),
        not_after: format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            u8::from(date.month()),
            date.day()
        ),
        expired: !cert.validity().is_valid(),
        serial: cert.raw_serial_as_string(),
        ca: cert.is_ca(),
    })
}

/// The first fields of CK_FUNCTION_LIST, up to C_GetSlotList.
#[repr(C)]
struct FunctionListHead {
    version: [u8; 2],
    initialize: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> std::ffi::c_ulong>,
    finalize: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> std::ffi::c_ulong>,
    get_info: usize,
    get_function_list: usize,
    get_slot_list: Option<
        unsafe extern "C" fn(
            u8,
            *mut std::ffi::c_ulong,
            *mut std::ffi::c_ulong,
        ) -> std::ffi::c_ulong,
    >,
}

/// How many slots the module lists (with a token only, or all), asked once
/// with no buffer. Needed because cryptoki asks a second time with a
/// (non-null) buffer even when the count is 0, and through p11-kit's RPC
/// that answers CKR_BUFFER_TOO_SMALL, which cryptoki retries without end:
/// the listing spins until its time limit, and supervision takes the driver
/// for hung. OpenSC with no reader connected (an ePass2003 unplugged) has
/// no slot at all.
///
/// p11-kit-client.so hands out a new, uninitialized instance on each
/// C_GetFunctionList (CKR_CRYPTOKI_NOT_INITIALIZED): that one is
/// initialized for the question and finalized after it. A driver loaded
/// directly hands out the copy cryptoki already initialized.
fn slot_count(module: &str, token_present: bool) -> Option<u64> {
    type GetFunctionList = unsafe extern "C" fn(*mut *const FunctionListHead) -> std::ffi::c_ulong;
    // SAFETY: the module is the one cryptoki has already loaded and
    // initialized in this process (dlopen of the same path gives the same
    // copy, which stays loaded while `lib` lives). C_GetFunctionList fills
    // in a pointer to the module's own CK_FUNCTION_LIST, whose layout starts
    // with FunctionListHead (CK_VERSION, then function pointers, in this
    // order, per PKCS#11). C_GetSlotList with a null list only writes the
    // count. C_Initialize with no arguments and C_Finalize are only called
    // on an instance that said it is not initialized, so cryptoki's own is
    // left alone.
    unsafe {
        let lib = libloading::Library::new(module).ok()?;
        let get: libloading::Symbol<GetFunctionList> = lib.get(b"C_GetFunctionList\0").ok()?;
        let mut list: *const FunctionListHead = std::ptr::null();
        if get(&mut list) != 0 || list.is_null() {
            return None;
        }
        const NOT_INITIALIZED: std::ffi::c_ulong = 0x190;
        let f = (*list).get_slot_list?;
        let mut n: std::ffi::c_ulong = 0;
        let mut rv = f(u8::from(token_present), std::ptr::null_mut(), &mut n);
        if rv == NOT_INITIALIZED {
            let (init, fini) = ((*list).initialize?, (*list).finalize?);
            if init(std::ptr::null_mut()) != 0 {
                return None;
            }
            rv = f(u8::from(token_present), std::ptr::null_mut(), &mut n);
            fini(std::ptr::null_mut());
        }
        (rv == 0).then_some(n as u64)
    }
}

/// Set by the parent for a listing that may find no slot (one of
/// Tessera's own servers): ask for the count first. Through p11-kit-client
/// that takes a second connection, so a second load of the driver, which
/// the system's server (always with slots) is spared.
const COUNT_FIRST: &str = "TESSERA_COUNT_FIRST";

/// The module's slots (all, or with a token), without cryptoki's endless
/// retry on a module with none (see slot_count). `count_first` false skips
/// the check.
fn slots(
    ctx: &Pkcs11,
    module: &str,
    token_present: bool,
    count_first: bool,
) -> cryptoki::error::Result<Vec<cryptoki::slot::Slot>> {
    if count_first && slot_count(module, token_present) == Some(0) {
        return Ok(vec![]);
    }
    if token_present {
        ctx.get_slots_with_token()
    } else {
        ctx.get_all_slots()
    }
}

/// Token fields are fixed-size, padded with spaces, and some drivers
/// (SerproID) pad with NUL bytes instead.
fn clean(s: &str) -> String {
    s.trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string()
}

/// `tessera probe-module MODULE`: load a driver directly, without p11-kit,
/// and report each step, to tell a driver error from a crash.
pub fn probe_module(module: &str) -> i32 {
    eprintln!("probe: loading {module}");
    let ctx = match Pkcs11::new(module) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("probe: load failed: {e}");
            return 2;
        }
    };
    for (what, flags) in [
        (
            "C_Initialize(OS_LOCKING_OK)",
            CInitializeFlags::OS_LOCKING_OK,
        ),
        ("C_Initialize(no flags)", CInitializeFlags::empty()),
    ] {
        eprintln!("probe: {what}...");
        match ctx.initialize(CInitializeArgs::new(flags)) {
            Ok(()) => {
                eprintln!("probe: {what} ok");
                match ctx.get_library_info() {
                    Ok(i) => eprintln!("probe: library: {i:?}"),
                    Err(e) => eprintln!("probe: C_GetInfo: {e}"),
                }
                match slots(&ctx, module, false, true) {
                    Ok(slots) => {
                        eprintln!("probe: {} slot(s)", slots.len());
                        for s in slots {
                            match ctx.get_token_info(s) {
                                Ok(t) => eprintln!(
                                    "probe:   token {:?} ({})",
                                    t.label().trim(),
                                    t.model().trim()
                                ),
                                Err(e) => eprintln!("probe:   slot without token: {e}"),
                            }
                            if let Ok(mechs) = ctx.get_mechanism_list(s) {
                                eprintln!("probe:   {} mechanism(s):", mechs.len());
                                for m in mechs {
                                    match ctx.get_mechanism_info(s, m) {
                                        Ok(i) => eprintln!("probe:     {m} {i:?}"),
                                        Err(_) => eprintln!("probe:     {m}"),
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("probe: C_GetSlotList: {e}"),
                }
                return 0;
            }
            Err(e) => eprintln!("probe: {what} failed: {e}"),
        }
    }
    1
}

/// `tessera probe-sign MODULE`: sign a short test text with the first
/// private key found, to see where a signature fails. With a cloud
/// certificate this sends a real authorization request. If the token asks
/// for a PIN, it is read from TESSERA_PIN.
pub fn probe_sign(module: &str) -> i32 {
    use cryptoki::mechanism::Mechanism;
    use cryptoki::session::UserType;
    use cryptoki::types::AuthPin;
    let ctx = match Pkcs11::new(module) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sign: load failed: {e}");
            return 2;
        }
    };
    if let Err(e) = ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
        eprintln!("sign: C_Initialize failed: {e}");
        return 2;
    }
    for slot in slots(&ctx, module, true, true).unwrap_or_default() {
        let Ok(info) = ctx.get_token_info(slot) else {
            continue;
        };
        eprintln!(
            "sign: token {:?}, login required: {}",
            clean(info.label()),
            info.login_required()
        );
        let session = match ctx
            .open_rw_session(slot)
            .or_else(|_| ctx.open_ro_session(slot))
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sign: C_OpenSession failed: {e}");
                continue;
            }
        };
        if info.login_required() {
            match std::env::var("TESSERA_PIN") {
                Ok(pin) => {
                    if let Err(e) = session.login(UserType::User, Some(&AuthPin::from(pin))) {
                        eprintln!("sign: C_Login failed: {e}");
                        continue;
                    }
                    eprintln!("sign: logged in");
                }
                Err(_) => eprintln!("sign: no TESSERA_PIN; trying without login"),
            }
        }
        let keys = session
            .find_objects(&[Attribute::Class(ObjectClass::PRIVATE_KEY)])
            .unwrap_or_default();
        eprintln!("sign: {} private key(s)", keys.len());
        let Some(key) = keys.first().copied() else {
            continue;
        };
        let mechs = ctx.get_mechanism_list(slot).unwrap_or_default();
        eprintln!("sign: mechanisms: {mechs:?}");
        // SHA-256 of "Tessera signing test", the way TLS clients sign: the
        // hash computed by the application, the token only signs it.
        let digest: [u8; 32] = [
            0x39, 0x9b, 0x35, 0xde, 0x55, 0x97, 0xb9, 0xa1, 0xab, 0xcb, 0x61, 0x17, 0x32, 0xa6,
            0x7b, 0xbf, 0x79, 0x49, 0xc4, 0x6c, 0x71, 0xc2, 0x53, 0xd6, 0x96, 0x51, 0x34, 0x0b,
            0x52, 0x85, 0xd9, 0x6e,
        ];
        // RSA PKCS#1 v1.5 (TLS 1.2): DigestInfo(SHA-256) || hash.
        let mut digest_info = vec![
            0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x01, 0x05, 0x00, 0x04, 0x20,
        ];
        digest_info.extend_from_slice(&digest);
        let pss = cryptoki::mechanism::rsa::PkcsPssParams {
            hash_alg: cryptoki::mechanism::MechanismType::SHA256,
            mgf: cryptoki::mechanism::rsa::PkcsMgfType::MGF1_SHA256,
            s_len: 32.into(),
        };
        let wanted = std::env::var("TESSERA_MECH").unwrap_or_default();
        let tries: Vec<(&str, Mechanism, &[u8])> = vec![
            ("CKM_RSA_PKCS (TLS 1.2)", Mechanism::RsaPkcs, &digest_info),
            (
                "CKM_RSA_PKCS_PSS (TLS 1.3)",
                Mechanism::RsaPkcsPss(pss),
                &digest,
            ),
        ];
        let mut result = 1;
        for (name, mech, data) in tries {
            if !wanted.is_empty() && !name.contains(&wanted) {
                continue;
            }
            let started = std::time::Instant::now();
            eprintln!("sign: C_Sign({name}) — approve on the phone if asked...");
            match session.sign(&mech, key, data) {
                Ok(sig) => {
                    eprintln!("sign: OK, {} bytes in {:?}", sig.len(), started.elapsed());
                    result = 0;
                }
                Err(e) => eprintln!("sign: failed after {:?}: {e}", started.elapsed()),
            }
        }
        return result;
    }
    eprintln!("sign: no token with a private key");
    1
}

/// Runs in the child process: list tokens and their public certificates
/// through p11-kit-client.so, at P11_KIT_SERVER_ADDRESS.
/// With `with_certs` false, only the tokens (no certificate is read): for
/// asking a server what it serves, cheaply.
pub fn list_tokens_in_process(with_certs: bool) -> Result<String, String> {
    let module = client_module();
    let ctx = Pkcs11::new(&module).map_err(|e| e.to_string())?;
    ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let count_first = std::env::var_os(COUNT_FIRST).is_some();
    for (slot_index, slot) in slots(&ctx, &module, false, count_first)
        .map_err(|e| e.to_string())?
        .into_iter()
        .enumerate()
    {
        let Ok(info) = ctx.get_token_info(slot) else {
            continue;
        };
        if !info.token_initialized() {
            // An empty slot some drivers keep for initializing a new token.
            continue;
        }
        let slot_name = ctx
            .get_slot_info(slot)
            .map(|s| clean(s.slot_description()))
            .unwrap_or_default();
        let mut token = Token {
            driver: String::new(),
            label: clean(info.label()),
            manufacturer: clean(info.manufacturer_id()),
            model: clean(info.model()),
            serial: clean(info.serial_number()),
            certificates: vec![],
            slot_index,
            slot: slot_name,
        };
        if let Some(session) = with_certs.then(|| ctx.open_ro_session(slot).ok()).flatten() {
            let handles = session
                .find_objects(&[Attribute::Class(ObjectClass::CERTIFICATE)])
                .unwrap_or_default();
            for h in handles {
                let Ok(attrs) =
                    session.get_attributes(h, &[AttributeType::Label, AttributeType::Value])
                else {
                    continue;
                };
                let mut label = String::new();
                let mut value = Vec::new();
                for a in attrs {
                    match a {
                        Attribute::Label(l) => {
                            label = String::from_utf8_lossy(&l).trim().to_string()
                        }
                        Attribute::Value(v) => value = v,
                        _ => {}
                    }
                }
                if let Some(c) = parse_cert(label, &value) {
                    token.certificates.push(c);
                }
            }
        }
        out.push(token);
    }
    toml::to_string(&TokenList { tokens: out }).map_err(|e| e.to_string())
}

/// Read one driver's server in a child process (p11-kit-client takes its
/// address from the environment, so each server gets its own process).
fn tokens_of(id: &str, name: &str) -> Result<Vec<Token>, ReadError> {
    tokens_at(&paths::driver_socket(id), name, true)
}

/// Tokens served at a p11-kit server socket, read in a child process with
/// a time limit.
fn tokens_at(
    socket: &std::path::Path,
    name: &str,
    with_certs: bool,
) -> Result<Vec<Token>, ReadError> {
    let exe = std::env::current_exe().map_err(|e| Failed(e.to_string()))?;
    let mut cmd = Command::new(exe);
    cmd.arg("list-tokens");
    if !with_certs {
        cmd.arg("--no-certificates");
    }
    // One of Tessera's own servers may have no slot at all (see
    // slot_count). The system's always has some (its trust store), and is
    // spared the extra connection the count takes.
    if socket != paths::host_p11kit_socket() {
        cmd.env(COUNT_FIRST, "1");
    }
    let mut child = cmd
        .env(
            "P11_KIT_SERVER_ADDRESS",
            format!("unix:path={}", socket.display()),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failed(e.to_string()))?;
    // Read the output while the child runs: a long listing would otherwise
    // fill the pipe and block it until the time limit.
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut b);
        b
    });
    let err_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut b);
        b
    });
    // A driver that hangs (waiting for a reader, say) must not hang the
    // window: give up after a while.
    let deadline = Instant::now() + LIST_TIMEOUT;
    loop {
        match child.try_wait().map_err(|e| Failed(e.to_string()))? {
            Some(_) => break,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Timeout);
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let status = child.wait().map_err(|e| Failed(e.to_string()))?;
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    if !status.success() {
        return Err(Failed(String::from_utf8_lossy(&stderr).trim().to_string()));
    }
    let list: TokenList =
        toml::from_str(&String::from_utf8_lossy(&stdout)).map_err(|e| Failed(e.to_string()))?;
    Ok(list
        .tokens
        .into_iter()
        .map(|mut t| {
            t.driver = name.to_string();
            t
        })
        .collect())
}

/// What apps see: whether the system's p11-kit server (at the socket a
/// sandboxed app with `xdg-run/p11-kit/pkcs11` uses) serves the tokens
/// Tessera serves. A socket that answers is not enough: without the host
/// server, Flatpak puts its own trust-store server at that place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostView {
    /// No server answers there.
    Off,
    /// A server answers, but not with Tessera's tokens.
    WithoutTessera,
    /// The apps see Tessera's tokens: how many, and those they see more
    /// often than Tessera serves them (label, serial): the system has a
    /// driver of its own for them too (its OpenSC, say), so each shows
    /// twice.
    Sees(usize, Vec<(String, String)>),
    /// Tessera has no token to look for.
    NothingToCheck,
    /// The listing failed or took too long: nothing learned.
    Unknown,
}

/// The protocol version the system's p11-kit server answers with: 1 up to
/// p11-kit 0.25.5, 2 from 0.25.6 on, the line that decides whether signing
/// works through it (native.rs, OLD_P11KIT_CODENAMES). It sends the client's
/// byte of p11-kit's handshake (2, the highest it knows), reads the server's
/// (its own highest, if lower) and hangs up. Must run off the main thread.
pub fn host_protocol() -> Option<u8> {
    protocol_at(&paths::host_p11kit_socket())
}

fn protocol_at(socket: &std::path::Path) -> Option<u8> {
    use std::io::{Read, Write};
    let mut s = std::os::unix::net::UnixStream::connect(socket).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    s.set_write_timeout(Some(Duration::from_secs(3))).ok()?;
    s.write_all(&[2]).ok()?;
    let mut answer = [0u8];
    s.read_exact(&mut answer).ok()?;
    Some(answer[0])
}

/// The position, among all the slots the system's p11-kit server lists, of
/// the first one holding one of Tessera's tokens (`ours`: label and
/// serial). None when the server does not answer or has none of them. Must
/// run off the main thread.
pub fn host_slot_index(ours: &[(String, String)]) -> Option<usize> {
    let seen = tokens_at(&paths::host_p11kit_socket(), "", false).ok()?;
    seen.iter()
        .filter(|t| ours.iter().any(|(l, s)| &t.label == l && &t.serial == s))
        .map(|t| t.slot_index)
        .min()
}

/// The tokens Tessera serves (label, serial), read from each driver's
/// server. Must run off the main thread.
pub fn tessera_tokens() -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(paths::socket_dir()) else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|e| tokens_at(&e.path(), "", false).ok())
        .flatten()
        .map(|t| (t.label, t.serial))
        .collect()
}

/// Must run off the main thread. `ours` are the tokens Tessera serves.
pub fn host_view(ours: &[(String, String)]) -> HostView {
    let socket = paths::host_p11kit_socket();
    if std::os::unix::net::UnixStream::connect(&socket).is_err() {
        return HostView::Off;
    }
    if ours.is_empty() {
        return HostView::NothingToCheck;
    }
    match tokens_at(&socket, "", false) {
        Ok(seen) => {
            let seen: Vec<(String, String)> =
                seen.into_iter().map(|t| (t.label, t.serial)).collect();
            let n = ours.iter().filter(|t| seen.contains(t)).count();
            if n > 0 {
                HostView::Sees(n, served_twice(ours, &seen))
            } else {
                HostView::WithoutTessera
            }
        }
        Err(_) => HostView::Unknown,
    }
}

/// The tokens of `ours` that `seen` lists more often than `ours` does.
fn served_twice(ours: &[(String, String)], seen: &[(String, String)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for t in ours {
        let mine = ours.iter().filter(|o| *o == t).count();
        let there = seen.iter().filter(|s| *s == t).count();
        if there > mine && !out.contains(t) {
            out.push(t.clone());
        }
    }
    out
}

/// Does a token Tessera serves sit in this reader? Drivers that name the
/// slot after its reader (OpenSC, SafeNet) say so; for the others, any token
/// at all counts, so a reader is only flagged when nothing is served.
pub fn served_in_reader(tokens: &[Token], reader: &str) -> bool {
    let base = reader.trim();
    tokens.iter().any(|t| {
        let slot = t.slot.trim();
        slot.is_empty() || slot.starts_with(base) || base.starts_with(slot) || slot.contains(base)
    })
}

/// Tokens from different drivers that hold the same certificate: the same
/// token, served by two of Tessera's drivers (OpenSC and the vendor's, say),
/// which the apps would list twice. Each pair once: (token, driver, other
/// driver).
pub fn same_token_twice(tokens: &[Token]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for (i, a) in tokens.iter().enumerate() {
        for b in &tokens[i + 1..] {
            // The holder's own certificate, not the CA chain both may carry.
            let shared = a.certificates.iter().filter(|c| !c.ca).any(|c| {
                b.certificates
                    .iter()
                    .any(|d| c.issuer == d.issuer && c.serial == d.serial && !c.serial.is_empty())
            });
            // A card with several PINs shows as several slots of the same
            // driver: one warning per pair of drivers is enough.
            let known = out.iter().any(|(_, x, y): &(String, String, String)| {
                (x == &a.driver && y == &b.driver) || (x == &b.driver && y == &a.driver)
            });
            if a.driver != b.driver && shared && !known {
                let name = if a.label.is_empty() {
                    &a.model
                } else {
                    &a.label
                };
                out.push((name.clone(), a.driver.clone(), b.driver.clone()));
            }
        }
    }
    out
}

/// Readers and tokens. `drivers` are (id, name) of the running servers,
/// read in parallel. Must run off the main thread.
pub fn snapshot(drivers: &[(String, String)]) -> Snapshot {
    let handles: Vec<_> = drivers
        .iter()
        .cloned()
        .map(|(id, name)| {
            std::thread::spawn(move || {
                let r = tokens_of(&id, &name);
                (id, name, r)
            })
        })
        .collect();
    let readers = readers();
    let mut snap = Snapshot {
        readers,
        tokens: Vec::new(),
        errors: Vec::new(),
        ok: Vec::new(),
        timed_out: Vec::new(),
    };
    for h in handles {
        let Ok((id, name, r)) = h.join() else {
            continue;
        };
        match r {
            Ok(t) => {
                snap.tokens.extend(t);
                snap.ok.push(id);
            }
            Err(Timeout) => {
                // Shown on the driver's own row (see check_drivers).
                let _ = name;
                snap.timed_out.push(id);
            }
            Err(Failed(e)) => snap.errors.push(format!("{name}: {e}")),
        }
    }
    snap
}

#[cfg(test)]
mod protocol_tests {
    use super::protocol_at;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    /// A server that answers the handshake as p11-kit does: the client's
    /// version, or its own highest if that is lower.
    fn fake_server(highest: u8) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tessera-protocol-{}-{highest}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("pkcs11");
        let listener = UnixListener::bind(&socket).unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut v = [0u8];
            s.read_exact(&mut v).unwrap();
            s.write_all(&[v[0].min(highest)]).unwrap();
        });
        socket
    }

    #[test]
    fn reads_the_servers_protocol_version() {
        assert_eq!(protocol_at(&fake_server(1)), Some(1));
        assert_eq!(protocol_at(&fake_server(2)), Some(2));
        assert_eq!(
            protocol_at(std::path::Path::new("/nonexistent/pkcs11")),
            None
        );
    }
}

#[cfg(test)]
mod duplicate_tests {
    use super::*;

    fn cert(serial: &str, ca: bool) -> Certificate {
        Certificate {
            subject: format!("S{serial}"),
            issuer: "AC Teste".into(),
            not_after: "2030-01-01".into(),
            expired: false,
            serial: serial.into(),
            ca,
        }
    }

    fn token(driver: &str, label: &str, certs: Vec<Certificate>) -> Token {
        Token {
            driver: driver.into(),
            label: label.into(),
            manufacturer: String::new(),
            model: String::new(),
            serial: String::new(),
            certificates: certs,
            slot_index: 0,
            slot: String::new(),
        }
    }

    #[test]
    fn a_token_from_two_drivers_is_found_by_its_certificate() {
        let tokens = vec![
            token(
                "SafeNet",
                "eToken",
                vec![cert("01", true), cert("aa", false)],
            ),
            token(
                "OpenSC",
                "Pedro (User)",
                vec![cert("01", true), cert("aa", false)],
            ),
            // The same card's second PIN: a second slot of OpenSC's.
            token(
                "OpenSC",
                "Pedro (Sign)",
                vec![cert("01", true), cert("aa", false)],
            ),
            // Another token with the same CA chain only: not the same.
            token(
                "SafeSign",
                "Outro",
                vec![cert("01", true), cert("bb", false)],
            ),
        ];
        assert_eq!(
            same_token_twice(&tokens),
            vec![(
                "eToken".to_string(),
                "SafeNet".to_string(),
                "OpenSC".to_string()
            )]
        );
    }

    #[test]
    fn a_reader_with_a_served_token_is_known() {
        let mut t = token("OpenSC", "IDPrime", vec![]);
        t.slot = "Generic USB Smart Card Reader 00 00".into();
        assert!(served_in_reader(
            &[t.clone()],
            "Generic USB Smart Card Reader 00 00"
        ));
        assert!(!served_in_reader(
            &[t.clone()],
            "SafeNet eToken 5100 [eToken 5110 SC] 00 00"
        ));
        assert!(!served_in_reader(
            &[],
            "Generic USB Smart Card Reader 00 00"
        ));
        // A driver that does not name its slot after the reader: any token counts.
        t.slot = String::new();
        assert!(served_in_reader(
            &[t],
            "Generic USB Smart Card Reader 00 00"
        ));
    }

    #[test]
    fn the_host_listing_a_token_twice_is_noticed() {
        let a = ("A".to_string(), "1".to_string());
        let b = ("B".to_string(), "2".to_string());
        let ours = vec![a.clone(), b.clone()];
        assert!(served_twice(&ours, &[a.clone(), b.clone()]).is_empty());
        assert_eq!(
            served_twice(&ours, &[a.clone(), b.clone(), b.clone()]),
            vec![b.clone()]
        );
        // Tessera serving it twice itself is reported elsewhere.
        let twice = vec![b.clone(), b.clone()];
        assert!(served_twice(&twice, &[b.clone(), b.clone()]).is_empty());
    }
}
