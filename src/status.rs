//! What the user sees: card readers (through pcscd) and tokens with their
//! certificates (through our own socket, the same path client apps use).

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::object::{Attribute, AttributeType, ObjectClass};

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

#[derive(Debug, Clone)]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    /// "YYYY-MM-DD"
    pub not_after: String,
    pub expired: bool,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub label: String,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub certificates: Vec<Certificate>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub readers: Readers,
    pub tokens: Result<Vec<Token>, String>,
}

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

/// List tokens and their public certificates through p11-kit-client. Must
/// run off the main thread: a slow token can take a second or two.
pub fn tokens() -> Result<Vec<Token>, String> {
    if !paths::socket_path().exists() {
        return Ok(vec![]);
    }
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
            label: info.label().trim().to_string(),
            manufacturer: info.manufacturer_id().trim().to_string(),
            model: info.model().trim().to_string(),
            serial: info.serial_number().trim().to_string(),
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
    Ok(out)
}

pub fn snapshot() -> Snapshot {
    Snapshot {
        readers: readers(),
        tokens: tokens(),
    }
}
