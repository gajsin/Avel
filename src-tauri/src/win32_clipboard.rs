use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::ptr;
use std::thread::sleep;
use std::time::Duration;

use base64::Engine;
use image::{imageops::FilterType, DynamicImage, ImageFormat, Rgba, RgbaImage};
use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_BITFIELDS, BI_RGB};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

pub const CF_DIB: u32 = 8;
pub const CF_UNICODETEXT: u32 = 13;
pub const CF_HDROP: u32 = 15;
pub const CF_DIBV5: u32 = 17;

#[repr(C)]
#[allow(non_snake_case)]
pub struct DROPFILES {
    pub pFiles: u32,
    pub pt: windows_sys::Win32::Foundation::POINT,
    pub fNC: windows_sys::Win32::Foundation::BOOL,
    pub fWide: windows_sys::Win32::Foundation::BOOL,
}

pub struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

fn open_clipboard_with_retry(owner: HWND) -> Option<ClipboardGuard> {
    for _ in 0..6 {
        unsafe {
            if OpenClipboard(owner) != 0 {
                return Some(ClipboardGuard);
            }
        }
        sleep(Duration::from_millis(15));
    }
    None
}

pub fn get_current_sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

pub struct ExtractedClipboard {
    pub content_type: String, // "text", "link", "code", "image"
    pub text_content: Option<String>,
    pub char_count: Option<i64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub thumbnail_b64: Option<String>,
    pub hash: String, // BLAKE3 hex
}

enum RawClipboardPayload {
    Image(Vec<u8>),
    Text(String),
}

pub fn read_clipboard_content(images_dir: &Path) -> Option<ExtractedClipboard> {
    // 1. Open clipboard and copy out raw data immediately to minimize lock hold time
    let payload = {
        let _guard = open_clipboard_with_retry(ptr::null_mut())?;
        unsafe {
            let has_dibv5 = IsClipboardFormatAvailable(CF_DIBV5) != 0;
            let has_dib = IsClipboardFormatAvailable(CF_DIB) != 0;

            if has_dibv5 || has_dib {
                let format = if has_dibv5 { CF_DIBV5 } else { CF_DIB };
                let handle = GetClipboardData(format);
                if !handle.is_null() {
                    let ptr = GlobalLock(handle);
                    let size = GlobalSize(handle);
                    if !ptr.is_null() && size > 0 {
                        let bytes =
                            std::slice::from_raw_parts(ptr as *const u8, size as usize).to_vec();
                        GlobalUnlock(handle);
                        Some(RawClipboardPayload::Image(bytes))
                    } else {
                        if !ptr.is_null() {
                            GlobalUnlock(handle);
                        }
                        None
                    }
                } else {
                    None
                }
            } else if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
                let handle = GetClipboardData(CF_UNICODETEXT);
                if !handle.is_null() {
                    let ptr = GlobalLock(handle);
                    let size = GlobalSize(handle);
                    if !ptr.is_null() && size >= 2 {
                        let u16_slice =
                            std::slice::from_raw_parts(ptr as *const u16, (size / 2) as usize);
                        let len = u16_slice
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(u16_slice.len());
                        let text = String::from_utf16_lossy(&u16_slice[..len]);
                        GlobalUnlock(handle);
                        Some(RawClipboardPayload::Text(text))
                    } else {
                        if !ptr.is_null() {
                            GlobalUnlock(handle);
                        }
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }; // _guard is dropped here; clipboard lock released!

    match payload? {
        RawClipboardPayload::Image(bytes) => {
            let rgba = parse_dib_to_rgba(&bytes)?;
            let (w, h) = (rgba.width(), rgba.height());
            if w == 0 || h == 0 {
                return None;
            }

            let mut png_bytes = Vec::new();
            let dyn_img = DynamicImage::ImageRgba8(rgba);
            dyn_img
                .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
                .ok()?;

            let hash_bytes = blake3::hash(&png_bytes);
            let hash_hex = hash_bytes.to_hex().to_string();

            // Save original PNG to disk
            let img_filename = format!("{}.png", hash_hex);
            let img_dest = images_dir.join(&img_filename);
            if !img_dest.exists() {
                let _ = fs::write(&img_dest, &png_bytes);
            }

            // Create high-quality thumbnail (Lanczos3 PNG)
            let thumb = dyn_img.resize(440, 320, FilterType::Lanczos3);
            let mut thumb_bytes = Vec::new();
            let _ = thumb.write_to(&mut Cursor::new(&mut thumb_bytes), ImageFormat::Png);
            let thumb_b64 = format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&thumb_bytes)
            );

            Some(ExtractedClipboard {
                content_type: "image".to_string(),
                text_content: None,
                char_count: None,
                width: Some(w),
                height: Some(h),
                thumbnail_b64: Some(thumb_b64),
                hash: hash_hex,
            })
        }
        RawClipboardPayload::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            let hash_bytes = blake3::hash(text.as_bytes());
            let hash_hex = hash_bytes.to_hex().to_string();
            let content_type = classify_text_content(&text);
            let char_count = text.chars().count() as i64;

            Some(ExtractedClipboard {
                content_type,
                text_content: Some(text),
                char_count: Some(char_count),
                width: None,
                height: None,
                thumbnail_b64: None,
                hash: hash_hex,
            })
        }
    }
}

pub fn classify_text_content(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || (trimmed.starts_with("www.") && !trimmed.contains('\n'))
    {
        return "link".to_string();
    }

    // Code heuristic
    if (trimmed.contains('{') && trimmed.contains('}'))
        || (trimmed.contains("fn ")
            || trimmed.contains("function ")
            || trimmed.contains("const ")
            || trimmed.contains("import ")
            || trimmed.contains("class "))
        || (trimmed.lines().count() > 2 && (trimmed.contains("    ") || trimmed.contains('\t')))
    {
        return "code".to_string();
    }

    "text".to_string()
}

pub fn set_clipboard_text(text: &str) -> Result<u32, String> {
    let _guard = open_clipboard_with_retry(ptr::null_mut())
        .ok_or_else(|| "Failed to open clipboard".to_string())?;
    unsafe {
        if EmptyClipboard() == 0 {
            return Err("Failed to empty clipboard".to_string());
        }

        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_len = utf16.len() * 2;

        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            return Err("Failed to allocate global memory".to_string());
        }

        let ptr = GlobalLock(h_mem);
        if ptr.is_null() {
            let _ = GlobalFree(h_mem);
            return Err("Failed to lock global memory".to_string());
        }

        ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, ptr as *mut u8, bytes_len);
        GlobalUnlock(h_mem);

        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            let _ = GlobalFree(h_mem);
            return Err("SetClipboardData failed".to_string());
        }

        Ok(GetClipboardSequenceNumber())
    }
}

pub fn set_clipboard_image(image_path: &Path) -> Result<u32, String> {
    let img = image::open(image_path)
        .map_err(|e| format!("open image: {e}"))?
        .to_rgba8();
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 || w > 32768 || h > 32768 {
        return Err("Invalid image dimensions".to_string());
    }

    let row_stride = (w as usize)
        .checked_mul(4)
        .ok_or_else(|| "Image width overflow".to_string())?;
    let image_data_size = row_stride
        .checked_mul(h as usize)
        .ok_or_else(|| "Image size overflow".to_string())?;
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let total_size = header_size
        .checked_add(image_data_size)
        .ok_or_else(|| "Total size overflow".to_string())?;

    let _guard = open_clipboard_with_retry(ptr::null_mut())
        .ok_or_else(|| "Failed to open clipboard".to_string())?;

    unsafe {
        if EmptyClipboard() == 0 {
            return Err("Failed to empty clipboard".to_string());
        }

        let h_mem = GlobalAlloc(GMEM_MOVEABLE, total_size);
        if h_mem.is_null() {
            return Err("Failed to allocate global memory".to_string());
        }

        let ptr = GlobalLock(h_mem);
        if ptr.is_null() {
            let _ = GlobalFree(h_mem);
            return Err("Failed to lock memory".to_string());
        }

        let header = ptr as *mut BITMAPINFOHEADER;
        (*header).biSize = header_size as u32;
        (*header).biWidth = w as i32;
        (*header).biHeight = h as i32; // positive = bottom-up
        (*header).biPlanes = 1;
        (*header).biBitCount = 32;
        (*header).biCompression = BI_RGB;
        (*header).biSizeImage = image_data_size as u32;
        (*header).biXPelsPerMeter = 3780;
        (*header).biYPelsPerMeter = 3780;
        (*header).biClrUsed = 0;
        (*header).biClrImportant = 0;

        let pixel_dst = (ptr as usize + header_size) as *mut u8;

        for y in 0..h {
            let src_y = h - 1 - y;
            let dst_row = pixel_dst.add(y as usize * row_stride);
            for x in 0..w {
                let pixel = img.get_pixel(x, src_y);
                let dst_pixel = dst_row.add(x as usize * 4);
                *dst_pixel = pixel[2]; // B
                *dst_pixel.add(1) = pixel[1]; // G
                *dst_pixel.add(2) = pixel[0]; // R
                *dst_pixel.add(3) = pixel[3]; // A
            }
        }

        GlobalUnlock(h_mem);

        if SetClipboardData(CF_DIB, h_mem).is_null() {
            let _ = GlobalFree(h_mem);
            return Err("SetClipboardData CF_DIB failed".to_string());
        }

        Ok(GetClipboardSequenceNumber())
    }
}

pub fn set_clipboard_files(file_paths: &[PathBuf]) -> Result<u32, String> {
    if file_paths.is_empty() {
        return Err("No files provided".to_string());
    }

    for p in file_paths {
        if !p.is_file() {
            return Err(format!("File does not exist: {}", p.display()));
        }
    }

    let _guard = open_clipboard_with_retry(ptr::null_mut())
        .ok_or_else(|| "Failed to open clipboard".to_string())?;

    use std::os::windows::ffi::OsStrExt;
    let mut wide_paths: Vec<u16> = Vec::new();
    for p in file_paths {
        let os_str = p.as_os_str();
        wide_paths.extend(os_str.encode_wide());
        wide_paths.push(0);
    }
    wide_paths.push(0); // double-null termination

    let dropfiles_size = std::mem::size_of::<DROPFILES>();
    let wide_paths_bytes = wide_paths.len() * std::mem::size_of::<u16>();
    let total_size = dropfiles_size + wide_paths_bytes;

    unsafe {
        if EmptyClipboard() == 0 {
            return Err("Failed to empty clipboard".to_string());
        }

        let h_mem = GlobalAlloc(GMEM_MOVEABLE, total_size);
        if h_mem.is_null() {
            return Err("Failed to allocate global memory for DROPFILES".to_string());
        }

        let ptr = GlobalLock(h_mem);
        if ptr.is_null() {
            let _ = GlobalFree(h_mem);
            return Err("Failed to lock memory".to_string());
        }

        let dropfiles = ptr as *mut DROPFILES;
        (*dropfiles).pFiles = dropfiles_size as u32;
        (*dropfiles).pt.x = 0;
        (*dropfiles).pt.y = 0;
        (*dropfiles).fNC = 0;
        (*dropfiles).fWide = 1;

        let dst_paths = (ptr as usize + dropfiles_size) as *mut u16;
        std::ptr::copy_nonoverlapping(wide_paths.as_ptr(), dst_paths, wide_paths.len());

        GlobalUnlock(h_mem);

        if SetClipboardData(CF_HDROP, h_mem).is_null() {
            let _ = GlobalFree(h_mem);
            return Err("SetClipboardData CF_HDROP failed".to_string());
        }

        Ok(GetClipboardSequenceNumber())
    }
}

fn parse_dib_to_rgba(data: &[u8]) -> Option<RgbaImage> {
    if data.len() < 40 {
        return None;
    }

    let bi_size = u32::from_le_bytes(data[0..4].try_into().ok()?);
    if bi_size < 40 || data.len() < bi_size as usize {
        return None;
    }

    let width = i32::from_le_bytes(data[4..8].try_into().ok()?);
    let height = i32::from_le_bytes(data[8..12].try_into().ok()?);
    let planes = u16::from_le_bytes(data[12..14].try_into().ok()?);
    let bit_count = u16::from_le_bytes(data[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(data[16..20].try_into().ok()?);

    if planes != 1 || width <= 0 || height == 0 || width > 32768 || height.unsigned_abs() > 32768 {
        return None;
    }
    if bit_count != 24 && bit_count != 32 {
        return None;
    }
    if compression != BI_RGB && compression != BI_BITFIELDS {
        return None;
    }

    let w = width as u32;
    let h = height.unsigned_abs();
    let is_bottom_up = height > 0;

    // Strict decoded-byte budget check BEFORE allocating memory (max 64 megapixels = 256MB)
    let total_pixels = (w as u64).checked_mul(h as u64)?;
    if total_pixels == 0 || total_pixels > 64 * 1024 * 1024 {
        return None;
    }

    let mut offset = bi_size as usize;

    let (mut r_mask, mut g_mask, mut b_mask, mut a_mask) =
        (0x00FF0000u32, 0x0000FF00u32, 0x000000FFu32, 0xFF000000u32);
    if compression == BI_BITFIELDS {
        if bi_size == 40 {
            if data.len() < 40 + 12 {
                return None;
            }
            r_mask = u32::from_le_bytes(data[40..44].try_into().ok()?);
            g_mask = u32::from_le_bytes(data[44..48].try_into().ok()?);
            b_mask = u32::from_le_bytes(data[48..52].try_into().ok()?);
            offset += 12;
        } else if bi_size >= 108 {
            r_mask = u32::from_le_bytes(data[40..44].try_into().ok()?);
            g_mask = u32::from_le_bytes(data[44..48].try_into().ok()?);
            b_mask = u32::from_le_bytes(data[48..52].try_into().ok()?);
            if bi_size >= 120 {
                a_mask = u32::from_le_bytes(data[52..56].try_into().ok()?);
            }
        }
        if r_mask == 0 || g_mask == 0 || b_mask == 0 {
            return None;
        }
        if (r_mask & g_mask) != 0 || (r_mask & b_mask) != 0 || (g_mask & b_mask) != 0 {
            return None;
        }
    }

    if offset > data.len() {
        return None;
    }

    let pixel_data = &data[offset..];
    let row_bits = (w as usize).checked_mul(bit_count as usize)?;
    let row_stride = row_bits.div_ceil(32).checked_mul(4)?;
    let total_bytes = row_stride.checked_mul(h as usize)?;

    if pixel_data.len() < total_bytes {
        return None;
    }

    let mut img = RgbaImage::new(w, h);

    if bit_count == 32 {
        let mut has_non_zero_alpha = false;

        for y in 0..h {
            let row_idx = if is_bottom_up { h - 1 - y } else { y };
            let row_bytes = &pixel_data[y as usize * row_stride..(y as usize + 1) * row_stride];

            for x in 0..w {
                let px_bytes = &row_bytes[x as usize * 4..(x as usize + 1) * 4];
                let Ok(arr) = px_bytes.try_into() else {
                    continue;
                };
                let val = u32::from_le_bytes(arr);

                let r = ((val & r_mask) >> r_mask.trailing_zeros()) as u8;
                let g = ((val & g_mask) >> g_mask.trailing_zeros()) as u8;
                let b = ((val & b_mask) >> b_mask.trailing_zeros()) as u8;
                let a = if a_mask != 0 {
                    ((val & a_mask) >> a_mask.trailing_zeros()) as u8
                } else {
                    255
                };

                if a > 0 {
                    has_non_zero_alpha = true;
                }

                img.put_pixel(x, row_idx, Rgba([r, g, b, a]));
            }
        }

        if !has_non_zero_alpha {
            for pixel in img.pixels_mut() {
                pixel[3] = 255;
            }
        }

        Some(img)
    } else if bit_count == 24 {
        for y in 0..h {
            let row_idx = if is_bottom_up { h - 1 - y } else { y };
            let row_bytes = &pixel_data[y as usize * row_stride..(y as usize + 1) * row_stride];

            for x in 0..w {
                let b = row_bytes[x as usize * 3];
                let g = row_bytes[x as usize * 3 + 1];
                let r = row_bytes[x as usize * 3 + 2];
                img.put_pixel(x, row_idx, Rgba([r, g, b, 255]));
            }
        }
        Some(img)
    } else {
        None
    }
}
