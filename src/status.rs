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
    let present = if ctx
        .get_status_change(std::time::Duration::from_millis(0), &mut states)
        .is_ok()
    {
        states
            .iter()
            .map(|s| s.event_state().contains(pcsc::State::PRESENT))
            .collect()
    } else {
        vec![false; names.len()]
    };
    let list = names
        .iter()
        .zip(present)
        .map(|(n, card_present)| Reader {
            name: n.to_string_lossy().into_owned(),
            card_present,
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
    })
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
                match ctx.get_all_slots() {
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
    let slots = ctx.get_slots_with_token().unwrap_or_default();
    for slot in slots {
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
pub fn list_tokens_in_process() -> Result<String, String> {
    let ctx = Pkcs11::new(client_module()).map_err(|e| e.to_string())?;
    ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for slot in ctx.get_slots_with_token().map_err(|e| e.to_string())? {
        let Ok(info) = ctx.get_token_info(slot) else {
            continue;
        };
        if !info.token_initialized() {
            // An empty slot some drivers keep for initializing a new token.
            continue;
        }
        let mut token = Token {
            driver: String::new(),
            label: clean(info.label()),
            manufacturer: clean(info.manufacturer_id()),
            model: clean(info.model()),
            serial: clean(info.serial_number()),
            certificates: vec![],
        };
        if let Ok(session) = ctx.open_ro_session(slot) {
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
    let exe = std::env::current_exe().map_err(|e| Failed(e.to_string()))?;
    let socket = paths::driver_socket(id);
    let mut child = Command::new(exe)
        .arg("list-tokens")
        .env(
            "P11_KIT_SERVER_ADDRESS",
            format!("unix:path={}", socket.display()),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failed(e.to_string()))?;
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
    let out = child
        .wait_with_output()
        .map_err(|e| Failed(e.to_string()))?;
    if !out.status.success() {
        return Err(Failed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    let list: TokenList =
        toml::from_str(&String::from_utf8_lossy(&out.stdout)).map_err(|e| Failed(e.to_string()))?;
    Ok(list
        .tokens
        .into_iter()
        .map(|mut t| {
            t.driver = name.to_string();
            t
        })
        .collect())
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
