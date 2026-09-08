use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use base64::Engine;
use image::{imageops::FilterType, ImageFormat};

#[derive(Clone)]
pub struct ThumbnailService {
    cache_dir: PathBuf,
}

impl ThumbnailService {
    pub fn new(app_data_dir: &Path) -> Self {
        let cache_dir = app_data_dir.join("cache").join("thumbnails");
        let _ = fs::create_dir_all(&cache_dir);
        Self { cache_dir }
    }

    pub fn get_or_generate_thumbnail(
        &self,
        source_path: &str,
        target_w: u32,
        target_h: u32,
        dpr: f32,
    ) -> Result<String, String> {
        let path = Path::new(source_path);
        if !path.exists() {
            return Err("File not found".to_string());
        }

        let mtime = fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mtime_secs = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let dpr_val = if dpr <= 0.0 { 1.0 } else { dpr };
        let phys_w = ((target_w as f32) * dpr_val).round() as u32;
        let phys_h = ((target_h as f32) * dpr_val).round() as u32;

        let phys_w = phys_w.clamp(32, 1920);
        let phys_h = phys_h.clamp(32, 1920);

        // Cache Key
        let key_input = format!(
            "{}:{}:{}:{}:{}",
            source_path, mtime_secs, phys_w, phys_h, dpr_val
        );
        let hash = blake3::hash(key_input.as_bytes()).to_hex().to_string();
        let cached_file = self.cache_dir.join(format!("{}.png", hash));

        // Return cached if exists
        if cached_file.exists() {
            if let Ok(bytes) = fs::read(&cached_file) {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(format!("data:image/png;base64,{}", b64));
            }
        }

        // Generate high-quality thumbnail
        let img = image::open(path).map_err(|e| format!("Failed to open image: {e}"))?;
        let orig_w = img.width();
        let orig_h = img.height();

        if orig_w == 0 || orig_h == 0 {
            return Err("Invalid image dimensions".to_string());
        }

        // Do not upscale if already smaller than target physical size
        let final_w = if orig_w < phys_w { orig_w } else { phys_w };
        let final_h = if orig_h < phys_h { orig_h } else { phys_h };

        // High-quality Lanczos3 filter
        let resized = img.resize(final_w, final_h, FilterType::Lanczos3);

        let mut buffer = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .map_err(|e| format!("Failed to encode thumbnail: {e}"))?;

        // Save to disk cache atomically
        atomic_write_cache_file(&cached_file, &buffer);
        enforce_cache_budget(&self.cache_dir, 100 * 1024 * 1024);

        let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
        Ok(format!("data:image/png;base64,{}", b64))
    }

    pub fn get_or_generate_wallpaper_thumbnail(
        &self,
        source_path: &str,
        target_w: u32,
        target_h: u32,
    ) -> Result<String, String> {
        let path = Path::new(source_path);
        if !path.is_file() {
            return Err("Файл обоев не найден".to_string());
        }

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if !["jpg", "jpeg", "png", "webp"].contains(&ext.as_str()) {
            return Err(format!("Unsupported image extension: {ext}"));
        }

        let meta = fs::metadata(path).map_err(|e| format!("metadata error: {e}"))?;
        let file_size = meta.len();
        let mtime = meta
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let wallpaper_cache_dir = self
            .cache_dir
            .parent()
            .unwrap_or(&self.cache_dir)
            .join("wallpapers");
        let _ = fs::create_dir_all(&wallpaper_cache_dir);

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let key_input = format!(
            "wp_v3:{}:{}:{}:{}:{}",
            canonical.to_string_lossy(),
            mtime,
            file_size,
            target_w,
            target_h
        );
        let hash = blake3::hash(key_input.as_bytes()).to_hex().to_string();
        let cached_file = wallpaper_cache_dir.join(format!("{}.png", hash));

        if cached_file.exists() {
            if let Ok(bytes) = fs::read(&cached_file) {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Ok(format!("data:image/png;base64,{}", b64));
            }
        }

        // Open image and create clean cover thumbnail
        let img = image::open(path).map_err(|e| format!("Ошибка открытия изображения: {e}"))?;
        let orig_w = img.width();
        let orig_h = img.height();

        if orig_w == 0 || orig_h == 0 {
            return Err("Неверные размеры изображения".to_string());
        }

        let resized = img.resize_to_fill(target_w, target_h, FilterType::Lanczos3);

        let mut buffer = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .map_err(|e| format!("Ошибка кодирования thumbnail: {e}"))?;

        // Save to disk cache atomically
        atomic_write_cache_file(&cached_file, &buffer);
        enforce_cache_budget(&wallpaper_cache_dir, 100 * 1024 * 1024);

        let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
        Ok(format!("data:image/png;base64,{}", b64))
    }
}

fn atomic_write_cache_file(target: &Path, bytes: &[u8]) {
    let tmp = target.with_extension(format!("tmp.{}", std::process::id()));
    if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, target).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

fn enforce_cache_budget(dir: &Path, max_bytes: u64) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
        let mut total_size = 0u64;
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    let len = meta.len();
                    total_size += len;
                    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    files.push((entry.path(), len, mtime));
                }
            }
        }

        if total_size > max_bytes {
            files.sort_by_key(|(_, _, mtime)| *mtime);
            for (path, len, _) in files {
                let _ = fs::remove_file(path);
                total_size = total_size.saturating_sub(len);
                if total_size <= max_bytes * 3 / 4 {
                    break;
                }
            }
        }
    }
}
