//! Launch-on-startup, per-user and without admin rights, on all platforms:
//! `HKCU\…\Run` on Windows, a Launch Agent on macOS, an XDG autostart `.desktop`
//! on Linux (so it works on GNOME/Wayland without a tray or any system libs).

mod imp {
    #[cfg(target_os = "macos")]
    use auto_launch::MacOSLaunchMode;
    use auto_launch::{AutoLaunch, AutoLaunchBuilder};

    fn auto_launch() -> Option<AutoLaunch> {
        let exe = std::env::current_exe().ok()?;
        let mut b = AutoLaunchBuilder::new();
        b.set_app_name("Noet").set_app_path(&exe.to_string_lossy());
        // macOS: a Launch Agent (plist in ~/Library/LaunchAgents) rather than an
        // AppleScript login item — cleaner and survives without Automation perms.
        #[cfg(target_os = "macos")]
        b.set_macos_launch_mode(MacOSLaunchMode::LaunchAgent);
        b.build().ok()
    }

    /// Whether Noet is registered to launch at login.
    pub fn is_enabled() -> bool {
        auto_launch()
            .and_then(|a| a.is_enabled().ok())
            .unwrap_or(false)
    }

    /// Enable/disable launch-at-login. Returns the resulting state (so the caller
    /// can reflect what actually happened, not just what was requested).
    pub fn set_enabled(on: bool) -> bool {
        let Some(a) = auto_launch() else { return false };
        let _ = if on { a.enable() } else { a.disable() };
        is_enabled()
    }
}

/// Launch-on-startup is wired on every desktop platform now.
pub const SUPPORTED: bool = true;

pub use imp::{is_enabled, set_enabled};
