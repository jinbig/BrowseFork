use super::wide;
use base64::Engine;
use windows_sys::Win32::{
    Graphics::Gdi::*,
    UI::{
        Shell::SHDefExtractIconW,
        WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
    },
};

const ICON_PIXELS: i32 = 128;
const CHANNELS: usize = 4;

struct Canvas {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
}
impl Canvas {
    fn new(fill: u8) -> Option<Self> {
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = ICON_PIXELS;
            info.bmiHeader.biHeight = -ICON_PIXELS; // PNG pixels are top-down.
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() {
                DeleteDC(dc);
                return None;
            }
            let old = SelectObject(dc, bitmap);
            if old.is_null() || old as isize == -1 {
                DeleteObject(bitmap);
                DeleteDC(dc);
                return None;
            }
            std::ptr::write_bytes(bits, fill, (ICON_PIXELS * ICON_PIXELS) as usize * CHANNELS);
            Some(Self {
                dc,
                bitmap,
                old,
                bits: bits.cast(),
            })
        }
    }
    fn pixels(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.bits, (ICON_PIXELS * ICON_PIXELS) as usize * CHANNELS)
        }
    }
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}
struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        unsafe {
            DestroyIcon(self.0);
        }
    }
}

pub(super) fn extract(path: &str, index: i32) -> Option<String> {
    let path = wide(path);
    let mut handle = std::ptr::null_mut();
    let status = unsafe {
        SHDefExtractIconW(
            path.as_ptr(),
            index,
            0,
            &mut handle,
            std::ptr::null_mut(),
            ICON_PIXELS as u32,
        )
    };
    if status < 0 || handle.is_null() {
        return None;
    }
    let icon = Icon(handle);
    let black = Canvas::new(0)?;
    let white = Canvas::new(255)?;
    for canvas in [&black, &white] {
        if unsafe {
            DrawIconEx(
                canvas.dc,
                0,
                0,
                icon.0,
                ICON_PIXELS,
                ICON_PIXELS,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            )
        } == 0
        {
            return None;
        }
    }
    unsafe {
        GdiFlush();
    }
    // Drawing against two backgrounds recovers both old mask-based and modern
    // alpha icons, without dark/white fringes around their antialiased edges.
    let mut rgba = Vec::with_capacity(black.pixels().len());
    for (black, white) in black
        .pixels()
        .chunks_exact(CHANNELS)
        .zip(white.pixels().chunks_exact(CHANNELS))
    {
        let alpha = 255
            - (0..3)
                .map(|channel| white[channel].saturating_sub(black[channel]))
                .max()
                .unwrap();
        for channel in [2, 1, 0] {
            rgba.push(if alpha == 0 {
                0
            } else {
                ((black[channel] as u32 * 255) / alpha as u32).min(255) as u8
            });
        }
        rgba.push(alpha);
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, ICON_PIXELS as u32, ICON_PIXELS as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
        writer.finish().ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_icon_extraction_produces_a_sharp_transparent_png() {
        let path =
            std::env::temp_dir().join(format!("browsefork-icon-test-{}.ico", std::process::id()));
        std::fs::write(&path, include_bytes!("../../../icons/icon.ico")).unwrap();
        let data = extract(&path.to_string_lossy(), 0);
        std::fs::remove_file(path).unwrap();
        let data = data.expect("Windows should extract the bundled icon at 128 pixels");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(
            (info.width, info.height),
            (ICON_PIXELS as u32, ICON_PIXELS as u32)
        );
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert!(
            pixels.chunks_exact(4).any(|pixel| pixel[3] == 0),
            "rounded corners must stay transparent"
        );
        assert!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel[3] == 255 && pixel[2] > pixel[0]),
            "icon must retain its blue pixels"
        );
    }
}
