//! Launch-on-startup, per-user and without admin rights (important on a locked-down
//! corporate Windows 11 machine). Windows uses `auto-launch` (writes
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`); macOS uses a per-user
//! Launch Agent. Linux is a no-op for now.

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod imp {
    use auto_launch::{AutoLaunch, AutoLaunchBuilder};

    fn auto_launch() -> Option<AutoLaunch> {
        let exe = std::env::current_exe().ok()?;
        let mut b = AutoLaunchBuilder::new();
        b.set_app_name("Noet").set_app_path(&exe.to_string_lossy());
        // macOS: a Launch Agent (plist in ~/Library/LaunchAgents) rather than an
        // AppleScript login item — cleaner and survives without Automation perms.
        #[cfg(target_os = "macos")]
        b.set_use_launch_agent(true);
        b.build().ok()
    }

    /// Whether Noet is registered to launch at login.
    pub fn is_enabled() -> bool {
        auto_launch().and_then(|a| a.is_enabled().ok()).unwrap_or(false)
    }

    /// Enable/disable launch-at-login. Returns the resulting state (so the caller
    /// can reflect what actually happened, not just what was requested).
    pub fn set_enabled(on: bool) -> bool {
        let Some(a) = auto_launch() else { return false };
        let _ = if on { a.enable() } else { a.disable() };
        is_enabled()
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn set_enabled(_on: bool) -> bool {
        false
    }
}

/// True only on platforms where launch-on-startup is wired (so the UI can hide the
/// toggle elsewhere).
pub const SUPPORTED: bool = cfg!(any(target_os = "windows", target_os = "macos"));

pub use imp::{is_enabled, set_enabled};
