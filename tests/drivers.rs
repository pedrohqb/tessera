//! End-to-end tests of the token service with test drivers: `tessera serve`
//! starts one p11-kit server per driver, each connection is served by
//! `tessera remote`, and a client signs through p11-kit-client.so, the way
//! the apps do.
//!
//! - SoftHSM: a real software token, with a PIN and an RSA key.
//! - tests/fake-driver/keeps-mechanism-pointer.c: behaves like SerproID,
//!   keeping the CK_MECHANISM pointer from C_SignInit and reading the PSS
//!   parameters only in C_Sign. It signs only if Tessera hands it parameters
//!   that are still valid then (src/stable.rs).
//! - tests/fake-driver/slow-slot-list.c: never answers in time, and must not
//!   be published.
//!
//! They need softhsm2, openssl, a C compiler, p11-kit's server helper and
//! client module, and the p11-kit headers. A test whose tools are missing is
//! skipped with a message, unless TESSERA_TEST_REQUIRE=1 (as in CI), where
//! it fails instead.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const TESSERA: &str = env!("CARGO_BIN_EXE_tessera");

const CLIENT_MODULES: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu/pkcs11/p11-kit-client.so",
    "/usr/lib64/pkcs11/p11-kit-client.so",
    "/usr/lib/pkcs11/p11-kit-client.so",
];
const SERVER_HELPERS: &[&str] = &[
    "/usr/libexec/p11-kit/p11-kit-server",
    "/usr/lib/x86_64-linux-gnu/p11-kit/p11-kit-server",
    "/usr/lib/p11-kit/p11-kit-server",
];
const SOFTHSM_MODULES: &[&str] = &[
    "/usr/lib/softhsm/libsofthsm2.so",
    "/usr/lib/x86_64-linux-gnu/softhsm/libsofthsm2.so",
    "/usr/lib64/pkcs11/libsofthsm2.so",
    "/usr/lib64/softhsm/libsofthsm2.so",
];

fn first(paths: &[&str]) -> Option<PathBuf> {
    paths.iter().map(PathBuf::from).find(|p| p.exists())
}

fn have(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd} >/dev/null")])
        .status()
        .is_ok_and(|s| s.success())
}

/// Skip (or, in CI, fail) when a tool is missing.
fn need(what: &str, ok: bool) -> bool {
    if ok {
        return true;
    }
    if std::env::var("TESSERA_TEST_REQUIRE").as_deref() == Ok("1") {
        panic!("missing for this test: {what}");
    }
    eprintln!("skipped: missing {what}");
    false
}

/// A scratch directory, removed at the end of the test.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("tessera-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
    fn path(&self, p: &str) -> PathBuf {
        self.0.join(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `tessera serve` with the given drivers, everything inside `s`.
struct Service {
    child: Child,
    sockets: PathBuf,
    modules: PathBuf,
}

impl Service {
    fn start(s: &Scratch, helper: &Path) -> Service {
        let run = s.path("run");
        fs::create_dir_all(&run).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&run, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let sockets = run.join("tessera");
        let modules = s.path("modules");
        let child = Command::new(TESSERA)
            .arg("serve")
            .env("XDG_RUNTIME_DIR", &run)
            .env("XDG_DATA_HOME", s.path("data"))
            .env("XDG_CONFIG_HOME", s.path("config"))
            .env("TESSERA_DRIVERS_PATH", s.path("drivers"))
            .env("TESSERA_SOCKET_DIR", &sockets)
            .env("TESSERA_HOST_MODULES", &modules)
            .env("TESSERA_P11KIT_SERVER", helper)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("starting tessera serve");
        Service {
            child,
            sockets,
            modules,
        }
    }

    /// Wait until `id` is published to the host (its module file exists).
    fn wait_published(&self, id: &str, limit: Duration) -> bool {
        let file = self.modules.join(format!("tessera-{id}.module"));
        let end = Instant::now() + limit;
        while Instant::now() < end {
            if file.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }

    /// Stop the service and return what it printed.
    fn stop(mut self) -> String {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let mut out = String::new();
        if let Some(mut e) = self.child.stderr.take() {
            let _ = e.read_to_string(&mut out);
        }
        out
    }
}

fn write_driver(s: &Scratch, id: &str, module: &Path, env: &[(&str, &str)]) {
    let dir = s.path(&format!("drivers/{id}"));
    fs::create_dir_all(&dir).unwrap();
    let mut toml = format!(
        "name = \"{id} (test)\"\nvendor = \"test\"\ndescription = \"test driver\"\nmodule = \"{}\"\n",
        module.display()
    );
    if !env.is_empty() {
        toml.push_str("[env]\n");
        for (k, v) in env {
            toml.push_str(&format!("{k} = \"{v}\"\n"));
        }
    }
    fs::write(dir.join("driver.toml"), toml).unwrap();
}

/// Sign through p11-kit-client.so connected to one driver's socket, as an
/// app would through the host's server.
fn sign_through(client: &Path, socket: &Path, pin: Option<&str>, mech: &str) -> Output {
    let mut cmd = Command::new(TESSERA);
    cmd.arg("probe-sign")
        .arg(client)
        .env(
            "P11_KIT_SERVER_ADDRESS",
            format!("unix:path={}", socket.display()),
        )
        .env("TESSERA_MECH", mech);
    if let Some(pin) = pin {
        cmd.env("TESSERA_PIN", pin);
    }
    cmd.output().expect("running probe-sign")
}

fn compile_fake(s: &Scratch, source: &str) -> Option<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake-driver")
        .join(source);
    let out = s.path(&source.replace(".c", ".so"));
    let ok = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-o"])
        .arg(&out)
        .arg(&src)
        .status()
        .is_ok_and(|st| st.success());
    ok.then_some(out)
}

#[test]
fn softhsm_token_is_served_and_signs_with_its_pin() {
    let (Some(helper), Some(client), Some(softhsm)) = (
        first(SERVER_HELPERS),
        first(CLIENT_MODULES),
        first(SOFTHSM_MODULES),
    ) else {
        need("p11-kit server helper, p11-kit-client.so or SoftHSM", false);
        return;
    };
    if !need(
        "softhsm2-util and openssl",
        have("softhsm2-util") && have("openssl"),
    ) {
        return;
    }
    let s = Scratch::new("softhsm");
    let tokens = s.path("hsm");
    fs::create_dir_all(&tokens).unwrap();
    let conf = s.path("softhsm2.conf");
    fs::write(
        &conf,
        format!(
            "directories.tokendir = {}\nobjectstore.backend = file\n",
            tokens.display()
        ),
    )
    .unwrap();
    let hsm = |args: &[&str]| {
        let st = Command::new("softhsm2-util")
            .args(args)
            .env("SOFTHSM2_CONF", &conf)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(st.success(), "softhsm2-util {args:?} failed");
    };
    hsm(&[
        "--init-token",
        "--free",
        "--label",
        "Teste A3",
        "--pin",
        "1234",
        "--so-pin",
        "1234",
    ]);
    let key = s.path("key.pem");
    let st = Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
        ])
        .arg(&key)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(st.success(), "openssl genpkey failed");
    hsm(&[
        "--import",
        key.to_str().unwrap(),
        "--token",
        "Teste A3",
        "--label",
        "chave",
        "--id",
        "01",
        "--pin",
        "1234",
    ]);
    write_driver(
        &s,
        "softhsm",
        &softhsm,
        &[("SOFTHSM2_CONF", conf.to_str().unwrap())],
    );

    let svc = Service::start(&s, &helper);
    let published = svc.wait_published("softhsm", Duration::from_secs(30));
    let socket = svc.sockets.join("softhsm");
    let module_file =
        fs::read_to_string(svc.modules.join("tessera-softhsm.module")).unwrap_or_default();
    let pss = sign_through(&client, &socket, Some("1234"), "PSS");
    let v15 = sign_through(&client, &socket, Some("1234"), "RSA_PKCS (");
    let wrong_pin = sign_through(&client, &socket, Some("0000"), "PSS");
    let log = svc.stop();

    assert!(
        published,
        "softhsm was never published; tessera serve said:\n{log}"
    );
    assert!(
        module_file.contains(&format!("remote: unix:path={}", socket.display())),
        "unexpected module file:\n{module_file}"
    );
    let text = |o: &Output| String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(pss.status.success(), "PSS signing failed:\n{}", text(&pss));
    assert!(
        text(&pss).contains("Teste A3"),
        "token not seen:\n{}",
        text(&pss)
    );
    assert!(
        v15.status.success(),
        "PKCS#1 v1.5 signing failed:\n{}",
        text(&v15)
    );
    assert!(
        !wrong_pin.status.success() && text(&wrong_pin).contains("C_Login failed"),
        "a wrong PIN was accepted:\n{}",
        text(&wrong_pin)
    );
}

#[test]
fn driver_that_keeps_the_mechanism_pointer_signs_pss() {
    let (Some(helper), Some(client)) = (first(SERVER_HELPERS), first(CLIENT_MODULES)) else {
        need("p11-kit server helper or p11-kit-client.so", false);
        return;
    };
    let s = Scratch::new("pointer");
    let Some(fake) = compile_fake(&s, "keeps-mechanism-pointer.c") else {
        need(
            "a C compiler and the p11-kit headers (libp11-kit-dev)",
            false,
        );
        return;
    };
    write_driver(&s, "fake", &fake, &[]);

    let svc = Service::start(&s, &helper);
    let published = svc.wait_published("fake", Duration::from_secs(30));
    let socket = svc.sockets.join("fake");
    let pss = sign_through(&client, &socket, None, "PSS");
    let log = svc.stop();
    // The driver's own messages go to its server's log.
    let driver_log = fs::read_to_string(s.path("data/tessera/logs/fake.log")).unwrap_or_default();

    assert!(published, "the fake driver was never published:\n{log}");
    let out = String::from_utf8_lossy(&pss.stderr);
    assert!(
        pss.status.success(),
        "PSS signing failed: the driver read parameters that were no longer valid.\n\
         probe-sign:\n{out}\ndriver log:\n{driver_log}"
    );
    assert!(
        driver_log.contains("fake: C_Sign sees mech=0xd hash=0x250 mgf=0x2 sLen=32 -> OK"),
        "the driver did not report correct PSS parameters:\n{driver_log}"
    );
}

#[test]
fn driver_that_never_answers_is_not_published() {
    let Some(helper) = first(SERVER_HELPERS) else {
        need("p11-kit server helper", false);
        return;
    };
    let s = Scratch::new("slow");
    let Some(slow) = compile_fake(&s, "slow-slot-list.c") else {
        need(
            "a C compiler and the p11-kit headers (libp11-kit-dev)",
            false,
        );
        return;
    };
    write_driver(&s, "slow", &slow, &[]);

    let svc = Service::start(&s, &helper);
    // The listing gives up after 10 s; it is counted as the first strike.
    let published = svc.wait_published("slow", Duration::from_secs(14));
    let log = svc.stop();

    assert!(
        !published,
        "a driver that never answers was published:\n{log}"
    );
    assert!(
        log.contains("slow: no answer (1 of 3)"),
        "the timeout was not reported:\n{log}"
    );
}
