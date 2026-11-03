use base64::{engine::general_purpose::STANDARD, Engine};
use windows_sys::Win32::{
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
};
/// Captures the primary display only after the local user enables sharing.
pub fn screen(width: u32) -> Result<String, String> {
    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        if sw <= 0 || sh <= 0 || sw > 16_384 || sh > 16_384 {
            return Err("Экран недоступен".into());
        }
        let longest = sw.max(sh);
        let scale = (width.clamp(1, 1280) as f64 / longest as f64).min(1.0);
        let w = (sw as f64 * scale).round().max(1.0) as i32;
        let h = (sh as f64 * scale).round().max(1.0) as i32;
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err("Экран недоступен".into());
        }
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        if dc.is_null() || bitmap.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap as _);
            }
            if !dc.is_null() {
                DeleteDC(dc);
            }
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("Недостаточно ресурсов для снимка".into());
        }
        let old = SelectObject(dc, bitmap as _);
        SetStretchBltMode(dc, HALFTONE as i32);
        let copied = StretchBlt(dc, 0, 0, w, h, screen, 0, 0, sw, sh, SRCCOPY);
        SelectObject(dc, old);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let rows = if copied != 0 {
            GetDIBits(
                dc,
                bitmap,
                0,
                h as u32,
                pixels.as_mut_ptr() as _,
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };
        DeleteObject(bitmap as _);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        if rows != h {
            return Err("Не удалось прочитать экран".into());
        }
        // Compact BGRA into RGB in place to avoid a second full-sized pixel buffer on x86.
        for i in 0..(w * h) as usize {
            let p = i * 4;
            let rgb = [pixels[p + 2], pixels[p + 1], pixels[p]];
            pixels[i * 3..i * 3 + 3].copy_from_slice(&rgb);
        }
        pixels.truncate((w * h * 3) as usize);
        for quality in [55, 35, 20, 10, 1] {
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
                .encode(&pixels, w as u32, h as u32, image::ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())?;
            if jpeg.len() <= 225_000 {
                return Ok(STANDARD.encode(jpeg));
            }
        }
        Err("Не удалось уменьшить снимок до допустимого размера".into())
    }
}
