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

/// State that outlives any window: the server keeps running in the
/// background when the window is closed.
struct Hub {
    server: RefCell<Server>,
    drivers: RefCell<Vec<Driver>>,
    settings: RefCell<Settings>,
    _hold: RefCell<Option<gio::ApplicationHoldGuard>>,
}

impl Hub {
    fn start_server(&self) {
        let ds = drivers::discover();
        let result = self.server.borrow_mut().start(&ds);
        *self.drivers.borrow_mut() = ds;
        if let Err(e) = result {
            self.server.borrow_mut().set_failure(format!("{e:#}"));
        }
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
    pcsc_row: adw::ActionRow,
    readers: Section,
    tokens: Section,
    drivers: Section,
    last: RefCell<String>,
    busy: Cell<bool>,
}

impl Win {
    fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
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
    let socket = paths::socket_path();
    let (subtitle, button, warn) = match &state {
        State::Running => (format!("{} · {}", t("Running"), socket.display()), t("Restart"), false),
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
    match snap.tokens {
        Err(e) => win.tokens.add(&status_page_row(t("No token found"), &e)),
        Ok(tokens) if tokens.is_empty() => win.tokens.add(&status_page_row(
            t("No token found"),
            t("Insert a token, or install the driver for it below."),
        )),
        Ok(tokens) => {
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
                    .subtitle(format!("{} {} · {count}", tok.manufacturer, tok.model).trim())
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
    if win.busy.replace(true) {
        return;
    }
    let win = win.clone();
    glib::spawn_future_local(async move {
        let snap = gio::spawn_blocking(status::snapshot).await;
        win.busy.set(false);
        if let Ok(snap) = snap {
            apply_snapshot(&win, snap);
        }
    });
}

fn fill_drivers(hub: &Hub, win: &Rc<Win>) {
    win.drivers.clear();
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
            let w = Rc::downgrade(win);
            b.connect_clicked(move |_| {
                let Some((prog, args)) = tool.exec.split_first() else {
                    return;
                };
                if let Err(e) = Command::new(prog).args(args).envs(&tool.env).spawn() {
                    if let Some(win) = w.upgrade() {
                        win.toast(&format!("{}: {e}", tool.name));
                    }
                }
            });
            row.add_suffix(&b);
        }
        win.drivers.add(&row);
    }
}

/// Which NSS database, if any, also needs the proxy module.
#[derive(Clone, Copy)]
enum NssTarget {
    /// Ask the user for a Firefox profile folder.
    FirefoxProfile,
    /// The database Tessera keeps for Papers.
    Papers,
}

fn nss_toast(
    win: &Win,
    res: Result<anyhow::Result<nss::Outcome>, Box<dyn std::any::Any + Send>>,
    done: &str,
) {
    match res {
        Ok(Ok(_)) => win.toast(done),
        Ok(Err(e)) => win.toast(&format!("{}: {e:#}", t("Could not set up"))),
        Err(_) => win.toast(t("Could not set up")),
    }
}

fn set_up_app(
    hub: &Rc<Hub>,
    win: &Rc<Win>,
    app_id: &'static str,
    nss_target: NssTarget,
    status: adw::ExpanderRow,
) {
    if !matches!(hub.server.borrow_mut().poll(), State::Running) {
        win.toast(t("Start the token service first"));
        return;
    }
    if let Err(e) = apps::write_module_file(app_id) {
        win.toast(&format!("{}: {e:#}", t("Could not set up")));
        return;
    }
    status.set_subtitle(&format!("{app_id} · {}", t("Configured")));
    match nss_target {
        NssTarget::Papers => {
            let win = win.clone();
            glib::spawn_future_local(async move {
                let res = gio::spawn_blocking(|| nss::add_module(&paths::papers_nss_dir())).await;
                nss_toast(&win, res, t("Papers is set up"));
            });
        }
        NssTarget::FirefoxProfile => {
            let dialog = gtk::FileDialog::builder()
                .title(t("Choose the Firefox profile folder"))
                .modal(true)
                .build();
            if let Some(root) = paths::firefox_profile_roots()
                .into_iter()
                .find(|p| p.is_dir())
            {
                dialog.set_initial_folder(Some(&gio::File::for_path(root)));
            }
            let w = Rc::downgrade(win);
            dialog.select_folder(Some(&win.window), gio::Cancellable::NONE, move |res| {
                let Some(win) = w.upgrade() else { return };
                let Some(dir) = res.ok().and_then(|f| f.path()) else {
                    win.toast(t("Load the module by hand in Firefox (see below)"));
                    return;
                };
                if nss::profile_in_use(&dir) {
                    win.toast(t("Close Firefox first, then try again"));
                    return;
                }
                glib::spawn_future_local(async move {
                    let res = gio::spawn_blocking(move || nss::add_module(&dir)).await;
                    nss_toast(&win, res, t("Firefox profile is set up"));
                });
            });
        }
    }
}

fn client_row(
    hub: &Rc<Hub>,
    win: &Rc<Win>,
    app: &'static apps::ClientApp,
    icon: &str,
    nss_target: NssTarget,
) -> adw::ExpanderRow {
    let state = if apps::module_file_present(app.id) {
        t("Configured")
    } else {
        t("Not set up")
    };
    let row = adw::ExpanderRow::builder()
        .use_markup(false)
        .title(app.name)
        .subtitle(format!("{} · {state}", app.id))
        .build();
    row.add_prefix(&gtk::Image::from_icon_name(icon));

    let cmd = apps::override_command(app.id);
    let step1 = adw::ActionRow::builder()
        .use_markup(false)
        .title(t("1. Let the app reach Tessera (run once in a terminal)"))
        .subtitle(&cmd)
        .subtitle_selectable(true)
        .build();
    step1.add_suffix(&copy_button(win, cmd));
    row.add_row(&step1);

    let step2 = adw::ActionRow::builder()
        .use_markup(false)
        .title(t("2. Set up the app"))
        .subtitle(match nss_target {
            NssTarget::FirefoxProfile => t("Close Firefox, then choose its profile folder"),
            NssTarget::Papers => t("Papers will find the token's certificates when signing"),
        })
        .build();
    let button = gtk::Button::builder()
        .label(t("Set up"))
        .valign(gtk::Align::Center)
        .build();
    {
        let (hub, w, status) = (hub.clone(), Rc::downgrade(win), row.clone());
        button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                set_up_app(&hub, &win, app.id, nss_target, status.clone());
            }
        });
    }
    step2.add_suffix(&button);
    row.add_row(&step2);

    if matches!(nss_target, NssTarget::FirefoxProfile) {
        let manual = adw::ActionRow::builder()
            .use_markup(false)
            .title(t("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:"))
            .subtitle(paths::PROXY_MODULE)
            .subtitle_selectable(true)
            .build();
        row.add_row(&manual);
    }
    row
}

fn apps_group(hub: &Rc<Hub>, win: &Rc<Win>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t("Apps"))
        .description(t("Apps that use the tokens through Tessera."))
        .build();
    group.add(&client_row(
        hub,
        win,
        &apps::FIREFOX,
        "web-browser-symbolic",
        NssTarget::FirefoxProfile,
    ));
    group.add(&client_row(
        hub,
        win,
        &apps::PAPERS,
        "x-office-document-symbolic",
        NssTarget::Papers,
    ));

    // PJeOffice Pro (FlatPark) loads p11-kit-client.so through its own bridge,
    // at the p11-kit default socket; it needs its package to point at
    // Tessera's socket instead.
    let pje = plain_row(
        apps::PJEOFFICE.name,
        t("Waiting for its FlatPark package to support Tessera"),
        "dialog-information-symbolic",
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
        pcsc_row,
        readers: Section::new(t("Readers"), None),
        tokens: Section::new(t("Tokens"), None),
        drivers: Section::new(t("Drivers"), None),
        last: RefCell::new(String::new()),
        busy: Cell::new(false),
    });
    page.add(&win.readers.group);
    page.add(&win.tokens.group);
    page.add(&win.drivers.group);
    page.add(&apps_group(hub, &win));

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

    fill_drivers(hub, &win);
    refresh(hub, &win);
    let (hub2, w) = (hub.clone(), Rc::downgrade(&win));
    glib::timeout_add_local(Duration::from_secs(3), move || match w.upgrade() {
        Some(win) if win.window.is_visible() => {
            refresh(&hub2, &win);
            glib::ControlFlow::Continue
        }
        Some(_) => glib::ControlFlow::Continue,
        None => glib::ControlFlow::Break,
    });
    win
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
        let app2 = app.clone();
        autostart.connect_activate(move |action, _| {
            let want = !action
                .state()
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            let action = action.clone();
            let hub = hub.clone();
            let app2 = app2.clone();
            glib::spawn_future_local(async move {
                match request_background(want).await {
                    Ok(granted) => {
                        action.set_state(&granted.to_variant());
                        hub.settings.borrow_mut().autostart = granted;
                        hub.settings.borrow().save();
                    }
                    Err(e) => {
                        eprintln!("tessera: background portal: {e}");
                        if let Some(win) = app2.active_window() {
                            if let Some(overlay) = win
                                .child()
                                .and_then(|c| c.downcast::<adw::ToastOverlay>().ok())
                            {
                                overlay.add_toast(adw::Toast::new(&format!("{e}")));
                            }
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
            hub.server.borrow_mut().stop();
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
            let h = Rc::new(Hub {
                server: RefCell::new(Server::new()),
                drivers: RefCell::new(vec![]),
                settings: RefCell::new(Settings::load()),
                _hold: RefCell::new(Some(app.hold())),
            });
            h.start_server();
            setup_actions(app, &h);
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
                match app.active_window() {
                    Some(w) => w.present(),
                    None => build_window(app, &h).window.present(),
                }
            }
            glib::ExitCode::SUCCESS
        });
    }
    {
        let hub = hub.clone();
        app.connect_shutdown(move |_| {
            if let Some(h) = hub.borrow().as_ref() {
                h.server.borrow_mut().stop();
            }
        });
    }
    let code = app.run();
    ExitCode::from(code.get())
}
