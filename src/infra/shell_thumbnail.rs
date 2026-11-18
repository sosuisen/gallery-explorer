//! Thumbnails made by the Windows shell, as File Explorer shows them.
//!
//! Apps register thumbnail handlers for their own file types, such as Clip Studio Paint for
//! `.clip`. The shell calls the handler, so this works for any type that File Explorer shows.

use std::ffi::c_void;
use std::mem;
use std::path::Path;

use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, SIZE};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_THUMBNAILONLY,
};
use windows::core::HSTRING;

use super::bitmap::encode_bmp;

/// Returns the shell thumbnail of a file as a BMP file, at most `size` pixels on each side.
///
/// Returns `None` when the file type has no thumbnail handler. A generic icon is never
/// returned, so the caller can show its own icon instead.
pub fn shell_thumbnail(path: &Path, size: u32) -> Option<Vec<u8>> {
    // The thread pool may already have set up COM on this thread, in either mode.
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let result = thumbnail(path, size);
    if initialized.is_ok() {
        unsafe { CoUninitialize() };
    } else if initialized != RPC_E_CHANGED_MODE {
        return None;
    }
    result
}

fn thumbnail(path: &Path, size: u32) -> Option<Vec<u8>> {
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None) }.ok()?;
    let side = size as i32;
    let bitmap = unsafe {
        // Without SIIGBF_BIGGERSIZEOK, the shell scales a larger cached thumbnail down to `size`.
        factory.GetImage(SIZE { cx: side, cy: side }, SIIGBF_THUMBNAILONLY)
    }
    .ok()?;
    let pixels = read_pixels(bitmap);
    unsafe {
        _ = DeleteObject(bitmap.into());
    }
    let (width, height, bgra) = pixels?;
    Some(encode_bmp(width, height, &bgra))
}

/// Copies the pixels of a bitmap as 32-bit BGRA, from the top row down.
fn read_pixels(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            bitmap.into(),
            mem::size_of::<BITMAP>() as i32,
            Some(&mut info as *mut BITMAP as *mut c_void),
        )
    };
    if read == 0 || info.bmWidth <= 0 || info.bmHeight == 0 {
        return None;
    }
    let width = info.bmWidth;
    let height = info.bmHeight.abs();
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // A negative height asks for the rows from top to bottom.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    let dc = unsafe { CreateCompatibleDC(None) };
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr() as *mut c_void),
            &mut header,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        _ = DeleteDC(dc);
    }
    (lines == height).then_some((width as u32, height as u32, bgra))
}
