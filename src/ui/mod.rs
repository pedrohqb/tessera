//! The app: the state that outlives the window (Hub), the actions and the
//! GApplication. The window is in window.rs and app_rows.rs, the
//! supervision of the drivers, the tray and the background status in
//! supervise.rs.

mod app_rows;
mod restart;
mod signer_rows;
mod supervise;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::process::ExitCode;
use std::rc::Rc;

use crate::apps;
use crate::drivers::{self, Driver};
use crate::i18n::t;
use crate::paths;
use crate::server::Server;
use crate::settings::Settings;
use crate::status;
use crate::tray;
use supervise::{start_supervision, supervise, update_status, HostCheck};
use window::{build_window, Win};

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
    /// Checks in a row that did not find Tessera's tokens: one is not shown
    /// (it may fall in a restart, when nothing is published yet), and is
    /// checked again after 5 s.
    host_misses: Cell<u32>,
    /// The protocol version of the system's p11-kit server, read when it
    /// was last seen serving Tessera's tokens (native.rs, host_has_old_p11kit).
    pub(super) host_protocol: Cell<Option<u8>>,
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
        *self.host.borrow_mut() =
            apps::write_host_modules(&ds, &[], &[]).map_err(|e| format!("{e:#}"));
        // Before anything is served, so no app opens a token with the
        // system's OpenSC in the meantime.
        if let Err(e) = apps::set_host_opensc_off(apps::opensc_on(&ds, &disabled)) {
            eprintln!("tessera: the system's OpenSC: {e:#}");
        }
        *self.drivers.borrow_mut() = ds;
        // Check what the apps see again once the drivers are back.
        *self.host_checked.borrow_mut() = None;
        *self.host_view.borrow_mut() = None;
        self.host_misses.set(0);
    }

    /// Stop serving: servers down and nothing left published on the host
    /// (module files pointing at stopped servers would only make apps wait).
    fn shut_down(&self) {
        self.generation.set(self.generation.get() + 1);
        self.server.borrow_mut().stop();
        let _ = apps::write_host_modules(&self.drivers.borrow(), &[], &[]);
        let _ = apps::set_host_opensc_off(false);
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

/// Open the window, or bring it to the front.
fn present(app: &adw::Application, hub: &Rc<Hub>) {
    let existing = hub.win.borrow().clone();
    match existing {
        Some(win) => win.window.present(),
        None => build_window(app, hub).window.present(),
    }
}

async fn request_background(autostart: bool) -> Result<bool, String> {
    let response = ashpd::desktop::background::Background::request()
        .reason("Keep the token service running so other apps can use your tokens")
        .auto_start(autostart)
        .command(["tessera", "--background"])
        .dbus_activatable(false)
        .send()
        .await
        .and_then(|r| r.response())
        .map_err(|e| e.to_string())?;
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
                            win.alert(t("Could not turn on starting at login"), &e);
                        }
                    }
                }
            });
        });
    }
    app.add_action(&autostart);

    let show_tray = gio::SimpleAction::new_stateful(
        "tray",
        None,
        &(!hub.settings.borrow().hide_tray).to_variant(),
    );
    {
        let hub = hub.clone();
        show_tray.connect_activate(move |action, _| {
            let show = !action.state().and_then(|v| v.get::<bool>()).unwrap_or(true);
            action.set_state(&show.to_variant());
            hub.settings.borrow_mut().hide_tray = !show;
            hub.settings.borrow().save();
            update_status(&hub);
        });
    }
    app.add_action(&show_tray);

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

    // Start again, in a sandbox mounted afresh (see restart.rs).
    let restart_action = gio::SimpleAction::new("restart", None);
    {
        let (hub, app2) = (hub.clone(), app.clone());
        restart_action.connect_activate(move |_, _| {
            let hub = hub.clone();
            restart::restart(&app2, move |e| {
                let win = hub.win.borrow().clone();
                if let Some(win) = win {
                    win.alert(
                        t("Could not restart Tessera"),
                        &format!("{} ({e})", t("Quit it from the menu and open it again")),
                    );
                }
            });
        });
    }
    app.add_action(&restart_action);

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
                host_misses: Cell::new(0),
                host_protocol: Cell::new(None),
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
    // Started by restart.rs: let the instance that started it leave first.
    let args: Vec<String> = std::env::args().collect();
    let restarted = args.iter().any(|a| a == restart::RESTARTED);
    let args: Vec<String> = args
        .into_iter()
        .filter(|a| a != restart::RESTARTED)
        .collect();
    if restarted {
        restart::wait_for_previous();
    }
    let code = app.run_with_args(&args);
    ExitCode::from(code.get())
}
