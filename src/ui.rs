//! The window: service status, readers, tokens, drivers and client apps.

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::process::{Command, ExitCode};
use std::rc::Rc;
use std::time::Duration;

use crate::apps;
use crate::drivers::{self, Driver};
use crate::i18n::t;
use crate::nss;
use crate::paths;
use crate::server::{Server, State};
use crate::settings::Settings;
use crate::status::{self, Readers, Snapshot};
use crate::tray;

/// State that outlives any window: the server keeps running in the
/// background when the window is closed.
struct Hub {
    server: RefCell<Server>,
    drivers: RefCell<Vec<Driver>>,
    settings: RefCell<Settings>,
    /// Host module files written (count), or why not.
    host: RefCell<Result<usize, String>>,
    _hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    /// Bumped by every (re)start: a listing started before it is stale and
    /// must not stop or publish the new servers.
    generation: Cell<u64>,
    health: RefCell<crate::Health>,
    /// A listing is running.
    busy: Cell<bool>,
    /// Tokens seen by the last listing (for the tray and the portal).
    tokens: Cell<usize>,
    /// Drivers that answered the last listing (published to the host).
    published: RefCell<Vec<String>>,
    /// What apps see through the system's p11-kit server, and when and for
    /// which tokens that was last checked.
    host_view: RefCell<Option<status::HostView>>,
    host_checked: RefCell<Option<HostCheck>>,
    host_busy: Cell<bool>,
    /// The open window, if any (dropped when the window is destroyed).
    win: RefCell<Option<Rc<Win>>>,
    tray: RefCell<Option<tray::Tray>>,
    tray_events: async_channel::Sender<tray::Event>,
    status_sent: RefCell<String>,
}

impl Hub {
    fn start_server(&self) {
        self.generation.set(self.generation.get() + 1);
        self.health.borrow_mut().reset();
        self.published.borrow_mut().clear();
        let ds = drivers::discover();
        let disabled = self.settings.borrow().disabled_drivers.clone();
        let result = self.server.borrow_mut().start(&ds, &disabled);
        if let Err(e) = result {
            self.server.borrow_mut().set_failure(format!("{e:#}"));
        }
        // Nothing is published to the host until each driver has answered
        // once (see refresh): a driver that hangs would hang every app.
        *self.host.borrow_mut() = apps::write_host_modules(&ds, &[]).map_err(|e| format!("{e:#}"));
        *self.drivers.borrow_mut() = ds;
    }

    /// Stop serving: servers down and nothing left published on the host
    /// (module files pointing at stopped servers would only make apps wait).
    fn shut_down(&self) {
        self.generation.set(self.generation.get() + 1);
        self.server.borrow_mut().stop();
        let _ = apps::write_host_modules(&self.drivers.borrow(), &[]);
        self.tray.borrow_mut().take();
    }

    /// One line for the tray and the background-apps list.
    fn status_text(&self) -> String {
        let (running, total) = self.server.borrow().counts();
        let published = self.host.borrow().as_ref().copied().unwrap_or(0);
        format!(
            "{} · {running}/{total} {} · {published} {} · {} {}",
            t("Serving"),
            t("driver(s)"),
            t("published"),
            self.tokens.get(),
            t("token(s)")
        )
    }

    /// (id, name) of the drivers whose server runs.
    fn running(&self) -> Vec<(String, String)> {
        let running = self.server.borrow().running();
        self.drivers
            .borrow()
            .iter()
            .filter(|d| running.contains(&d.id))
            .map(|d| (d.id.clone(), d.name.clone()))
            .collect()
    }
}

/// A preferences group whose rows are rebuilt on refresh.
struct Section {
    group: adw::PreferencesGroup,
    rows: RefCell<Vec<gtk::Widget>>,
}

impl Section {
    fn new(title: &str, description: Option<&str>) -> Self {
        let group = adw::PreferencesGroup::builder().title(title).build();
        if let Some(d) = description {
            group.set_description(Some(d));
        }
        Self {
            group,
            rows: RefCell::new(vec![]),
        }
    }

    fn clear(&self) {
        for w in self.rows.borrow_mut().drain(..) {
            self.group.remove(&w);
        }
    }

    fn add(&self, w: &impl IsA<gtk::Widget>) {
        self.group.add(w);
        self.rows.borrow_mut().push(w.clone().upcast());
    }
}

struct Win {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    service_row: adw::ActionRow,
    service_button: gtk::Button,
    host_row: adw::ActionRow,
    pcsc_row: adw::ActionRow,
    readers: Section,
    tokens: Section,
    drivers: Section,
    /// (driver id, its row, the row's own subtitle) for update_drivers.
    driver_rows: RefCell<Vec<(String, adw::ActionRow, String)>>,
    last: RefCell<String>,
}

impl Win {
    fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// For messages too long for a toast (what went wrong and what to do).
    fn alert(&self, heading: &str, body: &str) {
        let dialog = adw::AlertDialog::new(Some(heading), Some(body));
        dialog.add_response("ok", "OK");
        dialog.present(Some(&self.window));
    }
}

fn plain_row(title: &str, subtitle: &str, icon: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .use_markup(false)
        .title(title)
        .subtitle(subtitle)
        .build();
    if !icon.is_empty() {
        row.add_prefix(&gtk::Image::from_icon_name(icon));
    }
    row
}

fn status_page_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = plain_row(title, subtitle, "");
    row.add_css_class("dim-label");
    row
}

fn copy_button(win: &Rc<Win>, text: String) -> gtk::Button {
    let b = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text(t("Copy the command that lets this app reach Tessera"))
        .css_classes(["flat"])
        .build();
    let w = Rc::downgrade(win);
    b.connect_clicked(move |_| {
        if let Some(win) = w.upgrade() {
            win.window.clipboard().set_text(&text);
            win.toast(&format!("{} — {}", t("Command copied"), text));
        }
    });
    b
}

fn update_service(hub: &Hub, win: &Win) {
    let state = hub.server.borrow_mut().poll();
    let dir = paths::socket_dir();
    let n = hub.server.borrow().running().len();
    let (subtitle, button, warn) = match &state {
        State::Running => (format!("{} · {n} {} · {}", t("Running"), t("driver(s)"), dir.display()), t("Restart"), false),
        State::Stopped => (t("Stopped").to_string(), t("Start"), false),
        State::Conflict => (t("Another program already answers on this socket (is Tessera already running, for example as `tessera serve`?).").to_string(), t("Start"), true),
        State::Failed(msg) => (format!("{}: {msg}", t("The server exited")), t("Start"), true),
    };
    win.service_row.set_subtitle(&subtitle);
    win.service_button.set_label(button);
    if warn {
        win.service_row.add_css_class("error");
    } else {
        win.service_row.remove_css_class("error");
    }
    let off = format!(
        "{} {}",
        t("Off: the apps cannot see the tokens. Run once in a terminal, then quit and reopen Tessera:"),
        apps::HOST_SERVER_COMMAND
    );
    let (text, bad) = match (&*hub.host.borrow(), &*hub.host_view.borrow()) {
        (Err(e), _) => (format!("{}: {e}", t("Could not write the host module files")), true),
        (Ok(_), Some(status::HostView::Sees(n))) => (
            format!("{} · {n} {}", t("On"), t("token(s) of Tessera seen by the apps")),
            false,
        ),
        (Ok(n), Some(status::HostView::NothingToCheck)) => (
            format!("{} · {n} {}", t("On"), t("driver(s) published; no token to check yet")),
            false,
        ),
        (Ok(_), Some(status::HostView::WithoutTessera)) => (
            format!(
                "{} {}",
                t("The apps do not see Tessera's tokens: the system's p11-kit server is off, or was turned on after Tessera started. Run once in a terminal, then quit and reopen Tessera:"),
                apps::HOST_SERVER_COMMAND
            ),
            true,
        ),
        (Ok(_), Some(status::HostView::Off)) => (off, true),
        (Ok(_), None) if apps::host_server_answers() => (t("Checking…").to_string(), false),
        (Ok(_), None) => (off, true),
    };
    win.host_row.set_subtitle(&text);
    if bad {
        win.host_row.add_css_class("error");
    } else {
        win.host_row.remove_css_class("error");
    }
}

fn apply_snapshot(win: &Win, snap: Snapshot) {
    let fingerprint = format!("{snap:?}");
    if *win.last.borrow() == fingerprint {
        return;
    }
    *win.last.borrow_mut() = fingerprint;

    win.readers.clear();
    match &snap.readers {
        Readers::NoService(err) => {
            win.pcsc_row.set_subtitle(&format!(
                "{} ({err})",
                t("Not reachable: is pcscd installed and running on the system?")
            ));
            win.pcsc_row.add_css_class("error");
            win.readers.add(&status_page_row(
                t("No reader found"),
                t("Not reachable: is pcscd installed and running on the system?"),
            ));
        }
        Readers::Ok(list) => {
            win.pcsc_row.set_subtitle(t("Reachable"));
            win.pcsc_row.remove_css_class("error");
            if list.is_empty() {
                win.readers.add(&status_page_row(
                    t("No reader found"),
                    t("Connect a token or card reader."),
                ));
            }
            for r in list {
                let state = if r.card_present {
                    t("Card present")
                } else {
                    t("Empty")
                };
                win.readers
                    .add(&plain_row(&r.name, state, "media-removable-symbolic"));
            }
        }
    }

    win.tokens.clear();
    for e in &snap.errors {
        let row = status_page_row(t("A driver could not be read"), e);
        row.add_css_class("error");
        win.tokens.add(&row);
    }
    match snap.tokens {
        tokens if tokens.is_empty() => win.tokens.add(&status_page_row(
            t("No token found"),
            t("Insert a token, or install the driver for it below."),
        )),
        tokens => {
            for tok in tokens {
                let n = tok.certificates.len();
                let count = format!(
                    "{n} {}",
                    if n == 1 {
                        t("certificate")
                    } else {
                        t("certificates")
                    }
                );
                let row = adw::ExpanderRow::builder()
                    .use_markup(false)
                    .title(if tok.label.is_empty() {
                        &tok.model
                    } else {
                        &tok.label
                    })
                    .subtitle(
                        format!(
                            "{} {} · {} · {count}",
                            tok.manufacturer, tok.model, tok.driver
                        )
                        .trim(),
                    )
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name("auth-smartcard-symbolic"));
                if n == 0 {
                    row.add_row(&status_page_row(
                        t("No certificates visible before login"),
                        "",
                    ));
                }
                for c in tok.certificates {
                    let when = if c.expired {
                        t("Expired on")
                    } else {
                        t("Valid until")
                    };
                    let cert_row = plain_row(
                        &c.subject,
                        &format!("{} {} · {when} {}", t("Issued by"), c.issuer, c.not_after),
                        "application-certificate-symbolic",
                    );
                    if c.expired {
                        cert_row.add_css_class("warning");
                    }
                    row.add_row(&cert_row);
                }
                if n > 0 {
                    row.set_expanded(true);
                }
                win.tokens.add(&row);
            }
        }
    }
}

fn refresh(hub: &Rc<Hub>, win: &Rc<Win>) {
    update_service(hub, win);
    supervise(hub);
}

/// List the tokens of every running driver, then publish the drivers that
/// answered and restart (or, in the end, stop) those that did not. Runs with
/// or without a window: with the window closed, a token inserted later must
/// still reach the apps.
fn supervise(hub: &Rc<Hub>) {
    if hub.busy.replace(true) {
        return;
    }
    let generation = hub.generation.get();
    let running = hub.running();
    let hub = hub.clone();
    glib::spawn_future_local(async move {
        let snap = gio::spawn_blocking(move || status::snapshot(&running)).await;
        hub.busy.set(false);
        let Ok(snap) = snap else { return };
        if generation != hub.generation.get() {
            // The servers were restarted while this listing ran: its
            // timeouts are about servers that no longer exist.
            supervise(&hub);
            return;
        }
        let host = {
            let ds = hub.drivers.borrow();
            crate::check_drivers(
                &mut hub.server.borrow_mut(),
                &ds,
                &snap,
                &mut hub.health.borrow_mut(),
            )
        };
        *hub.host.borrow_mut() = host;
        hub.tokens.set(snap.tokens.len());
        *hub.published.borrow_mut() = snap.ok.clone();
        let ours: Vec<(String, String)> = snap
            .tokens
            .iter()
            .map(|t| (t.label.clone(), t.serial.clone()))
            .collect();
        check_host(&hub, ours);
        update_status(&hub);
        let win = hub.win.borrow().clone();
        if let Some(win) = win {
            update_service(&hub, &win);
            update_drivers(&hub, &win);
            apply_snapshot(&win, snap);
        }
    });
}

/// When the host was last checked, and for which (label, serial) tokens.
type HostCheck = (std::time::Instant, Vec<(String, String)>);

/// Ask, through the system's p11-kit server, whether apps see Tessera's
/// tokens: when they change, and every 30 s with the window open (each
/// check loads the drivers again through that server).
fn check_host(hub: &Rc<Hub>, ours: Vec<(String, String)>) {
    let visible = hub
        .win
        .borrow()
        .as_ref()
        .is_some_and(|w| w.window.is_visible());
    let due = match &*hub.host_checked.borrow() {
        None => true,
        Some((when, last)) => {
            *last != ours || (visible && when.elapsed() > Duration::from_secs(30))
        }
    };
    if !due || hub.host_busy.replace(true) {
        return;
    }
    *hub.host_checked.borrow_mut() = Some((std::time::Instant::now(), ours.clone()));
    let hub = hub.clone();
    glib::spawn_future_local(async move {
        let view = gio::spawn_blocking(move || status::host_view(&ours)).await;
        hub.host_busy.set(false);
        if let Ok(view) = view {
            *hub.host_view.borrow_mut() = Some(view);
            let win = hub.win.borrow().clone();
            if let Some(win) = win {
                update_service(&hub, &win);
            }
        }
    });
}

/// Keep the tray icon and the background-apps status in step.
fn update_status(hub: &Rc<Hub>) {
    let text = hub.status_text();
    let visible = hub
        .win
        .borrow()
        .as_ref()
        .is_some_and(|w| w.window.is_visible());
    {
        let mut tray = hub.tray.borrow_mut();
        if visible {
            tray.take();
        } else if let Some(t) = tray.as_ref() {
            t.set_status(&text);
        } else {
            *tray = tray::Tray::show(text.clone(), tray::icon_pixmaps(), hub.tray_events.clone());
        }
    }
    if *hub.status_sent.borrow() != text {
        *hub.status_sent.borrow_mut() = text.clone();
        glib::spawn_future_local(async move {
            // GNOME lists background apps with this line; older portals
            // lack SetStatus, which is fine.
            if let Ok(proxy) = ashpd::desktop::background::BackgroundProxy::new().await {
                let opts = ashpd::desktop::background::SetStatusOptions::default()
                    .set_message(&text.chars().take(96).collect::<String>());
                let _ = proxy.set_status(opts).await;
            }
        });
    }
}

/// Each driver row says whether apps can use it, and why not.
fn update_drivers(hub: &Hub, win: &Win) {
    let disabled = hub.settings.borrow().disabled_drivers.clone();
    let running = hub.server.borrow().running();
    for (id, row, base) in win.driver_rows.borrow().iter() {
        let strikes = hub.health.borrow().strikes(id);
        let (state, bad) = if disabled.contains(id) {
            (t("Switched off").to_string(), false)
        } else if let Some(why) = hub.server.borrow().failure_of(id) {
            (format!("{}: {why}", t("Stopped")), true)
        } else if hub.published.borrow().contains(id) {
            (t("Published: the apps can use it").to_string(), false)
        } else if strikes > 0 && running.contains(id) {
            (
                format!(
                    "{} ({strikes} {} {})",
                    t("No answer; trying again"),
                    t("of"),
                    crate::MAX_STRIKES
                ),
                true,
            )
        } else if running.contains(id) {
            (t("Starting…").to_string(), false)
        } else {
            (String::new(), false)
        };
        row.set_subtitle(&match (base.is_empty(), state.is_empty()) {
            (true, _) => state,
            (false, true) => base.clone(),
            (false, false) => format!("{base}\n{state}"),
        });
        if bad {
            row.add_css_class("warning");
        } else {
            row.remove_css_class("warning");
        }
    }
}

fn fill_drivers(hub: &Rc<Hub>, win: &Rc<Win>) {
    win.drivers.clear();
    win.driver_rows.borrow_mut().clear();
    let ds = hub.drivers.borrow();
    if ds.is_empty() {
        win.drivers
            .add(&status_page_row(t("No driver installed"), ""));
    }
    for d in ds.iter() {
        let mut subtitle = String::new();
        if !d.vendor.is_empty() {
            subtitle.push_str(&d.vendor);
        }
        if !d.description.is_empty() {
            if !subtitle.is_empty() {
                subtitle.push_str(" · ");
            }
            subtitle.push_str(&d.description);
        }
        let row = plain_row(&d.name, &subtitle, "application-x-addon-symbolic");
        if !d.available() {
            let missing = format!("{}: {}", t("Driver files missing"), d.module.display());
            row.set_subtitle(&if subtitle.is_empty() {
                missing
            } else {
                format!("{subtitle}\n{missing}")
            });
            row.add_css_class("warning");
        }
        for tool in &d.tools {
            let b = gtk::Button::builder()
                .label(&tool.name)
                .valign(gtk::Align::Center)
                .build();
            b.set_sensitive(d.available());
            let tool = tool.clone();
            let driver = d.clone();
            let w = Rc::downgrade(win);
            b.connect_clicked(move |_| {
                let Some((prog, args)) = tool.exec.split_first() else {
                    return;
                };
                // The folders the vendor tool needs (SerproID crashes
                // without ~/.config/serproid/certificados), even when the
                // driver itself is switched off.
                drivers::prepare_dirs(std::slice::from_ref(&driver));
                if let Err(e) = Command::new(prog).args(args).envs(&tool.env).spawn() {
                    if let Some(win) = w.upgrade() {
                        win.toast(&format!("{}: {e}", tool.name));
                    }
                }
            });
            row.add_suffix(&b);
        }
        // On/off: a switched-off driver gets no server and is not published.
        let enabled = !hub.settings.borrow().disabled_drivers.contains(&d.id);
        let switch = gtk::Switch::builder()
            .active(enabled)
            .valign(gtk::Align::Center)
            .tooltip_text(t("Use this driver"))
            .build();
        switch.set_sensitive(d.available());
        {
            let (hub, w, id) = (hub.clone(), Rc::downgrade(win), d.id.clone());
            switch.connect_active_notify(move |sw| {
                {
                    let mut st = hub.settings.borrow_mut();
                    st.disabled_drivers.retain(|x| x != &id);
                    if !sw.is_active() {
                        st.disabled_drivers.push(id.clone());
                    }
                    st.save();
                }
                hub.start_server();
                if let Some(win) = w.upgrade() {
                    win.last.borrow_mut().clear();
                    refresh(&hub, &win);
                }
            });
        }
        row.add_suffix(&switch);
        win.drivers.add(&row);
        if d.available() {
            win.driver_rows
                .borrow_mut()
                .push((d.id.clone(), row.clone(), subtitle.clone()));
        }
    }
    drop(ds);
    update_drivers(hub, win);
}

use apps::{ClientApp, Setup};

/// Is this app ready: (has the p11-kit socket, its NSS side is set up).
fn app_state(app: &ClientApp) -> (bool, bool) {
    let socket = apps::has_socket_override(app.id);
    let nss = match app.setup {
        Setup::Poppler => nss::poppler_db_ready(&paths::poppler_nss_dir(app.id)),
        Setup::Firefox => {
            let profiles = paths::firefox_profiles();
            !profiles.is_empty() && profiles.iter().all(|p| nss::has_module(p))
        }
        Setup::Chromium => !apps::chromium_nssdb(app).is_some_and(|db| nss::has_module(&db)),
        Setup::None => true,
    };
    (socket, nss)
}

fn app_subtitle(app: &ClientApp) -> String {
    let state = match app_state(app) {
        (true, true) => t("Ready"),
        (false, true) => t("Step 1 missing"),
        (true, false) => t("Step 2 missing"),
        (false, false) => t("Not set up"),
    };
    format!("{} · {state}", app.id)
}

/// The NSS work of step 2, in a child process. Returns a line for the user.
fn set_up_nss(app: &'static ClientApp) -> Result<String, String> {
    match app.setup {
        Setup::Poppler => nss::set_up_poppler_db(&paths::poppler_nss_dir(app.id))
            .map(|_| {
                format!(
                    "{} {}",
                    app.name,
                    t("is set up: restart it to load the module")
                )
            })
            .map_err(|e| format!("{e:#}")),
        Setup::Firefox => {
            let profiles = paths::firefox_profiles();
            if profiles.is_empty() {
                return Err(
                    t("No Firefox profile found: open Firefox once, then try again").into(),
                );
            }
            if profiles.iter().any(|p| nss::profile_in_use(p)) {
                return Err(t("Close Firefox first, then try again").into());
            }
            for p in &profiles {
                nss::add_module(p).map_err(|e| format!("{}: {e:#}", p.display()))?;
            }
            Ok(format!("{} ({})", t("Firefox is set up"), profiles.len()))
        }
        // Chromium loads the root-certificates module (libnssckbi.so) by
        // itself, and in the Flatpak runtime that is p11-kit, which reaches
        // the system's server once step 1 is done: like Papers, the browser
        // needs no module of its own. With p11-kit-client.so registered too,
        // every token showed twice and the PIN was asked once per copy, so
        // step 2 only removes the module an older Tessera registered.
        Setup::Chromium => match apps::chromium_nssdb(app) {
            Some(db) if nss::has_module(&db) => {
                nss::set_up_poppler_db(&db).map_err(|e| format!("{}: {e:#}", db.display()))?;
                Ok(format!(
                    "{} {}",
                    app.name,
                    t("is set up: the duplicate module was removed; restart the browser")
                ))
            }
            _ => Ok(format!(
                "{} {}",
                app.name,
                t("needs nothing more: step 1 is enough")
            )),
        },
        Setup::None => Ok(String::new()),
    }
}

fn set_up_app(
    win: &Rc<Win>,
    app: &'static ClientApp,
    status: adw::ExpanderRow,
    step1: adw::ActionRow,
) {
    // Registering the client module needs no running driver.
    let win = win.clone();
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(move || set_up_nss(app)).await;
        match res {
            Ok(Ok(done)) => win.toast(&done),
            Ok(Err(e)) => win.alert(t("Could not set up"), &e),
            Err(_) => win.toast(t("Could not set up")),
        }
        status.set_subtitle(&app_subtitle(app));
        step1_state(&step1, app);
    });
}

fn step1_state(row: &adw::ActionRow, app: &ClientApp) {
    if apps::has_socket_override(app.id) {
        row.set_title(t("1. The app may reach the system's p11-kit server ✓"));
    } else {
        row.set_title(t(
            "1. Let the app reach the system's p11-kit server (run once in a terminal, then restart the app)",
        ));
    }
}

fn client_row(win: &Rc<Win>, app: &'static ClientApp) -> adw::ExpanderRow {
    let row = adw::ExpanderRow::builder()
        .use_markup(false)
        .title(app.name)
        .subtitle(app_subtitle(app))
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(app.icon));

    let cmd = apps::override_command(app);
    let step1 = adw::ActionRow::builder()
        .use_markup(false)
        .subtitle(&cmd)
        .subtitle_selectable(true)
        .build();
    step1_state(&step1, app);
    step1.add_suffix(&copy_button(win, cmd));
    row.add_row(&step1);

    let step2 = adw::ActionRow::builder()
        .use_markup(false)
        .title(t("2. Set up the app"))
        .subtitle(match app.setup {
            Setup::Firefox => t("Close Firefox first: adds the module to every Firefox profile"),
            Setup::Poppler => t("Makes the certificate database the app reads when signing"),
            _ => t("Nothing to add: the browser reaches the tokens through its root-certificates module once step 1 is done. The button removes the module an older Tessera added, which made it ask for the PIN twice"),
        })
        .build();
    let button = gtk::Button::builder()
        .label(t("Set up"))
        .valign(gtk::Align::Center)
        .build();
    {
        let (w, status, s1) = (Rc::downgrade(win), row.clone(), step1.clone());
        button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                set_up_app(&win, app, status.clone(), s1.clone());
            }
        });
    }
    step2.add_suffix(&button);
    row.add_row(&step2);

    if app.setup == Setup::Firefox {
        let manual = adw::ActionRow::builder()
            .use_markup(false)
            .title(t("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:"))
            .subtitle(paths::CLIENT_MODULE)
            .subtitle_selectable(true)
            .build();
        row.add_row(&manual);
    }
    row
}

fn apps_group(win: &Rc<Win>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t("Apps"))
        .description(t(
            "Apps that use the tokens through Tessera. Each needs the two steps once.",
        ))
        .build();
    for app in apps::APPS {
        group.add(&client_row(win, app));
    }
    // PJeOffice Pro (FlatPark) already has access to the host's p11-kit
    // server socket and loads p11-kit-client.so through its own bridge.
    let pje = plain_row(
        apps::PJEOFFICE.name,
        t("Works once the system connection is on (no setup needed)"),
        apps::PJEOFFICE.icon,
    );
    group.add(&pje);
    group
}

fn build_window(app: &adw::Application, hub: &Rc<Hub>) -> Rc<Win> {
    let header = adw::HeaderBar::new();
    let menu = gio::Menu::new();
    menu.append(
        Some(t("Start automatically at login")),
        Some("app.autostart"),
    );
    menu.append(Some(t("About Tessera")), Some("app.about"));
    menu.append(Some(t("Quit")), Some("app.quit"));
    header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .build(),
    );
    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(t("Refresh"))
        .build();
    header.pack_start(&refresh_button);

    let page = adw::PreferencesPage::new();

    let service = adw::PreferencesGroup::builder().title(t("Service")).build();
    let service_row = plain_row(t("Token service"), "", "emblem-system-symbolic");
    let service_button = gtk::Button::builder()
        .label(t("Start"))
        .valign(gtk::Align::Center)
        .build();
    service_row.add_suffix(&service_button);
    service.add(&service_row);
    let host_row = plain_row(
        t("System connection (the system's p11-kit server)"),
        "",
        "network-wired-symbolic",
    );
    host_row.set_subtitle_selectable(true);
    service.add(&host_row);
    let pcsc_row = plain_row(t("Smart card service (pcscd)"), "", "computer-symbolic");
    service.add(&pcsc_row);
    page.add(&service);

    let win = Rc::new(Win {
        window: adw::ApplicationWindow::builder()
            .application(app)
            .title(t("Tessera"))
            .default_width(620)
            .default_height(780)
            .build(),
        toasts: adw::ToastOverlay::new(),
        service_row,
        service_button,
        host_row,
        pcsc_row,
        readers: Section::new(t("Readers"), None),
        tokens: Section::new(t("Tokens"), None),
        drivers: Section::new(t("Drivers"), None),
        driver_rows: RefCell::new(vec![]),
        last: RefCell::new(String::new()),
    });
    win.host_row
        .add_suffix(&copy_button(&win, apps::HOST_SERVER_COMMAND.to_string()));
    page.add(&win.readers.group);
    page.add(&win.tokens.group);
    page.add(&win.drivers.group);
    page.add(&apps_group(&win));

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    win.toasts.set_child(Some(&view));
    win.window.set_content(Some(&win.toasts));

    {
        let (hub, w) = (hub.clone(), Rc::downgrade(&win));
        win.service_button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                hub.start_server();
                fill_drivers(&hub, &win);
                win.last.borrow_mut().clear();
                refresh(&hub, &win);
            }
        });
    }
    {
        let (hub, w) = (hub.clone(), Rc::downgrade(&win));
        refresh_button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                win.last.borrow_mut().clear();
                refresh(&hub, &win);
            }
        });
    }

    *hub.win.borrow_mut() = Some(win.clone());
    {
        // Closing the window leaves the service running: forget the window
        // (a closed window cannot be shown again) and show the tray at once.
        let hub = hub.clone();
        win.window.connect_close_request(move |_| {
            hub.win.borrow_mut().take();
            let hub = hub.clone();
            glib::idle_add_local_once(move || update_status(&hub));
            glib::Propagation::Proceed
        });
    }
    {
        let hub = hub.clone();
        win.window.connect_show(move |_| update_status(&hub));
    }
    fill_drivers(hub, &win);
    refresh(hub, &win);
    win
}

/// Open the window, or bring it to the front.
fn present(app: &adw::Application, hub: &Rc<Hub>) {
    let existing = hub.win.borrow().clone();
    match existing {
        Some(win) => win.window.present(),
        None => build_window(app, hub).window.present(),
    }
}

/// Checks run every few seconds with the window open, and less often
/// without it (still often enough to publish a token inserted meanwhile).
fn start_supervision(hub: &Rc<Hub>) {
    let hub = hub.clone();
    let tick = Cell::new(0u32);
    glib::timeout_add_local(Duration::from_secs(4), move || {
        tick.set(tick.get().wrapping_add(1));
        let win = hub.win.borrow().clone();
        match win {
            Some(win) if win.window.is_visible() => refresh(&hub, &win),
            _ if tick.get() % 4 == 0 => supervise(&hub),
            _ => {}
        }
        glib::ControlFlow::Continue
    });
}

async fn request_background(autostart: bool) -> ashpd::Result<bool> {
    let response = ashpd::desktop::background::Background::request()
        .reason("Keep the token service running so other apps can use your tokens")
        .auto_start(autostart)
        .command(["tessera", "--background"])
        .dbus_activatable(false)
        .send()
        .await?
        .response()?;
    Ok(response.auto_start())
}

fn setup_actions(app: &adw::Application, hub: &Rc<Hub>) {
    let autostart = gio::SimpleAction::new_stateful(
        "autostart",
        None,
        &hub.settings.borrow().autostart.to_variant(),
    );
    {
        let hub = hub.clone();
        autostart.connect_activate(move |action, _| {
            let want = !action
                .state()
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            let action = action.clone();
            let hub = hub.clone();
            glib::spawn_future_local(async move {
                match request_background(want).await {
                    Ok(granted) => {
                        action.set_state(&granted.to_variant());
                        hub.settings.borrow_mut().autostart = granted;
                        hub.settings.borrow().save();
                    }
                    Err(e) => {
                        eprintln!("tessera: background portal: {e}");
                        let win = hub.win.borrow().clone();
                        if let Some(win) = win {
                            win.alert(t("Could not turn on starting at login"), &format!("{e}"));
                        }
                    }
                }
            });
        });
    }
    app.add_action(&autostart);

    let about = gio::SimpleAction::new("about", None);
    {
        let app2 = app.clone();
        about.connect_activate(move |_, _| {
            let dialog = adw::AboutDialog::builder()
                .application_name("Tessera")
                .application_icon(paths::APP_ID)
                .developer_name("Pedro Henrique Quitete Barreto")
                .version(env!("CARGO_PKG_VERSION"))
                .comments(t("Token hub"))
                .website("https://github.com/pedrohqb/tessera")
                .issue_url("https://github.com/pedrohqb/tessera/issues")
                .license_type(gtk::License::Gpl30)
                .build();
            dialog.present(app2.active_window().as_ref());
        });
    }
    app.add_action(&about);

    let quit = gio::SimpleAction::new("quit", None);
    {
        let (hub, app2) = (hub.clone(), app.clone());
        quit.connect_activate(move |_, _| {
            hub.shut_down();
            hub._hold.borrow_mut().take();
            app2.quit();
        });
    }
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
}

pub fn run() -> ExitCode {
    let app = adw::Application::builder()
        .application_id(paths::APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.add_main_option(
        "background",
        glib::Char::from(b'b'),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Start the token service without opening a window",
        None,
    );

    let hub: Rc<RefCell<Option<Rc<Hub>>>> = Rc::new(RefCell::new(None));
    {
        let hub = hub.clone();
        app.connect_startup(move |app| {
            let (tx, rx) = async_channel::unbounded();
            let h = Rc::new(Hub {
                server: RefCell::new(Server::new()),
                drivers: RefCell::new(vec![]),
                settings: RefCell::new(Settings::load()),
                host: RefCell::new(Ok(0)),
                _hold: RefCell::new(Some(app.hold())),
                generation: Cell::new(0),
                health: RefCell::new(crate::Health::default()),
                busy: Cell::new(false),
                tokens: Cell::new(0),
                published: RefCell::new(vec![]),
                host_view: RefCell::new(None),
                host_checked: RefCell::new(None),
                host_busy: Cell::new(false),
                win: RefCell::new(None),
                tray: RefCell::new(None),
                tray_events: tx,
                status_sent: RefCell::new(String::new()),
            });
            h.start_server();
            setup_actions(app, &h);
            start_supervision(&h);
            supervise(&h);
            {
                let (h, app) = (h.clone(), app.clone());
                glib::spawn_future_local(async move {
                    while let Ok(ev) = rx.recv().await {
                        match ev {
                            tray::Event::Open => present(&app, &h),
                            tray::Event::Quit => app.activate_action("quit", None),
                        }
                    }
                });
            }
            *hub.borrow_mut() = Some(h);
        });
    }
    {
        let hub = hub.clone();
        app.connect_command_line(move |app, cmdline| {
            let background = cmdline.options_dict().contains("background");
            let Some(h) = hub.borrow().clone() else {
                return glib::ExitCode::FAILURE;
            };
            if !background {
                present(app, &h);
            }
            glib::ExitCode::SUCCESS
        });
    }
    {
        let hub = hub.clone();
        app.connect_shutdown(move |_| {
            if let Some(h) = hub.borrow().as_ref() {
                h.shut_down();
            }
        });
    }
    let code = app.run();
    ExitCode::from(code.get())
}
