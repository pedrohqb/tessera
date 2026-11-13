//! The window: service status, readers, tokens and drivers. The app rows
//! are in app_rows.rs.

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

use super::app_rows::{apps_group, update_apps, AppRows, NativeRows};
use super::signer_rows::{signers_group, BrowserRows};
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
    pub(super) signer_rows: RefCell<Vec<BrowserRows>>,
    pub(super) last: RefCell<String>,
    pub(super) old_p11kit: adw::ActionRow,
    /// The drivers not installed (Drivers page).
    pub(super) available: Section,
    /// Home's first row: everything works, or what needs attention.
    pub(super) summary_row: adw::ActionRow,
    summary_icon: gtk::Image,
    summary_button: gtk::Button,
    /// The sidebar: the tokens found (Home) and a warning on System.
    token_badge: gtk::Label,
    system_alert: gtk::Image,
    split: adw::NavigationSplitView,
    content_page: adw::NavigationPage,
    sidebar: gtk::ListBox,
    stack: gtk::Stack,
    /// What needs attention, from update_service and apply_snapshot.
    service_bad: Cell<bool>,
    host_bad: Cell<bool>,
    pcsc_bad: Cell<bool>,
    tokens_found: Cell<usize>,
}

impl Win {
    /// A toast with the text as is: a toast's title is Pango markup by
    /// default, and the commands it shows (`&&`, `>`) are not valid markup,
    /// which left it blank.
    pub(super) fn toast(&self, text: &str) {
        self.toasts
            .add_toast(adw::Toast::builder().title(text).use_markup(false).build());
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
    store_button_for(
        win,
        catalog::store_page(k),
        t("Open this driver's page on FlatPark, to install it from the app store"),
    )
}

/// A button that opens an extension's page on FlatPark's site.
pub(super) fn store_button_for(win: &Rc<Win>, url: String, tooltip: &str) -> gtk::Button {
    let b = gtk::Button::builder()
        .icon_name("web-browser-symbolic")
        .valign(gtk::Align::Center)
        .tooltip_text(tooltip)
        .css_classes(["flat"])
        .build();
    let w = Rc::downgrade(win);
    b.connect_clicked(move |_| {
        let parent = w.upgrade().map(|win| win.window.clone());
        gtk::UriLauncher::new(&url).launch(parent.as_ref(), gio::Cancellable::NONE, |_| {});
    });
    b
}

pub(super) fn copy_button_with(win: &Rc<Win>, text: String, tooltip: &str) -> gtk::Button {
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

/// On systems whose p11-kit is too old for signing (native.rs), a row on
/// Home that says so, with a button to the README's list of distributions.
/// It is shown or hidden again in update_service, once the system's server
/// has told its protocol version. (A banner over the window does not wrap
/// its text, and made the window as wide as the sentence.)
fn old_p11kit_banner() -> adw::ActionRow {
    let row = plain_row(
        t("On this system signing does not work (old p11-kit); see the project's documentation"),
        "",
        "dialog-error-symbolic",
    );
    row.add_css_class("error");
    row.set_visible(crate::native::host_has_old_p11kit(None));
    row
}

fn connect_old_p11kit_banner(win: &Rc<Win>) {
    let b = gtk::Button::builder()
        .label(t("Documentation"))
        .valign(gtk::Align::Center)
        .build();
    win.old_p11kit.add_suffix(&b);
    let w = Rc::downgrade(win);
    b.connect_clicked(move |_| {
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
        .set_visible(crate::native::host_has_old_p11kit(hub.host_protocol.get()));
    win.service_button.set_label(button);
    win.service_bad.set(!matches!(state, State::Running));
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
                t("these show twice to the apps, because the system has a driver of its own for them too (its OpenSC, for example); switch off Tessera's driver for them on the Drivers page, or remove the system's"),
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
    win.host_bad.set(bad);
    if bad {
        win.host_row.add_css_class("error");
    } else {
        win.host_row.remove_css_class("error");
    }
    update_summary(win);
}

/// Home's first row and the sidebar's marks: everything works, or which
/// parts of the System page need attention.
fn update_summary(win: &Win) {
    let mut wrong = Vec::new();
    if win.service_bad.get() {
        wrong.push(t("Token service"));
    }
    if win.host_bad.get() {
        wrong.push(t("System connection"));
    }
    if win.pcsc_bad.get() {
        wrong.push(t("Smart card service (pcscd)"));
    }
    let n = win.tokens_found.get();
    win.token_badge.set_label(&n.to_string());
    win.token_badge.set_visible(n > 0);
    win.system_alert.set_visible(!wrong.is_empty());
    let tokens = match n {
        0 => t("No token connected").to_string(),
        1 => format!("1 {}", t("token found")),
        n => format!("{n} {}", t("tokens found")),
    };
    if wrong.is_empty() {
        win.summary_row.set_title(t("Everything works"));
        win.summary_row.set_subtitle(&tokens);
        win.summary_icon
            .set_icon_name(Some("object-select-symbolic"));
        win.summary_row.remove_css_class("warning");
        win.summary_button.set_visible(false);
    } else {
        win.summary_row.set_title(t("Needs attention"));
        win.summary_row
            .set_subtitle(&format!("{} · {tokens}", wrong.join(" · ")));
        win.summary_icon
            .set_icon_name(Some("dialog-warning-symbolic"));
        win.summary_row.add_css_class("warning");
        win.summary_button.set_visible(true);
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
            win.pcsc_bad.set(true);
            win.readers.add(&status_page_row(
                t("No reader found"),
                t("Not reachable: is pcscd installed and running on the system?"),
            ));
        }
        Readers::Ok(list) => {
            win.pcsc_row.set_subtitle(t("Reachable"));
            win.pcsc_row.remove_css_class("error");
            win.pcsc_bad.set(false);
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
                t("The apps would list it twice: switch one of the two drivers off on the Drivers page")
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
    win.tokens_found.set(snap.tokens.len());
    update_summary(win);
    match snap.tokens {
        tokens if tokens.is_empty() => win.tokens.add(&status_page_row(
            t("No token found"),
            t("Insert a token, or install the driver for it on the Drivers page."),
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
        let retiring = hub.server.borrow().retiring();
        let (state, bad) = if disabled.contains(id) && retiring.contains(id) {
            (
                format!(
                    "{} · {}",
                    t("Switched off"),
                    t("still serving the apps that had it open, until they close")
                ),
                false,
            )
        } else if disabled.contains(id) {
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
    win.available.clear();
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
        if catalog::known(&d.id).is_some_and(|k| k.experimental) {
            row.add_suffix(&experimental_tag());
        }
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
                let on = sw.is_active();
                hub.set_driver_enabled(&id, on);
                if let Some(win) = w.upgrade() {
                    // p11-kit reads the list of drivers when an app
                    // connects; and a driver an app is using keeps serving
                    // it (Server::set_enabled).
                    win.toast(if on {
                        t("Driver on. Apps that were already open see it once reopened.")
                    } else {
                        t("Driver off. Apps that were already open keep it until they close.")
                    });
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
            &format!("{} · {}", k.vendor, t(k.tokens)),
            "application-x-addon-symbolic",
        );
        row.set_tooltip_text(Some(&format!(
            "{}\n{}",
            t("Install it from the app store (an add-on on Tessera's page) or FlatPark's page (button), or with the command (copy button); then restart Tessera (menu)"),
            catalog::install_command(k)
        )));
        if k.experimental {
            row.add_suffix(&experimental_tag());
        }
        row.add_suffix(&store_button(win, k));
        row.add_suffix(&copy_command_button(win, catalog::install_command(k)));
        win.available.add(&row);
    }
    win.available
        .group
        .set_visible(!win.available.rows.borrow().is_empty());
    win.drivers
        .group
        .set_visible(!win.drivers.rows.borrow().is_empty());
    drop(ds);
    update_drivers(hub, win);
}

/// The mark of a driver still being tried out (catalog::Known::experimental).
fn experimental_tag() -> gtk::Label {
    gtk::Label::builder()
        .label(t("Experimental"))
        .css_classes(["caption-heading", "warning"])
        .valign(gtk::Align::Center)
        .tooltip_text(t("Still being tried out: it may not work with every site or app. Problems go to its developer."))
        .build()
}

/// The system's own OpenSC: off by default (see apps::set_host_opensc_off),
/// even with Tessera closed; the switch turns it back on.
fn system_opensc_row(hub: &Rc<Hub>) -> adw::ActionRow {
    let row = plain_row(
        t("Switch off the system's OpenSC"),
        t("Recommended: it hangs some tokens (GD Burti) and shows others twice"),
        "action-unavailable-symbolic",
    );
    row.set_tooltip_text(Some(t("Recommended. If the system has OpenSC of its own, it hangs some tokens (GD Burti) and shows the others twice; Tessera's OpenSC serves the same cards. Stays off with Tessera closed. Restart the apps that were open.")));
    let switch = gtk::Switch::builder()
        .active(!hub.settings.borrow().system_opensc_on)
        .valign(gtk::Align::Center)
        .build();
    let hub = hub.clone();
    switch.connect_active_notify(move |sw| {
        let off = sw.is_active();
        {
            let mut st = hub.settings.borrow_mut();
            st.system_opensc_on = !off;
            st.save();
        }
        if let Err(e) = apps::set_host_opensc_off(off) {
            if let Some(win) = hub.win.borrow().as_ref() {
                win.alert(t("Could not change the system's OpenSC"), &format!("{e:#}"));
            }
        }
        // What the apps see changes: check it again.
        *hub.host_checked.borrow_mut() = None;
    });
    row.add_suffix(&switch);
    row.set_activatable_widget(Some(&switch));
    row
}

/// A page of the window: its groups in a column that grows to 900 px (an
/// AdwPreferencesPage stops at 600, which left most of a maximized window
/// empty).
fn page_column() -> (gtk::ScrolledWindow, gtk::Box) {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    let clamp = adw::Clamp::builder()
        .maximum_size(960)
        .tightening_threshold(700)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();
    (scroll, column)
}

/// One entry of the sidebar: an icon, the page's name and, at the end,
/// a mark (the tokens found, a warning).
fn sidebar_row(name: &str, icon: &str, mark: Option<&gtk::Widget>) -> gtk::ListBoxRow {
    let line = gtk::Box::builder()
        .spacing(12)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    line.append(&gtk::Image::from_icon_name(icon));
    line.append(
        &gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    if let Some(m) = mark {
        line.append(m);
    }
    let row = gtk::ListBoxRow::builder().child(&line).build();
    row.set_widget_name(name);
    row
}

/// The pages, in the sidebar's order: (stack name, title, icon).
const PAGES: [(&str, &str, &str); 4] = [
    ("home", "Home", "go-home-symbolic"),
    ("drivers", "Drivers", "application-x-addon-symbolic"),
    ("apps", "Apps", "view-app-grid-symbolic"),
    ("system", "System", "emblem-system-symbolic"),
];

/// Show a page: the stack, the content's title and the sidebar's choice.
pub(super) fn show_page(win: &Win, name: &str) {
    let Some(i) = PAGES.iter().position(|(n, _, _)| *n == name) else {
        return;
    };
    win.stack.set_visible_child_name(name);
    win.content_page.set_title(t(PAGES[i].1));
    if let Some(row) = win.sidebar.row_at_index(i as i32) {
        if win.sidebar.selected_row().as_ref() != Some(&row) {
            win.sidebar.select_row(Some(&row));
        }
    }
    win.split.set_show_content(true);
}

pub(super) fn build_window(app: &adw::Application, hub: &Rc<Hub>) -> Rc<Win> {
    add_host_icons();
    let menu = gio::Menu::new();
    menu.append(Some(t("Restart Tessera")), Some("app.restart"));
    menu.append(Some(t("About Tessera")), Some("app.about"));
    menu.append(Some(t("Quit")), Some("app.quit"));
    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(t("Refresh"))
        .build();

    // System: the service, what connects Tessera to the apps, and how it
    // starts.
    let service = adw::PreferencesGroup::builder().title(t("Service")).build();
    let service_row = plain_row(t("Token service"), "", "emblem-system-symbolic");
    let service_button = gtk::Button::builder()
        .label(t("Start"))
        .valign(gtk::Align::Center)
        .tooltip_text(t("Starts every driver again: the apps that were open lose the tokens until they are reopened"))
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
    service.add(&system_opensc_row(hub));
    let startup = adw::PreferencesGroup::builder().title(t("Startup")).build();
    for (title, action, icon) in [
        (
            t("Start automatically at login"),
            "app.autostart",
            "system-run-symbolic",
        ),
        (
            t("Show an icon in the tray"),
            "app.tray",
            "view-reveal-symbolic",
        ),
    ] {
        let row = plain_row(title, "", icon);
        let switch = gtk::Switch::builder()
            .valign(gtk::Align::Center)
            .action_name(action)
            .build();
        row.add_suffix(&switch);
        row.set_activatable_widget(Some(&switch));
        startup.add(&row);
    }

    // Home's first row.
    let summary_icon = gtk::Image::from_icon_name("object-select-symbolic");
    summary_icon.set_pixel_size(32);
    let summary_row = adw::ActionRow::builder()
        .use_markup(false)
        .title(t("Checking…"))
        .build();
    summary_row.add_prefix(&summary_icon);
    let summary_button = gtk::Button::builder()
        .label(t("See"))
        .valign(gtk::Align::Center)
        .tooltip_text(t("Open the System page"))
        .visible(false)
        .build();
    summary_row.add_suffix(&summary_button);

    // The sidebar.
    let token_badge = gtk::Label::builder()
        .css_classes(["dim-label", "numeric"])
        .visible(false)
        .build();
    let system_alert = gtk::Image::builder()
        .icon_name("dialog-warning-symbolic")
        .css_classes(["warning"])
        .visible(false)
        .build();
    let sidebar = gtk::ListBox::builder()
        .css_classes(["navigation-sidebar"])
        .build();
    for (name, title, icon) in PAGES {
        let mark: Option<gtk::Widget> = match name {
            "home" => Some(token_badge.clone().upcast()),
            "system" => Some(system_alert.clone().upcast()),
            _ => None,
        };
        sidebar.append(&sidebar_row(t(title), icon, mark.as_ref()));
    }
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .hhomogeneous(false)
        .vhomogeneous(false)
        .build();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(t("Tessera"))
        .default_width(980)
        .default_height(760)
        .width_request(360)
        .height_request(400)
        .build();
    let content_page = adw::NavigationPage::builder()
        .title(t("Home"))
        .tag("content")
        .build();
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(200.0)
        .max_sidebar_width(260.0)
        .build();

    let win = Rc::new(Win {
        window,
        toasts: adw::ToastOverlay::new(),
        service_row,
        service_button,
        host_row,
        pcsc_row,
        readers: Section::new(t("Readers"), None),
        tokens: Section::new(t("Tokens"), None),
        drivers: Section::new(t("Installed"), None),
        driver_rows: RefCell::new(vec![]),
        app_rows: RefCell::new(vec![]),
        native_rows: RefCell::new(vec![]),
        signer_rows: RefCell::new(vec![]),
        last: RefCell::new(String::new()),
        old_p11kit: old_p11kit_banner(),
        available: Section::new(
            t("Available to install"),
            Some(t(
                "Vendors' drivers, as add-ons of Tessera: install the one for your token.",
            )),
        ),
        summary_row,
        summary_icon,
        summary_button,
        token_badge,
        system_alert,
        split,
        content_page,
        sidebar,
        stack,
        service_bad: Cell::new(false),
        host_bad: Cell::new(false),
        pcsc_bad: Cell::new(false),
        tokens_found: Cell::new(0),
    });
    connect_old_p11kit_banner(&win);
    win.host_row
        .add_suffix(&copy_button(&win, apps::host_server_command().to_string()));

    // The pages.
    let (home, column) = page_column();
    let summary = adw::PreferencesGroup::new();
    summary.add(&win.summary_row);
    summary.add(&win.old_p11kit);
    column.append(&summary);
    column.append(&win.tokens.group);
    column.append(&win.readers.group);
    win.stack.add_named(&home, Some("home"));

    let (drivers_page, column) = page_column();
    column.append(&win.drivers.group);
    column.append(&win.available.group);
    win.stack.add_named(&drivers_page, Some("drivers"));

    let (apps_page, column) = page_column();
    let (apps, apps_stack, switcher) = apps_group(&win);
    column.append(&switcher);
    column.append(&apps);
    // The web signers run in Flatpak browsers only: shown with the Flatpak
    // tab, hidden with the native one (native browsers use the vendors'
    // own packages).
    let signers = signers_group(&win);
    apps_stack
        .bind_property("visible-child-name", &signers, "visible")
        .transform_to(|_, name: Option<String>| Some(name.as_deref() == Some("flatpak")))
        .sync_create()
        .build();
    column.append(&signers);
    win.stack.add_named(&apps_page, Some("apps"));

    let (system_page, column) = page_column();
    column.append(&service);
    column.append(&startup);
    win.stack.add_named(&system_page, Some("system"));

    // The sidebar's page, with the app's menu.
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .primary(true)
            .tooltip_text(t("Menu"))
            .build(),
    );
    let sidebar_view = adw::ToolbarView::new();
    sidebar_view.add_top_bar(&sidebar_header);
    sidebar_view.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&win.sidebar)
            .build(),
    ));
    win.split.set_sidebar(Some(
        &adw::NavigationPage::builder()
            .title(t("Tessera"))
            .child(&sidebar_view)
            .build(),
    ));
    // The content's page.
    let header = adw::HeaderBar::new();
    header.pack_start(&refresh_button);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&win.stack));
    win.content_page.set_child(Some(&view));
    win.split.set_content(Some(&win.content_page));
    win.toasts.set_child(Some(&win.split));
    win.window.set_content(Some(&win.toasts));
    // Narrow (a phone, a window half the screen): the sidebar is the first
    // screen, and each page opens over it.
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        620.0,
        adw::LengthUnit::Sp,
    ));
    narrow.add_setter(&win.split, "collapsed", Some(&true.to_value()));
    win.window.add_breakpoint(narrow);

    // A page opens when its row is chosen (selected, as the keyboard and
    // screen readers do) or clicked again (on a narrow window, to open the
    // page already chosen).
    {
        let w = Rc::downgrade(&win);
        win.sidebar.connect_row_activated(move |_, row| {
            if let Some(win) = w.upgrade() {
                if let Some((name, _, _)) = PAGES.get(row.index() as usize) {
                    show_page(&win, name);
                }
            }
        });
        let w = Rc::downgrade(&win);
        win.sidebar.connect_row_selected(move |_, row| {
            if let (Some(win), Some(row)) = (w.upgrade(), row) {
                if let Some((name, _, _)) = PAGES.get(row.index() as usize) {
                    if win.stack.visible_child_name().as_deref() != Some(name) {
                        show_page(&win, name);
                    }
                }
            }
        });
    }
    {
        let w = Rc::downgrade(&win);
        win.summary_button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                show_page(&win, "system");
            }
        });
    }
    show_page(&win, "home");
    win.split.set_show_content(false);

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
