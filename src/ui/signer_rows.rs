//! The Web signers section of the window: for each Flatpak browser, whether
//! each installed signer (Web PKI, WebSigner…) is set up in it, and the one
//! command that does it; then the signers not installed. See signers.rs.

use adw::prelude::*;
use std::rc::Rc;

use super::app_rows::app_icon;
use super::window::{copy_button_with, plain_row, store_button_for, Win};
use crate::apps;
use crate::catalog;
use crate::i18n::t;
use crate::signers::{self, Browser, Signer};

/// One signer's row in one browser's row.
struct SignerRow {
    signer: Signer,
    row: adw::ActionRow,
    copy: gtk::Button,
}

/// The rows of one browser, refreshed by update_signers.
pub(super) struct BrowserRows {
    browser: &'static Browser,
    row: adw::ExpanderRow,
    signers: Vec<SignerRow>,
}

fn update_browser(r: &BrowserRows) {
    let b = r.browser;
    let socket = apps::has_socket_override(b.app.id);
    let mut done = 0;
    for s in &r.signers {
        let ready = signers::is_set_up(&s.signer, b);
        done += usize::from(ready);
        if ready {
            s.row.set_title(&format!("{} ✓", s.signer.name));
            let mut text =
                t("Done: the browser can start it (restart the browser if it was open).")
                    .to_string();
            if let Some(o) = &s.signer.option {
                text = format!(
                    "{text} {} “{o}” {}",
                    t("In the plugin's settings, tick"),
                    t("once.")
                );
            }
            s.row.set_subtitle(&text);
        } else {
            s.row.set_title(&s.signer.name);
            s.row.set_subtitle(if socket {
                t("Run once in a terminal (copy button), then restart the browser.")
            } else {
                t("First do step 1 of this browser (Apps section). Then run once in a terminal (copy button) and restart the browser.")
            });
        }
        s.copy.set_visible(!ready);
    }
    let total = r.signers.len();
    let state = if !socket {
        t("Step 1 missing (Apps section)").to_string()
    } else if done == total {
        t("Ready").to_string()
    } else {
        format!("{done} {} {total} {}", t("of"), t("set up"))
    };
    r.row.set_subtitle(&format!("{} · {state}", b.app.id));
}

pub(super) fn update_signers(win: &Win) {
    for r in win.signer_rows.borrow().iter() {
        update_browser(r);
    }
}

fn browser_row(win: &Rc<Win>, b: &'static Browser, installed: &[Signer]) -> BrowserRows {
    let row = adw::ExpanderRow::builder()
        .use_markup(false)
        .title(b.app.name)
        .build();
    row.add_prefix(&app_icon(b.app));
    let mut rows = vec![];
    for s in installed {
        let r = adw::ActionRow::builder().use_markup(false).build();
        let cmd = signers::command(s, b);
        r.set_tooltip_text(Some(&cmd));
        let copy = copy_button_with(
            win,
            cmd,
            t("Copy the command that lets this browser start the signer"),
        );
        r.add_suffix(&copy);
        row.add_row(&r);
        rows.push(SignerRow {
            signer: s.clone(),
            row: r,
            copy,
        });
    }
    let rows = BrowserRows {
        browser: b,
        row,
        signers: rows,
    };
    update_browser(&rows);
    rows
}

pub(super) fn signers_group(win: &Rc<Win>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(t("Web signers"))
        .description(t("Browser plugins that sign on websites (Web PKI, WebSigner), installed as add-ons of Tessera. The Flatpak browsers run them: each needs one command, once per plugin."))
        .build();
    let installed = signers::installed();
    if !installed.is_empty() {
        let browsers = signers::installed_browsers();
        if browsers.is_empty() {
            let row = plain_row(
                t("No Flatpak browser installed"),
                t("Firefox, Chromium, Google Chrome, Microsoft Edge or Brave, from Flathub"),
                "web-browser-symbolic",
            );
            row.add_css_class("dim-label");
            group.add(&row);
        }
        for b in browsers {
            let rows = browser_row(win, b, &installed);
            group.add(&rows.row);
            win.signer_rows.borrow_mut().push(rows);
        }
    }
    for k in signers::missing(&installed) {
        let ext = k.extension();
        let install = catalog::install_command_of(&ext);
        let row = plain_row(
            k.name,
            &format!(
                "{} · {} · {}\n{}",
                t("Not installed"),
                k.vendor,
                t(k.used_by),
                t("Install it from the app store (an add-on on Tessera's page) or FlatPark's page (button), or with the command (copy button); then restart Tessera (menu)")
            ),
            "application-x-addon-symbolic",
        );
        row.add_css_class("dim-label");
        row.set_tooltip_text(Some(&install));
        row.add_suffix(&store_button_for(
            win,
            catalog::store_page_of(&ext),
            t("Open this signer's page on FlatPark, to install it from the app store"),
        ));
        row.add_suffix(&copy_button_with(
            win,
            install,
            t("Copy the command that installs this signer"),
        ));
        group.add(&row);
    }
    group
}
