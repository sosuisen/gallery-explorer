//! The app icon in the title bar and on the taskbar.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GWL_STYLE, GetWindowLongPtrW, ICON_BIG, ICON_SMALL, IMAGE_ICON,
    LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SYSTEM_METRICS_INDEX, SendMessageW,
    WM_SETICON, WS_CAPTION,
};
use windows::core::{BOOL, PCWSTR};

/// The resource ID of the icon that build.rs embeds in the exe.
const ICON_ID: usize = 1;

/// Shows the app icon in the title bar and on the taskbar of the windows of this thread.
///
/// windows-reactor 0.100.0 keeps `WindowVisuals::icon` but never uses it, and it does not give
/// out the window handle. So this finds the windows itself. It must run on the UI thread after
/// the window is created.
pub fn show_app_icon() {
    unsafe {
        // Only the windows with a title bar get the icon. WinUI may also own hidden windows.
        let _ = EnumThreadWindows(GetCurrentThreadId(), Some(set_icon), LPARAM(0));
    }
}

unsafe extern "system" fn set_icon(window: HWND, _: LPARAM) -> BOOL {
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) } as u32;
    if style & WS_CAPTION.0 == WS_CAPTION.0 {
        set_icon_of_size(window, ICON_BIG, SM_CXICON);
        set_icon_of_size(window, ICON_SMALL, SM_CXSMICON);
    }
    true.into()
}

/// Gives the window the icon of the size that Windows uses for `which` at the window's DPI.
fn set_icon_of_size(window: HWND, which: u32, metric: SYSTEM_METRICS_INDEX) {
    unsafe {
        let size = GetSystemMetricsForDpi(metric, GetDpiForWindow(window));
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        // LoadImageW picks the image of this size from the .ico. The icon stays loaded until
        // the app ends, because the window keeps using it.
        let Ok(icon) = LoadImageW(
            Some(module.into()),
            PCWSTR(ICON_ID as *const u16),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        ) else {
            return;
        };
        SendMessageW(
            window,
            WM_SETICON,
            Some(WPARAM(which as usize)),
            Some(LPARAM(icon.0 as isize)),
        );
    }
}
