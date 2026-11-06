//! Supervision, with or without the window: listing the tokens, publishing
//! the drivers that answer, checking what the apps see through the system's
//! p11-kit server, and keeping the tray and the background status in step.

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use super::app_rows::update_apps;
use super::window::{apply_snapshot, refresh, update_drivers, update_service};
use super::Hub;
use crate::status;
use crate::tray;

/// List the tokens of every running driver, then publish the drivers that
/// answered and restart (or, in the end, stop) those that did not. Runs with
/// or without a window: with the window closed, a token inserted later must
/// still reach the apps.
pub(super) fn supervise(hub: &Rc<Hub>) {
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
            if win.window.is_visible() {
                // A step done in a terminal shows without restarting.
                update_apps(&win);
            }
            apply_snapshot(&win, snap);
        }
    });
}

/// When the host was last checked, and for which (label, serial) tokens.
pub(super) type HostCheck = (std::time::Instant, Vec<(String, String)>);

/// Ask, through the system's p11-kit server, whether apps see Tessera's
/// tokens: when they change, and every 15 s with the window open (each
/// check loads the drivers again through that server).
pub(super) fn check_host(hub: &Rc<Hub>, ours: Vec<(String, String)>) {
    let visible = hub
        .win
        .borrow()
        .as_ref()
        .is_some_and(|w| w.window.is_visible());
    let every = if hub.host_misses.get() > 0 { 5 } else { 15 };
    let due = match &*hub.host_checked.borrow() {
        None => true,
        Some((when, last)) => {
            *last != ours
                || ((visible || hub.host_misses.get() > 0)
                    && when.elapsed() > Duration::from_secs(every))
        }
    };
    if !due || hub.host_busy.replace(true) {
        return;
    }
    *hub.host_checked.borrow_mut() = Some((std::time::Instant::now(), ours.clone()));
    let hub = hub.clone();
    let generation = hub.generation.get();
    glib::spawn_future_local(async move {
        let view = gio::spawn_blocking(move || {
            let view = status::host_view(&ours);
            // Only a server that serves Tessera's tokens is surely the
            // system's (and not Flatpak's own, which answers there when the
            // system's is off).
            let protocol = match view {
                status::HostView::Sees(..) => status::host_protocol(),
                _ => None,
            };
            (view, protocol)
        })
        .await;
        hub.host_busy.set(false);
        if generation != hub.generation.get() {
            // Started before a restart: the answer says nothing about now.
            return;
        }
        if let Ok((view, protocol)) = view {
            use status::HostView::{Off, Sees, Unknown, WithoutTessera};
            match view {
                Sees(..) if protocol.is_some() => hub.host_protocol.set(protocol),
                Off => hub.host_protocol.set(None),
                _ => {}
            }
            let miss = matches!(view, WithoutTessera | Unknown);
            hub.host_misses
                .set(if miss { hub.host_misses.get() + 1 } else { 0 });
            match view {
                Unknown => return,
                WithoutTessera if hub.host_misses.get() < 2 => return,
                _ => {}
            }
            *hub.host_view.borrow_mut() = Some(view);
            let win = hub.win.borrow().clone();
            if let Some(win) = win {
                update_service(&hub, &win);
            }
        }
    });
}

/// Keep the tray icon and the background-apps status in step.
pub(super) fn update_status(hub: &Rc<Hub>) {
    let text = hub.status_text();
    {
        let mut tray = hub.tray.borrow_mut();
        if hub.settings.borrow().hide_tray {
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

/// Checks run every few seconds with the window open, and less often
/// without it (still often enough to publish a token inserted meanwhile).
pub(super) fn start_supervision(hub: &Rc<Hub>) {
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
