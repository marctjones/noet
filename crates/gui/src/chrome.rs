//! Windows 11 window chrome via DWM: a dark-mode titlebar that matches Noet's theme
//! (Win11 rounded corners come for free) and a Mica backdrop hint. True Mica
//! translucency can't show through Slint's opaque rendering surface, but the dark
//! titlebar is a real native touch. Applied to this process's visible top-level
//! window(s), found via `EnumWindows` so we don't depend on Slint's window-handle
//! API or the window title. No-op off Windows.

#[cfg(target_os = "windows")]
mod imp {
    use core::ffi::c_void;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    // windows-sys: BOOL = i32, and DwmSetWindowAttribute's attribute id is a u32.
    /// Apply Win11 chrome to our visible windows. The dark flag rides in `lparam`.
    pub fn apply(dark: bool) {
        unsafe {
            EnumWindows(Some(enum_cb), if dark { 1 } else { 0 });
        }
    }

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == GetCurrentProcessId() && IsWindowVisible(hwnd) != 0 {
            let dark: i32 = if lparam != 0 { 1 } else { 0 };
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &dark as *const i32 as *const c_void,
                core::mem::size_of::<i32>() as u32,
            );
            let backdrop: i32 = DWMSBT_MAINWINDOW;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                &backdrop as *const i32 as *const c_void,
                core::mem::size_of::<i32>() as u32,
            );
        }
        1 // TRUE → keep enumerating
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn apply(_dark: bool) {}
}

pub use imp::apply;
