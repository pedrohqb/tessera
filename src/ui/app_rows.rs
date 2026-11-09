//! The Apps section of the window: each client app and its one or two
//! steps (the p11-kit socket override and the NSS work of step 2).

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

use super::window::{copy_button, plain_row, Win};
use crate::apps::{self, ClientApp, Setup};
use crate::i18n::t;
use crate::nss;
use crate::paths;
use crate::{jsignpdf, libreoffice, masterpdf, native, status};

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
        Setup::Profiles(_) => {
            let profiles = apps::app_profiles(app);
            profiles.is_empty() || !profiles.iter().all(|p| nss::has_module(p))
        }
        Setup::Chromium => apps::chromium_nssdb(app).is_some_and(|db| nss::has_module(&db)),
        Setup::LibreOffice => {
            let db = libreoffice::flatpak_db();
            libreoffice::cert_dir(&libreoffice::flatpak_profile()).flatten()
                != Some(db.to_string_lossy().into_owned())
                || !nss::poppler_db_ready(&db)
        }
        // Whether the slot is still the right one takes a look at the
        // system's server, too slow for here: Set up stays available.
        Setup::JSignPdf => !jsignpdf::ready(None),
        Setup::MasterPdf => !std::fs::read_to_string(masterpdf::config_file())
            .is_ok_and(|t| masterpdf::is_set_up(&t)),
        Setup::ByHand | Setup::None => false,
    }
}

/// Is step 2 waiting for a folder of the app that Tessera does not see:
/// the app is installed, but its profile or settings are not there? Either
/// it was never opened, or it was first opened after Tessera started (the
/// sandbox mounts only what existed then): open it once, then restart
/// Tessera.
pub(super) fn needs_restart(app: &ClientApp) -> bool {
    let missing = match app.setup {
        Setup::Profiles(_) => apps::app_profiles(app).is_empty(),
        Setup::LibreOffice => {
            libreoffice::cert_dir(&libreoffice::flatpak_profile()).is_none()
                || libreoffice::flatpak_db().parent().map(|p| p.is_dir()) != Some(true)
        }
        Setup::JSignPdf => !jsignpdf::config_dir()
            .parent()
            .is_some_and(std::path::Path::is_dir),
        Setup::MasterPdf => !masterpdf::config_file().is_file(),
        Setup::Poppler => !paths::poppler_config_dir(app.id).is_dir(),
        Setup::Chromium | Setup::ByHand | Setup::None => false,
    };
    missing && paths::flatpak_installed(app.id)
}

pub(super) fn app_subtitle(app: &ClientApp) -> String {
    let state = match app_state(app) {
        (true, true) if app.setup == Setup::ByHand => t("Step 2 by hand"),
        (false, true) if app.setup == Setup::ByHand => t("Step 1 missing; step 2 by hand"),
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
pub(super) fn set_up_nss(
    app: &'static ClientApp,
    token: Option<jsignpdf::TokenId>,
) -> Result<String, String> {
    match app.setup {
        Setup::Poppler => {
            if !paths::poppler_config_dir(app.id).is_dir() {
                return Err(format!(
                    "{}: {}",
                    app.name,
                    t("open the app once, then restart Tessera and try again")
                ));
            }
            nss::set_up_poppler_db(&paths::poppler_nss_dir(app.id))
                .map(|_| step2_done(app))
                .map_err(|e| format!("{e:#}"))
        }
        Setup::Profiles(_) => {
            let profiles = apps::app_profiles(app);
            if profiles.is_empty() {
                return Err(format!(
                    "{}: {}",
                    app.name,
                    t("no profile found: open the app once, then try again")
                ));
            }
            if profiles.iter().any(|p| nss::profile_in_use(p)) {
                return Err(format!(
                    "{}: {}",
                    app.name,
                    t("close the app first, then try again")
                ));
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
        Setup::LibreOffice => {
            let profile = libreoffice::flatpak_profile();
            // Tessera sees the app's folders only if they were there when it
            // started (its sandbox mounts what exists).
            let db_parent = libreoffice::flatpak_db().parent().map(|p| p.is_dir());
            if libreoffice::cert_dir(&profile).is_none() || db_parent != Some(true) {
                return Err(format!(
                    "{}: {}",
                    app.name,
                    t("open the app once, then restart Tessera and try again")
                ));
            }
            if libreoffice::running(&profile) {
                return Err(format!(
                    "{}: {} ({})",
                    app.name,
                    t("close the app first, then try again"),
                    t("if it is closed, a crash may have left the file .lock in its settings folder")
                ));
            }
            let db = libreoffice::flatpak_db();
            if let Some(Some(dir)) = libreoffice::cert_dir(&profile) {
                // A certificate path the user chose is kept.
                if std::path::Path::new(&dir) != db && std::path::Path::new(&dir).is_dir() {
                    return Err(format!(
                        "{}: {} ({dir})",
                        app.name,
                        t("it already has a certificate path of its own, which Tessera keeps. To use Tessera's, clear it in Tools → Options → Security → Certificate Path, then try again")
                    ));
                }
            }
            nss::set_up_poppler_db(&db).map_err(|e| format!("{}: {e:#}", db.display()))?;
            libreoffice::set_cert_dir(&profile, &db)?;
            Ok(step2_done(app))
        }
        Setup::JSignPdf => {
            let ours = status::tessera_tokens();
            let index = status::host_slot_of(&ours, token.as_ref()).ok_or_else(|| {
                format!(
                    "{}: {}",
                    app.name,
                    t("the system's p11-kit server does not list Tessera's tokens: plug in the token, check the system connection, then try again")
                )
            })?;
            // The choice counts only if the system's server lists it
            // (host_slot_of fell back to the first token otherwise).
            let listed = token.filter(|c| ours.contains(c));
            jsignpdf::set_up(index, listed.as_ref())?;
            Ok(step2_done(app))
        }
        Setup::MasterPdf => {
            masterpdf::set_up()?;
            Ok(step2_done(app))
        }
        Setup::ByHand | Setup::None => Ok(String::new()),
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
    restart: gtk::Button,
    pub(super) manual: Option<adw::ActionRow>,
    /// JSignPdf: which of Tessera's tokens it opens (one slot, one token).
    choice: Option<TokenChoice>,
}

/// A list of Tessera's tokens, shown when there is more than one.
pub(super) struct TokenChoice {
    drop: gtk::DropDown,
    list: gtk::StringList,
    items: RefCell<Vec<jsignpdf::TokenId>>,
}

impl TokenChoice {
    fn selected(&self) -> Option<jsignpdf::TokenId> {
        self.items
            .borrow()
            .get(self.drop.selected() as usize)
            .cloned()
    }
}

/// Offer the tokens Tessera serves now in JSignPdf's list, keeping the one
/// picked if it is still there, else the one its file names, else the
/// first.
pub(super) fn set_token_choices(win: &Win, tokens: &[status::Token]) {
    let mut items: Vec<jsignpdf::TokenId> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for tk in tokens {
        let id = (tk.label.clone(), tk.serial.clone());
        if !items.contains(&id) {
            names.push(format!("{} · {}", tk.label, tk.driver));
            items.push(id);
        }
    }
    for r in win.app_rows.borrow().iter() {
        let Some(c) = &r.choice else { continue };
        if *c.items.borrow() == items {
            continue;
        }
        let keep = c.selected().or_else(jsignpdf::configured_token);
        let at = keep
            .and_then(|k| items.iter().position(|i| *i == k))
            .unwrap_or(0);
        *c.items.borrow_mut() = items.clone();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        c.list.splice(0, c.list.n_items(), &refs);
        c.drop.set_selected(at as u32);
        c.drop.set_visible(items.len() > 1);
        update_app(r);
    }
}

/// Show each step as done or to do; a step with nothing to do has no
/// button, and the Chromium browsers show step 2 only to remove an old
/// module.
pub(super) fn update_app(r: &AppRows) {
    let app = r.app;
    let (s1, mut s2) = app_state(app);
    // JSignPdf set up for another token than the one picked.
    let picked = r.choice.as_ref().and_then(TokenChoice::selected);
    let configured = jsignpdf::configured_token();
    let switch = s2 && picked.is_some() && configured.is_some() && picked != configured;
    s2 &= !switch;
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
    r.step2
        .set_visible(app.setup != Setup::ByHand && (!chromium || !s2));
    let title = if chromium {
        t("Step 2 · remove the duplicate module")
    } else {
        t("Step 2 · certificate database")
    };
    if switch {
        r.step2.set_title(title);
        r.step2.set_subtitle(t(
            "Close JSignPdf, then press Set up: it was set up for another token",
        ));
    } else if s2 {
        r.step2.set_title(&format!("{title} ✓"));
        r.step2.set_subtitle(match app.setup {
            Setup::Profiles(_) => t("Done: Tessera's module is in every profile"),
            Setup::JSignPdf => t("Done: JSignPdf opens Tessera's tokens (keystore type PKCS11). To sign, type the token's PIN in “Keystore password” and press “Load keys”. Set up again if the token moves to another reader"),
            Setup::LibreOffice => t("Done: LibreOffice uses a certificate database of its own (Tools → Options → Security → Certificate Path)"),
            Setup::MasterPdf => t("Done: Master PDF Editor's PKCS#11 provider is p11-kit (Tools → Settings → Certificates)"),
            _ => t("Done: the app has its own certificate database"),
        });
        if let Some((label, _)) =
            jsignpdf::configured_token().filter(|_| app.setup == Setup::JSignPdf)
        {
            r.step2.set_subtitle(&format!(
                "{}: {label}. {}",
                t("Token JSignPdf opens"),
                r.step2.subtitle().unwrap_or_default()
            ));
        }
    } else if needs_restart(app) {
        r.step2.set_title(title);
        r.step2.set_subtitle(t("Open the app once, so it makes its settings, then restart Tessera: it sees an app's folders only if they were there when it started"));
    } else {
        r.step2.set_title(title);
        r.step2.set_subtitle(match app.setup {
            Setup::Profiles(_) => t("Close the app first: adds the module to every profile"),
            Setup::Poppler => t("Makes the certificate database the app reads when signing"),
            Setup::LibreOffice => t("Close LibreOffice first: makes a certificate database of its own and sets it as its certificate path"),
            Setup::MasterPdf => t("Close Master PDF Editor first: sets p11-kit as its PKCS#11 provider (Tools → Settings → Certificates)"),
            Setup::JSignPdf => t("Close JSignPdf first: writes its pkcs11.cfg (p11-kit and the slot of Tessera's tokens) and chooses the keystore type PKCS11 in its settings (needs the token plugged in and the system connection)"),
            _ => t("An older Tessera added a module that makes the app ask for the PIN twice. Close the app first"),
        });
    }
    // JSignPdf's slot may change: Set up stays available.
    let restart = !s2 && needs_restart(app);
    r.button
        .set_visible((!s2 || app.setup == Setup::JSignPdf) && !restart);
    r.restart.set_visible(restart);
    if let Some(m) = &r.manual {
        m.set_visible(!s2 || app.setup == Setup::ByHand);
    }
}

pub(super) fn update_apps(win: &Win) {
    for r in win.app_rows.borrow().iter() {
        update_app(r);
    }
    for r in win.native_rows.borrow().iter() {
        update_native(r);
    }
    super::signer_rows::update_signers(win);
}

pub(super) fn set_up_app(win: &Rc<Win>, app: &'static ClientApp) {
    // Registering the client module needs no running driver.
    let token = win
        .app_rows
        .borrow()
        .iter()
        .find(|r| r.app.id == app.id)
        .and_then(|r| r.choice.as_ref())
        .and_then(TokenChoice::selected);
    let win = win.clone();
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(move || set_up_nss(app, token)).await;
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
    let choice = (app.setup == Setup::JSignPdf).then(|| {
        let list = gtk::StringList::new(&[]);
        let drop = gtk::DropDown::builder()
            .model(&list)
            .valign(gtk::Align::Center)
            .tooltip_text(t("Token JSignPdf opens"))
            .visible(false)
            .build();
        let w = Rc::downgrade(win);
        drop.connect_selected_notify(move |_| {
            if let Some(win) = w.upgrade() {
                update_apps(&win);
            }
        });
        step2.add_suffix(&drop);
        TokenChoice {
            drop,
            list,
            items: RefCell::new(Vec::new()),
        }
    });
    step2.add_suffix(&button);
    let restart = gtk::Button::builder()
        .label(t("Restart Tessera"))
        .action_name("app.restart")
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    step2.add_suffix(&restart);
    row.add_row(&step2);

    let manual = app.by_hand.map(|how| {
        let manual = adw::ActionRow::builder()
            .use_markup(false)
            .subtitle(paths::CLIENT_MODULE)
            .subtitle_selectable(true)
            .build();
        // Set after use-markup is off: in the builder the title may come
        // first, and GTK then reads its "&" as markup.
        manual.set_title(t(how));
        if matches!(app.setup, Setup::ByHand | Setup::MasterPdf) {
            manual.add_suffix(&copy_button(win, paths::CLIENT_MODULE.to_string()));
        }
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
        restart,
        manual,
        choice,
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

/// Does the theme have this icon, in a file Tessera can read? An icon the
/// host exports may be a link into a folder the sandbox cannot see.
fn readable_icon(theme: &gtk::IconTheme, name: &str) -> bool {
    if !theme.has_icon(name) {
        return false;
    }
    let icon = theme.lookup_icon(
        name,
        &[],
        32,
        1,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    );
    match icon.file().and_then(|f| f.path()) {
        Some(path) => usable_icon_file(&path) && std::fs::File::open(path).is_ok(),
        // In a resource, not a file.
        None => true,
    }
}

/// Is this file the app's full-colour icon? A theme may hold only a
/// monochrome (`-symbolic`) one, which GTK hands out for the plain name too,
/// or only the small sizes of a panel theme (16 to 24 px): either looks
/// broken at 32 px next to the others, and the icon the Flatpak app exports
/// (or Flatpak's AppStream copy of it) is better.
fn usable_icon_file(path: &std::path::Path) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.ends_with("-symbolic") {
        return false;
    }
    let small = ["16", "22", "24", "16x16", "22x22", "24x24", "symbolic"];
    !path
        .ancestors()
        .skip(1)
        .take(3)
        .filter_map(|d| d.file_name().and_then(|n| n.to_str()))
        .any(|n| small.contains(&n) || small.iter().any(|s| n.starts_with(&format!("{s}@"))))
}

/// An app's icon, at 32 px: for each of `names` in turn, the icon theme's
/// (the desktop's or Tessera's own copy) or the one in Flatpak's AppStream
/// data (every app of each remote, installed or not); else `fallback`.
pub(super) fn themed_icon(names: &[&str], fallback: &str) -> gtk::Image {
    let theme = gtk::gdk::Display::default().map(|d| gtk::IconTheme::for_display(&d));
    let found = names.iter().find_map(|n| {
        if theme.as_ref().is_some_and(|t| readable_icon(t, n)) {
            Some(gtk::Image::from_icon_name(n))
        } else {
            paths::flatpak_export_icon(n)
                .or_else(|| paths::appstream_icon(n))
                .map(gtk::Image::from_file)
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
    r.button.set_label(native::button_label(r.app));
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
    for app in apps::APPS.into_iter().filter(|a| apps::shown(a)) {
        let rows = client_row(win, app);
        flatpak_list.append(&rows.row);
        win.app_rows.borrow_mut().push(rows);
    }
    // PJeOffice Pro and Assinador SERPRO (FlatPark) already have access to
    // the host's p11-kit server socket and load p11-kit-client.so through
    // their own bridges.
    for app in apps::NO_STEPS {
        let row = plain_row(
            app.name,
            &format!(
                "{} · {}",
                app.id,
                t("No steps: the system connection is enough")
            ),
            "",
        );
        row.add_prefix(&app_icon(app));
        flatpak_list.append(&row);
    }

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

#[cfg(test)]
mod tests {
    use super::usable_icon_file;
    use std::path::Path;

    #[test]
    fn small_and_symbolic_icons_are_passed_over() {
        let ok = |p: &str| usable_icon_file(Path::new(p));
        assert!(ok(
            "/run/host/share/icons/hicolor/scalable/apps/org.gnome.Evolution.svg"
        ));
        assert!(ok("/usr/share/icons/hicolor/128x128/apps/firefox.png"));
        assert!(ok("/usr/share/icons/breeze/apps/48/okular.svg"));
        assert!(!ok(
            "/run/host/share/icons/hicolor/symbolic/apps/org.gnome.Evolution-symbolic.svg"
        ));
        assert!(!ok("/usr/share/icons/hicolor/16x16/apps/evolution.png"));
        assert!(!ok("/usr/share/icons/breeze-dark/apps/22/evolution.svg"));
        assert!(!ok("/usr/share/icons/Papirus/24x24@2x/apps/x.svg"));
    }
}
