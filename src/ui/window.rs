//! The window: service status, readers, tokens and drivers. The app rows
//! are in app_rows.rs.

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use super::app_rows::{apps_group, update_apps, AppRows, NativeRows};
use super::supervise::{supervise, update_status};
use super::Hub;
use crate::apps;
use crate::catalog;
use crate::drivers;
use crate::i18n::t;
use crate::paths;
use crate::server::State;
use crate::status::{self, Readers, Snapshot};

/// A preferences group whose rows are rebuilt on refresh.
pub(super) struct Section {
    pub(super) group: adw::PreferencesGroup,
    pub(super) rows: RefCell<Vec<gtk::Widget>>,
}

impl Section {
    pub(super) fn new(title: &str, description: Option<&str>) -> Self {
        let group = adw::PreferencesGroup::builder().title(title).build();
        if let Some(d) = description {
            group.set_description(Some(d));
        }
        Self {
            group,
            rows: RefCell::new(vec![]),
        }
    }

    pub(super) fn clear(&self) {
        for w in self.rows.borrow_mut().drain(..) {
            self.group.remove(&w);
        }
    }

    pub(super) fn add(&self, w: &impl IsA<gtk::Widget>) {
        self.group.add(w);
        self.rows.borrow_mut().push(w.clone().upcast());
    }
}

pub(super) struct Win {
    pub(super) window: adw::ApplicationWindow,
    pub(super) toasts: adw::ToastOverlay,
    pub(super) service_row: adw::ActionRow,
    pub(super) service_button: gtk::Button,
    pub(super) host_row: adw::ActionRow,
    pub(super) pcsc_row: adw::ActionRow,
    pub(super) readers: Section,
    pub(super) tokens: Section,
    pub(super) drivers: Section,
    /// (driver id, its row, the row's own subtitle) for update_drivers.
    pub(super) driver_rows: RefCell<Vec<(String, adw::ActionRow, String)>>,
    pub(super) app_rows: RefCell<Vec<AppRows>>,
    pub(super) native_rows: RefCell<Vec<NativeRows>>,
    pub(super) last: RefCell<String>,
    pub(super) old_p11kit: adw::Banner,
}

impl Win {
    pub(super) fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    /// For messages too long for a toast (what went wrong and what to do).
    pub(super) fn alert(&self, heading: &str, body: &str) {
        let dialog = adw::AlertDialog::new(Some(heading), Some(body));
        dialog.add_response("ok", "OK");
        dialog.present(Some(&self.window));
    }
}

pub(super) fn plain_row(title: &str, subtitle: &str, icon: &str) -> adw::ActionRow {
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

/// Let the icon theme find Tessera's own icons wherever it is unpacked, and
/// the host's own icon themes, where Flatpak shows them, for the native
/// apps' icons. The Flatpak apps' icons come from
/// Flatpak's AppStream data (see app_rows.rs), which needs no access to the
/// apps' own folders.
pub(super) fn add_host_icons() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    // Tessera's own icons next to its executable: in FlatPark's package the
    // release tree is unpacked under /app/extra/tessera, outside /app/share,
    // where the theme would not look (in Tessera's own Flatpak this is
    // /app/share/icons, which the theme already has).
    let own = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("share/icons")));
    let host =
        ["/run/host/share/icons", "/run/host/user-share/icons"].map(std::path::PathBuf::from);
    for dir in own.into_iter().chain(host) {
        if dir.is_dir() {
            theme.add_search_path(dir);
        }
    }
}

pub(super) fn status_page_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = plain_row(title, subtitle, "");
    row.add_css_class("dim-label");
    row
}

pub(super) fn copy_button(win: &Rc<Win>, text: String) -> gtk::Button {
    copy_button_with(
        win,
        text,
        t("Copy the command that lets this app reach Tessera"),
    )
}

/// A button that copies a driver's install command.
pub(super) fn copy_command_button(win: &Rc<Win>, text: String) -> gtk::Button {
    copy_button_with(win, text, t("Copy the command that installs this driver"))
}

/// A button that opens a driver's page on FlatPark's site, whose install
/// button hands the extension to the app store.
fn store_button(win: &Rc<Win>, k: &'static catalog::Known) -> gtk::Button {
    let b = gtk::Button::builder()
        .icon_name("web-browser-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text(t(
            "Open this driver's page on FlatPark, to install it from the app store",
        ))
        .css_classes(["flat"])
        .build();
    let w = Rc::downgrade(win);
    b.connect_clicked(move |_| {
        let parent = w.upgrade().map(|win| win.window.clone());
        gtk::UriLauncher::new(&catalog::store_page(k)).launch(
            parent.as_ref(),
            gio::Cancellable::NONE,
            |_| {},
        );
    });
    b
}

fn copy_button_with(win: &Rc<Win>, text: String, tooltip: &str) -> gtk::Button {
    let b = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text(tooltip)
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

/// On systems whose p11-kit is too old for signing (native.rs), a banner
/// that says so, with a button to the README's list of distributions. It is
/// shown or hidden again in update_service, once the system's server has
/// told its protocol version.
fn old_p11kit_banner() -> adw::Banner {
    let banner = adw::Banner::new(t(
        "On this system signing does not work (old p11-kit); see the project's documentation",
    ));
    banner.set_button_label(Some(t("Documentation")));
    banner.set_revealed(crate::native::host_has_old_p11kit(None));
    banner
}

fn connect_old_p11kit_banner(win: &Rc<Win>) {
    let w = Rc::downgrade(win);
    win.old_p11kit.connect_button_clicked(move |_| {
        let parent = w.upgrade().map(|win| win.window.clone());
        gtk::UriLauncher::new(t("https://github.com/pedrohqb/tessera#distributions")).launch(
            parent.as_ref(),
            gio::Cancellable::NONE,
            |_| {},
        );
    });
}

/// What to tell the user to run for the host's p11-kit server: the command
/// itself, or, where it is long (Debian, Ubuntu), the copy button.
fn run_once(lead: &'static str, copy: &'static str) -> String {
    if apps::host_server_command_is_long() {
        t(copy).to_string()
    } else {
        format!("{} {}", t(lead), apps::host_server_command())
    }
}

pub(super) fn update_service(hub: &Hub, win: &Win) {
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
    win.old_p11kit
        .set_revealed(crate::native::host_has_old_p11kit(hub.host_protocol.get()));
    win.service_button.set_label(button);
    if warn {
        win.service_row.add_css_class("error");
    } else {
        win.service_row.remove_css_class("error");
    }
    let off = run_once(
        "Off: the apps cannot see the tokens. Run once in a terminal:",
        "Off: the apps cannot see the tokens. Copy the command with the button on the right and run it once in a terminal.",
    );
    let (text, bad) = match (&*hub.host.borrow(), &*hub.host_view.borrow()) {
        (Err(e), _) => (format!("{}: {e}", t("Could not write the host module files")), true),
        (Ok(_), Some(status::HostView::Sees(n, twice))) if twice.is_empty() => (
            format!("{} · {n} {}", t("On"), t("token(s) of Tessera seen by the apps")),
            false,
        ),
        (Ok(_), Some(status::HostView::Sees(n, twice))) => (
            format!(
                "{} · {n} {} · {}: {}",
                t("On"),
                t("token(s) of Tessera seen by the apps"),
                t("these show twice to the apps, because the system has a driver of its own for them too (its OpenSC, for example); switch off Tessera's driver for them below, or remove the system's"),
                twice
                    .iter()
                    .map(|(label, _)| label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            true,
        ),
        (Ok(n), Some(status::HostView::NothingToCheck)) => (
            format!("{} · {n} {}", t("On"), t("driver(s) published; no token to check yet")),
            false,
        ),
        (Ok(_), Some(status::HostView::WithoutTessera)) => (
            run_once(
                "The apps do not see Tessera's tokens: what answers at the system connection does not list them (the system's p11-kit server is probably off, and Flatpak's own answers in its place). Run once in a terminal:",
                "The apps do not see Tessera's tokens: what answers at the system connection does not list them (the system's p11-kit server is probably off, and Flatpak's own answers in its place). Copy the command with the button on the right and run it once in a terminal.",
            ),
            true,
        ),
        (Ok(_), Some(status::HostView::Unknown)) => (t("Checking…").to_string(), false),
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

pub(super) fn apply_snapshot(win: &Rc<Win>, snap: Snapshot, installed: &[String]) {
    let fingerprint = format!("{snap:?}{installed:?}");
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
                let row = plain_row(&r.name, state, "media-removable-symbolic");
                // A token whose vendor driver is not installed: which one,
                // and the command that installs it.
                let need = r
                    .card_present
                    .then(|| catalog::for_reader(&r.name, &r.atr))
                    .flatten()
                    .filter(|k| !installed.iter().any(|id| id == k.id))
                    // Another driver (the built-in OpenSC, for IDPrime
                    // cards) may already serve it.
                    .filter(|_| !status::served_in_reader(&snap.tokens, &r.name));
                if let Some(k) = need {
                    row.set_subtitle(&format!(
                        "{state} · {}: {}. {}",
                        t("It looks like a token for the driver"),
                        k.name,
                        t("If it does not show under Tokens, install that driver: from the app store or FlatPark's page (button), or with the command (copy button); then quit Tessera from its menu and open it again")
                    ));
                    row.add_css_class("warning");
                    row.add_suffix(&store_button(win, k));
                    row.add_suffix(&copy_command_button(win, catalog::install_command(k)));
                }
                win.readers.add(&row);
            }
        }
    }

    win.tokens.clear();
    for (token, a, b) in status::same_token_twice(&snap.tokens) {
        let row = status_page_row(
            t("The same token comes from two drivers"),
            &format!(
                "{token}: {a} · {b}. {}",
                t("The apps would list it twice: switch one of the two drivers off below")
            ),
        );
        row.add_css_class("warning");
        win.tokens.add(&row);
    }
    for e in &snap.errors {
        let row = status_page_row(t("A driver could not be read"), e);
        row.add_css_class("error");
        win.tokens.add(&row);
    }
    super::app_rows::set_token_choices(win, &snap.tokens);
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

pub(super) fn refresh(hub: &Rc<Hub>, win: &Rc<Win>) {
    update_service(hub, win);
    supervise(hub);
}

/// Each driver row says whether apps can use it, and why not.
pub(super) fn update_drivers(hub: &Hub, win: &Win) {
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

pub(super) fn fill_drivers(hub: &Rc<Hub>, win: &Rc<Win>) {
    win.drivers.clear();
    win.driver_rows.borrow_mut().clear();
    let ds = hub.drivers.borrow();
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
    // The vendors' drivers not installed: what each is for, and the command
    // that installs it (they are Flatpak extensions, chosen by the user).
    for k in catalog::missing(&ds) {
        let row = plain_row(
            k.name,
            &format!(
                "{} · {} · {}\n{}",
                t("Not installed"),
                k.vendor,
                t(k.tokens),
                t("Install it from the app store (an add-on on Tessera's page) or FlatPark's page (button), or with the command (copy button); then restart Tessera (menu)")
            ),
            "application-x-addon-symbolic",
        );
        row.add_css_class("dim-label");
        row.set_tooltip_text(Some(&catalog::install_command(k)));
        row.add_suffix(&store_button(win, k));
        row.add_suffix(&copy_command_button(win, catalog::install_command(k)));
        win.drivers.add(&row);
    }
    drop(ds);
    update_drivers(hub, win);
}

pub(super) fn build_window(app: &adw::Application, hub: &Rc<Hub>) -> Rc<Win> {
    add_host_icons();
    let header = adw::HeaderBar::new();
    let menu = gio::Menu::new();
    menu.append(
        Some(t("Start automatically at login")),
        Some("app.autostart"),
    );
    menu.append(Some(t("Show an icon in the tray")), Some("app.tray"));
    menu.append(Some(t("Restart Tessera")), Some("app.restart"));
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
        app_rows: RefCell::new(vec![]),
        native_rows: RefCell::new(vec![]),
        last: RefCell::new(String::new()),
        old_p11kit: old_p11kit_banner(),
    });
    connect_old_p11kit_banner(&win);
    win.host_row
        .add_suffix(&copy_button(&win, apps::host_server_command().to_string()));
    page.add(&win.readers.group);
    page.add(&win.tokens.group);
    page.add(&win.drivers.group);
    page.add(&apps_group(&win));

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&win.old_p11kit);
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
                // Everything again: the tokens, what the apps see through
                // the system (now, not at the next 15 s check) and each
                // app's steps.
                win.last.borrow_mut().clear();
                *hub.host_checked.borrow_mut() = None;
                update_apps(&win);
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
