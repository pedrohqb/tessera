//! The Apps section of the window: each client app and its one or two
//! steps (the p11-kit socket override and the NSS work of step 2).

use adw::prelude::*;
use gtk::{gio, glib};
use std::rc::Rc;

use super::window::{copy_button, plain_row, Win};
use crate::apps::{self, ClientApp, Setup};
use crate::i18n::t;
use crate::native;
use crate::nss;
use crate::paths;

/// The app's own icon (see themed_icon), else Tessera's copy of it, else a
/// generic one.
pub(super) fn app_icon(app: &ClientApp) -> gtk::Image {
    let names: Vec<&str> = std::iter::once(app.id).chain(app.bundled_icon).collect();
    themed_icon(&names, app.icon)
}

/// Is this app ready: (has the p11-kit socket, step 2 is done or not needed).
pub(super) fn app_state(app: &ClientApp) -> (bool, bool) {
    (apps::has_socket_override(app.id), !step2_needed(app))
}

/// Does step 2 still have something to do? The Chromium browsers need no
/// step 2 at all, unless an older Tessera left a module to remove.
pub(super) fn step2_needed(app: &ClientApp) -> bool {
    match app.setup {
        Setup::Poppler => !nss::poppler_db_ready(&paths::poppler_nss_dir(app.id)),
        Setup::Firefox => {
            let profiles = paths::firefox_profiles();
            profiles.is_empty() || !profiles.iter().all(|p| nss::has_module(p))
        }
        Setup::Chromium => apps::chromium_nssdb(app).is_some_and(|db| nss::has_module(&db)),
        Setup::None => false,
    }
}

pub(super) fn app_subtitle(app: &ClientApp) -> String {
    let state = match app_state(app) {
        (true, true) => t("Ready"),
        (false, true) => t("Step 1 missing"),
        (true, false) => t("Step 2 missing"),
        (false, false) => t("Steps 1 and 2 missing"),
    };
    format!("{} · {state}", app.id)
}

pub(super) fn step2_done(app: &ClientApp) -> String {
    format!("{}: {}", app.name, t("step 2 done; restart the app"))
}

/// The NSS work of step 2, in a child process. Returns a line for the user.
pub(super) fn set_up_nss(app: &'static ClientApp) -> Result<String, String> {
    match app.setup {
        Setup::Poppler => nss::set_up_poppler_db(&paths::poppler_nss_dir(app.id))
            .map(|_| step2_done(app))
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
            Ok(step2_done(app))
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
                Ok(step2_done(app))
            }
            _ => Ok(step2_done(app)),
        },
        Setup::None => Ok(String::new()),
    }
}

/// The rows of one app, refreshed by update_apps.
pub(super) struct AppRows {
    pub(super) app: &'static ClientApp,
    pub(super) row: adw::ExpanderRow,
    step1: adw::ActionRow,
    pub(super) copy: gtk::Button,
    step2: adw::ActionRow,
    pub(super) button: gtk::Button,
    pub(super) manual: Option<adw::ActionRow>,
}

/// Show each step as done or to do; a step with nothing to do has no
/// button, and the Chromium browsers show step 2 only to remove an old
/// module.
pub(super) fn update_app(r: &AppRows) {
    let app = r.app;
    let (s1, s2) = app_state(app);
    r.row.set_subtitle(&app_subtitle(app));
    let cmd = apps::override_command(app);
    if s1 {
        r.step1.set_title(&format!(
            "{} ✓",
            t("Step 1 · access to the system's p11-kit server")
        ));
        r.step1.set_subtitle(&format!(
            "{} ({cmd})",
            t("Done: the app can reach the system's p11-kit server")
        ));
    } else {
        r.step1
            .set_title(t("Step 1 · access to the system's p11-kit server"));
        r.step1.set_subtitle(&format!(
            "{} {cmd}",
            t("Run once in a terminal, then restart the app:")
        ));
    }
    r.copy.set_visible(!s1);

    let chromium = app.setup == Setup::Chromium;
    r.step2.set_visible(!chromium || !s2);
    let title = if chromium {
        t("Step 2 · remove the duplicate module")
    } else {
        t("Step 2 · certificate database")
    };
    if s2 {
        r.step2.set_title(&format!("{title} ✓"));
        r.step2.set_subtitle(match app.setup {
            Setup::Firefox => t("Done: Tessera's module is in every Firefox profile"),
            _ => t("Done: the app has its own certificate database"),
        });
    } else {
        r.step2.set_title(title);
        r.step2.set_subtitle(match app.setup {
            Setup::Firefox => t("Close Firefox first: adds the module to every Firefox profile"),
            Setup::Poppler => t("Makes the certificate database the app reads when signing"),
            _ => t("An older Tessera added a module that makes the browser ask for the PIN twice. Close the browser first"),
        });
    }
    r.button.set_visible(!s2);
    if let Some(m) = &r.manual {
        m.set_visible(!s2);
    }
}

pub(super) fn update_apps(win: &Win) {
    for r in win.app_rows.borrow().iter() {
        update_app(r);
    }
    for r in win.native_rows.borrow().iter() {
        update_native(r);
    }
}

pub(super) fn set_up_app(win: &Rc<Win>, app: &'static ClientApp) {
    // Registering the client module needs no running driver.
    let win = win.clone();
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(move || set_up_nss(app)).await;
        match res {
            Ok(Ok(done)) => win.toast(&done),
            Ok(Err(e)) => win.alert(t("Could not set up"), &e),
            Err(_) => win.toast(t("Could not set up")),
        }
        update_apps(&win);
    });
}

pub(super) fn client_row(win: &Rc<Win>, app: &'static ClientApp) -> AppRows {
    let row = adw::ExpanderRow::builder()
        .use_markup(false)
        .title(app.name)
        .build();
    row.add_prefix(&app_icon(app));

    let step1 = adw::ActionRow::builder()
        .use_markup(false)
        .subtitle_selectable(true)
        .build();
    let copy = copy_button(win, apps::override_command(app));
    step1.add_suffix(&copy);
    row.add_row(&step1);

    let step2 = adw::ActionRow::builder().use_markup(false).build();
    let button = gtk::Button::builder()
        .label(if app.setup == Setup::Chromium {
            t("Remove")
        } else {
            t("Set up")
        })
        .valign(gtk::Align::Center)
        .build();
    {
        let w = Rc::downgrade(win);
        button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                set_up_app(&win, app);
            }
        });
    }
    step2.add_suffix(&button);
    row.add_row(&step2);

    let manual = (app.setup == Setup::Firefox).then(|| {
        let manual = adw::ActionRow::builder()
            .use_markup(false)
            .subtitle(paths::CLIENT_MODULE)
            .subtitle_selectable(true)
            .build();
        // Set after use-markup is off: in the builder the title may come
        // first, and GTK then reads its "&" as markup.
        manual.set_title(t("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:"));
        row.add_row(&manual);
        manual
    });
    let rows = AppRows {
        app,
        row,
        step1,
        copy,
        step2,
        button,
        manual,
    };
    update_app(&rows);
    rows
}

/// The rows of one native app, refreshed by update_apps.
pub(super) struct NativeRows {
    app: &'static native::NativeApp,
    row: adw::ActionRow,
    button: gtk::Button,
}

/// An app's icon, at 32 px: for each of `names` in turn, the icon theme's
/// (the desktop's, an installed Flatpak's, or Tessera's own copy) or the
/// one in Flatpak's AppStream data (apps not installed); else `fallback`.
fn themed_icon(names: &[&str], fallback: &str) -> gtk::Image {
    let theme = gtk::gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d));
    let found = names.iter().find_map(|n| {
        if theme.as_ref().is_some_and(|t| t.has_icon(n)) {
            Some(gtk::Image::from_icon_name(n))
        } else {
            paths::appstream_icon(n).map(gtk::Image::from_file)
        }
    });
    let image = match found {
        Some(image) => {
            image.set_pixel_size(32);
            image
        }
        None => gtk::Image::from_icon_name(fallback),
    };
    // The same width either way, so the titles line up.
    image.set_size_request(32, 32);
    image
}

fn update_native(r: &NativeRows) {
    r.row.set_subtitle(&native::subtitle(r.app));
    r.button.set_visible(native::can_set_up(r.app));
}

fn set_up_native(win: &Rc<Win>, app: &'static native::NativeApp) {
    let win = win.clone();
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(move || native::set_up(app)).await;
        match res {
            Ok(Ok(done)) => win.toast(&done),
            Ok(Err(e)) => win.alert(t("Could not set up"), &e),
            Err(_) => win.toast(t("Could not set up")),
        }
        update_apps(&win);
    });
}

fn native_row(win: &Rc<Win>, app: &'static native::NativeApp) -> NativeRows {
    let row = adw::ActionRow::builder()
        .use_markup(false)
        .title(t(app.name))
        .build();
    row.add_prefix(&themed_icon(app.icons, app.fallback_icon));
    let button = gtk::Button::builder()
        .label(t("Set up"))
        .valign(gtk::Align::Center)
        .tooltip_text(if app.kind == native::Kind::PjeOffice {
            t("Points PJeOffice Pro's list of A3 libraries (list.a3) at p11-kit's library. Close PJeOffice Pro first")
        } else {
            t("Adds p11-kit-proxy.so to the app's certificate database. Close the app first")
        })
        .build();
    {
        let w = Rc::downgrade(win);
        button.connect_clicked(move |_| {
            if let Some(win) = w.upgrade() {
                set_up_native(&win, app);
            }
        });
    }
    row.add_suffix(&button);
    let rows = NativeRows { app, row, button };
    update_native(&rows);
    rows
}

/// A page of the Apps section: a line about it, then the rows.
fn apps_page(caption: &str) -> (gtk::Box, gtk::ListBox) {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let label = gtk::Label::builder()
        .label(caption)
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    page.append(&label);
    page.append(&list);
    (page, list)
}

/// The Apps section, with a tab for the Flatpak apps and one for the
/// native ones.
pub(super) fn apps_group(win: &Rc<Win>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t("Apps"))
        .description(t("Apps that use the tokens through Tessera."))
        .build();

    let (flatpak_page, flatpak_list) = apps_page(t(
        "Each needs one or two steps, once: access to the system connection, then (for some) a certificate database.",
    ));
    for app in apps::APPS {
        let rows = client_row(win, app);
        flatpak_list.append(&rows.row);
        win.app_rows.borrow_mut().push(rows);
    }
    // PJeOffice Pro (FlatPark) already has access to the host's p11-kit
    // server socket and loads p11-kit-client.so through its own bridge.
    let pje = plain_row(
        apps::PJEOFFICE.name,
        &format!(
            "{} · {}",
            apps::PJEOFFICE.id,
            t("No steps: the system connection is enough")
        ),
        "",
    );
    pje.add_prefix(&app_icon(&apps::PJEOFFICE));
    flatpak_list.append(&pje);

    let (native_page, native_list) = apps_page(t(
        "Apps installed from the distribution reach Tessera through p11-kit-proxy.so, without the system connection: each needs it in its certificate database, once.",
    ));
    for app in native::APPS {
        let rows = native_row(win, app);
        native_list.append(&rows.row);
        win.native_rows.borrow_mut().push(rows);
    }

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vhomogeneous(false)
        .build();
    stack.add_titled(&flatpak_page, Some("flatpak"), t("Flatpak"));
    stack.add_titled(&native_page, Some("native"), t("Native"));
    let switcher = gtk::StackSwitcher::builder()
        .stack(&stack)
        .valign(gtk::Align::Center)
        .build();
    group.set_header_suffix(Some(&switcher));
    group.add(&stack);
    group
}
