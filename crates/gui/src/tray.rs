//! System-tray integration (Windows + macOS): a tray icon + small menu so Noet
//! stays one click away when a meeting starts, plus a Ctrl+Alt+N global hotkey.
//! Tray/menu/hotkey events are drained on the UI thread via a Slint `Timer` (no
//! event-loop ownership — the crates deliver events through global channels we
//! poll; both platforms run the required main-thread loop). No-op on Linux (see
//! the dependency note in Cargo.toml for why).

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod imp {
    use crate::AppWindow;
    use global_hotkey::{
        hotkey::{Code, HotKey, Modifiers},
        GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    };
    use slint::ComponentHandle;
    use std::time::Duration;
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem},
        TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    /// Keeps the tray icon, global-hotkey manager, and poll timer alive for the
    /// app's lifetime (drop = remove tray + unregister hotkeys).
    pub struct Tray {
        _tray: TrayIcon,
        _hotkeys: Option<GlobalHotKeyManager>,
        _timer: slint::Timer,
    }

    /// Build the tray icon + menu and start polling its events. Returns `None` if
    /// the tray can't be created (then Noet just runs without it).
    pub fn setup(ui: &AppWindow) -> Option<Tray> {
        if crate::runtime_flags::disable_tray() {
            return None;
        }

        let menu = Menu::new();
        let mi_meeting = MenuItem::new("New meeting note   Ctrl+Alt+N", true, None);
        let mi_capture = MenuItem::new("Quick capture   Ctrl+Alt+C", true, None);
        let mi_show = MenuItem::new("Show Noet", true, None);
        let mi_quit = MenuItem::new("Quit Noet", true, None);
        menu.append(&mi_meeting).ok()?;
        menu.append(&mi_capture).ok()?;
        menu.append(&mi_show).ok()?;
        menu.append(&mi_quit).ok()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Noet")
            .with_icon(brand_icon())
            .build()
            .ok()?;

        let (id_meeting, id_capture, id_show, id_quit) = (
            mi_meeting.id().clone(),
            mi_capture.id().clone(),
            mi_show.id().clone(),
            mi_quit.id().clone(),
        );

        // Global hotkeys: Ctrl+Alt+N → new meeting note, Ctrl+Alt+C → quick capture,
        // from anywhere. Best-effort — if a combo is taken, the tray still works.
        let hotkeys = GlobalHotKeyManager::new().ok();
        let (mut hk_meeting_id, mut hk_capture_id) = (0u32, 0u32);
        if let Some(mgr) = &hotkeys {
            let hk_n = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);
            if mgr.register(hk_n).is_ok() {
                hk_meeting_id = hk_n.id;
            }
            let hk_c = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);
            if mgr.register(hk_c).is_ok() {
                hk_capture_id = hk_c.id;
            }
        }

        let weak = ui.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(150),
            move || {
                // A click on the tray icon brings the window forward.
                while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                    let bring = matches!(
                        ev,
                        TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
                    );
                    if bring {
                        if let Some(ui) = weak.upgrade() {
                            let _ = ui.show();
                        }
                    }
                }
                // Menu selections.
                while let Ok(ev) = MenuEvent::receiver().try_recv() {
                    let Some(ui) = weak.upgrade() else { continue };
                    if ev.id == id_meeting {
                        crate::dispatch_cmd(&ui, "new-meeting");
                    } else if ev.id == id_capture {
                        crate::dispatch_cmd(&ui, "capture");
                    } else if ev.id == id_show {
                        crate::dispatch_cmd(&ui, "show");
                    } else if ev.id == id_quit {
                        let _ = slint::quit_event_loop();
                    }
                }
                // Global hotkeys (fire on press only).
                while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
                    if ev.state() != HotKeyState::Pressed {
                        continue;
                    }
                    let Some(ui) = weak.upgrade() else { continue };
                    if ev.id() == hk_meeting_id {
                        crate::dispatch_cmd(&ui, "new-meeting");
                    } else if ev.id() == hk_capture_id {
                        crate::dispatch_cmd(&ui, "capture");
                    }
                }
            },
        );

        Some(Tray {
            _tray: tray,
            _hotkeys: hotkeys,
            _timer: timer,
        })
    }

    /// A 32×32 brand-teal rounded-square icon, generated in code so we don't bundle
    /// or decode an image asset just for the tray.
    fn brand_icon() -> tray_icon::Icon {
        const N: i32 = 32;
        const INSET: i32 = 2;
        const R: f32 = 7.0;
        let (cr, cg, cb) = (0x2c_u8, 0x6e_u8, 0x68_u8); // Theme.accent
        let mut rgba = vec![0u8; (N * N * 4) as usize];
        let lo = INSET as f32 + R;
        let hi = (N - 1 - INSET) as f32 - R;
        for y in 0..N {
            for x in 0..N {
                let fx = x as f32;
                let fy = y as f32;
                // Nearest rounded-rect corner centre; distance test only in corners.
                let nx = fx.clamp(lo, hi);
                let ny = fy.clamp(lo, hi);
                let inside = fx >= INSET as f32
                    && fx <= (N - 1 - INSET) as f32
                    && fy >= INSET as f32
                    && fy <= (N - 1 - INSET) as f32
                    && ((fx - nx).powi(2) + (fy - ny).powi(2)).sqrt() <= R;
                if inside {
                    let i = ((y * N + x) * 4) as usize;
                    rgba[i] = cr;
                    rgba[i + 1] = cg;
                    rgba[i + 2] = cb;
                    rgba[i + 3] = 255;
                }
            }
        }
        tray_icon::Icon::from_rgba(rgba, N as u32, N as u32).expect("valid 32x32 icon")
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    use crate::AppWindow;
    /// Placeholder so callers don't need their own cfg.
    pub struct Tray;
    pub fn setup(_ui: &AppWindow) -> Option<Tray> {
        None
    }
}

pub use imp::{setup, Tray};
