//! The tray icon (StatusNotifierItem, shown by KDE and by GNOME with the
//! AppIndicator extension, which Bluefin, Aurora and Ubuntu enable): while
//! no window is open it reminds that Tessera keeps serving the tokens, and
//! brings the window back.

use ksni::blocking::TrayMethods;

use crate::i18n::t;
use crate::paths;

pub enum Event {
    Open,
    Quit,
}

struct TesseraTray {
    status: String,
    icon: Vec<ksni::Icon>,
    events: async_channel::Sender<Event>,
}

impl ksni::Tray for TesseraTray {
    fn id(&self) -> String {
        paths::APP_ID.into()
    }
    fn title(&self) -> String {
        "Tessera".into()
    }
    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }
    fn icon_name(&self) -> String {
        format!("{}-symbolic", paths::APP_ID)
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon.clone()
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Tessera".into(),
            description: self.status.clone(),
            ..Default::default()
        }
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.events.send_blocking(Event::Open);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: self.status.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: t("Open Tessera").into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.events.send_blocking(Event::Open);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: t("Quit (stop serving the tokens)").into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.events.send_blocking(Event::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct Tray {
    handle: ksni::blocking::Handle<TesseraTray>,
}

impl Tray {
    /// Show the tray icon. `icon` is the app icon as ARGB pixmaps, for hosts
    /// that cannot find it by name.
    pub fn show(
        status: String,
        icon: Vec<ksni::Icon>,
        events: async_channel::Sender<Event>,
    ) -> Option<Self> {
        let tray = TesseraTray {
            status,
            icon,
            events,
        };
        // A sandbox may not own the well-known name the spec suggests.
        match tray.disable_dbus_name(true).spawn() {
            Ok(handle) => Some(Self { handle }),
            Err(e) => {
                // Tried again at every check (the host may get a tray
                // later); say so only once.
                static SAID: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    eprintln!("tessera: no tray: {e}");
                }
                None
            }
        }
    }

    pub fn set_status(&self, status: &str) {
        let status = status.to_string();
        self.handle.update(move |t| {
            if t.status != status {
                t.status = status;
            }
        });
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let _ = self.handle.shutdown();
    }
}

/// The app icon as tray pixmaps (ARGB32, network byte order).
pub fn icon_pixmaps() -> Vec<ksni::Icon> {
    use gtk::gdk_pixbuf::Pixbuf;
    let path = format!(
        "/app/share/icons/hicolor/scalable/apps/{}.svg",
        paths::APP_ID
    );
    [22, 32, 48]
        .iter()
        .filter_map(|&size| {
            let pb = Pixbuf::from_file_at_size(&path, size, size).ok()?;
            let pb = if pb.has_alpha() {
                pb
            } else {
                pb.add_alpha(false, 0, 0, 0).ok()?
            };
            let (w, h, stride) = (pb.width(), pb.height(), pb.rowstride() as usize);
            let bytes = pb.read_pixel_bytes();
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let i = y * stride + x * 4;
                    let (r, g, b, a) = (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]);
                    data.extend_from_slice(&[a, r, g, b]);
                }
            }
            Some(ksni::Icon {
                width: w,
                height: h,
                data,
            })
        })
        .collect()
}
