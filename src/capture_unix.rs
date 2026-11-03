use base64::{engine::general_purpose::STANDARD, Engine};
pub fn screen(width: u32) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland")
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return Err(
            "Для показа экрана в этой сборке выбери сеанс X11. Wayland пока не поддерживается"
                .into(),
        );
    }
    let screens = screenshots::Screen::all().map_err(|e| format!("Экран недоступен: {e}"))?;
    let screen = screens
        .iter()
        .find(|s| s.display_info.is_primary)
        .or(screens.first())
        .ok_or("Мониторы не найдены")?;
    let capture = screen
        .capture()
        .map_err(|e| format!("Разреши запись экрана в системных настройках: {e}"))?;
    let (w, h) = (capture.width(), capture.height());
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err("Неподдерживаемый размер экрана".into());
    }
    let image = image::RgbaImage::from_raw(w, h, capture.into_raw())
        .ok_or("Повреждено изображение экрана")?;
    let image = image::DynamicImage::ImageRgba8(image)
        .thumbnail(width.clamp(1, 1280), width.clamp(1, 1280))
        .to_rgb8();
    for quality in [55, 35, 20, 10, 1] {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality)
            .encode(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| e.to_string())?;
        if bytes.len() <= 225000 {
            return Ok(STANDARD.encode(bytes));
        }
    }
    Err("Не удалось сжать снимок экрана".into())
}
