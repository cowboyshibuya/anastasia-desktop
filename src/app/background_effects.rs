use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anastasia_protocol::settings::{NewThreadBackgroundEffect, NewThreadComposerBackground};
use gpui::{App, Pixels, RenderImage, px, size};

use crate::theme::Theme;

pub const NEW_THREAD_BACKGROUND_DIR: &str = "new-thread-backgrounds";

pub fn decode(bytes: &[u8]) -> image::ImageResult<image::DynamicImage> {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()?
        .decode()
}

type EffectEntry = (
    (NewThreadBackgroundEffect, bool),
    Option<Arc<RenderImage>>,
);

#[derive(Debug)]
struct BackgroundLuminance {
    width: u32,
    height: u32,
    pixels: Box<[u8]>,
    colors: Box<[[u8; 4]]>,
    effects: Mutex<Vec<EffectEntry>>,
}

impl BackgroundLuminance {
    fn raster_image(
        self: &Arc<Self>,
        effect: NewThreadBackgroundEffect,
        light: bool,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        let mut effects = self.effects.lock().unwrap();
        let key = (
            effect,
            light
                && !matches!(
                    effect,
                    NewThreadBackgroundEffect::Dither | NewThreadBackgroundEffect::None
                ),
        );
        if let Some((_, image)) = effects.iter().find(|(cached, _)| *cached == key) {
            return image.clone();
        }
        effects.push((key, None));
        drop(effects);

        let source = self.clone();
        cx.spawn(async move |cx| {
            let worker = source.clone();
            let image = cx
                .background_executor()
                .spawn(async move {
                    let pixels = match effect {
                        NewThreadBackgroundEffect::None => {
                            image::RgbaImage::from_fn(worker.width, worker.height, |x, y| {
                                let [r, g, b, a] = worker.colors[(y * worker.width + x) as usize];
                                image::Rgba([b, g, r, a])
                            })
                        }
                        NewThreadBackgroundEffect::Dither => {
                            worker.dither_pixels(worker.width, worker.height)
                        }
                        NewThreadBackgroundEffect::Halftone => {
                            worker.halftone_pixels(worker.width, worker.height, light)
                        }
                        NewThreadBackgroundEffect::Ascii => worker.ascii_pixels(light),
                        NewThreadBackgroundEffect::Scanlines => worker.scanline_pixels(light),
                    };
                    Arc::new(RenderImage::new([image::Frame::new(pixels)]))
                })
                .await;

            cx.update(|cx| {
                if let Some((_, ready)) = source
                    .effects
                    .lock()
                    .unwrap()
                    .iter_mut()
                    .find(|(cached, _)| *cached == key)
                {
                    *ready = Some(image);
                }
                cx.refresh_windows();
            });
        })
        .detach();

        None
    }

    fn scanline_pixels(&self, light: bool) -> image::RgbaImage {
        image::RgbaImage::from_fn(self.width, self.height, |x, y| {
            let [r, g, b, a] = self.colors[(y * self.width + x) as usize];
            let gain = if y % 3 == 0 { 0.52 } else { 1.0 };
            let channel = |value: u8| {
                if light {
                    (value as f32 + (255.0 - value as f32) * (1.0 - gain)) as u8
                } else {
                    (value as f32 * gain) as u8
                }
            };
            image::Rgba([channel(b), channel(g), channel(r), a])
        })
    }

    fn ascii_pixels(&self, light: bool) -> image::RgbaImage {
        const GLYPHS: [[u8; 7]; 10] = [
            [0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 4, 0],
            [0, 4, 0, 0, 4, 0, 0],
            [0, 0, 0, 14, 0, 0, 0],
            [0, 0, 14, 0, 14, 0, 0],
            [0, 4, 4, 31, 4, 4, 0],
            [0, 21, 14, 31, 14, 21, 0],
            [10, 10, 31, 10, 31, 10, 10],
            [17, 2, 4, 4, 8, 16, 17],
            [14, 17, 23, 21, 23, 16, 14],
        ];

        image::RgbaImage::from_fn(self.width, self.height, |x, y| {
            let sx = (x / 6 * 6 + 3).min(self.width - 1);
            let sy = (y / 8 * 8 + 4).min(self.height - 1);
            let sample = (sy * self.width + sx) as usize;
            let ink_density = if light {
                255 - self.pixels[sample]
            } else {
                self.pixels[sample]
            };
            let index = ((ink_density as f32 / 255.0).sqrt() * 9.0) as usize;
            let ink =
                x % 6 < 5 && y % 8 < 7 && GLYPHS[index][y as usize % 8] & (1 << (4 - x % 6)) != 0;
            let [r, g, b, a] = self.colors[(y * self.width + x) as usize];
            let [cr, cg, cb, _] = self.colors[sample];
            let mix = |base: u8, glyph: u8| {
                let paper = if light { 255.0 } else { 0.0 };
                (base as f32 * 0.60
                    + if ink {
                        glyph as f32 * 0.40
                    } else {
                        paper * 0.40
                    }) as u8
            };
            image::Rgba([mix(b, cb), mix(g, cg), mix(r, cr), a])
        })
    }

    fn halftone_pixels(&self, width: u32, height: u32, light: bool) -> image::RgbaImage {
        let bounds = size(px(width as f32), px(height as f32));
        let paper = if light { 255 } else { 0 };
        let mut pixels =
            image::RgbaImage::from_pixel(width, height, image::Rgba([paper, paper, paper, 255]));

        for y in (0..height).step_by(4) {
            for x in (0..width).step_by(4) {
                let luma = self.sample_cover(bounds, x as f32, y as f32);
                let luma = if light { 255 - luma } else { luma };
                let radius = 2.0 * (0.3 + 0.7 * (luma as f32 / 255.0).sqrt());
                let [r, g, b, a] =
                    self.colors[self.cover_index(bounds, x as f32 + 2.0, y as f32 + 2.0)];
                for dy in 0..4.min(height - y) {
                    for dx in 0..4.min(width - x) {
                        let distance =
                            ((dx as f32 - 1.5).powi(2) + (dy as f32 - 1.5).powi(2)).sqrt();
                        let coverage =
                            (radius + 0.5 - distance).clamp(0.0, 1.0) * a as f32 / 255.0;
                        let [sr, sg, sb, sa] = self.colors[((y + dy) * width + x + dx) as usize];
                        let blend = |source: u8, dot: u8| {
                            (source as f32 * 0.60
                                + (dot as f32 * coverage + paper as f32 * (1.0 - coverage))
                                    * 0.40) as u8
                        };
                        pixels.put_pixel(
                            x + dx,
                            y + dy,
                            image::Rgba([blend(sb, b), blend(sg, g), blend(sr, r), sa]),
                        );
                    }
                }
            }
        }
        pixels
    }

    fn dither_pixels(&self, width: u32, height: u32) -> image::RgbaImage {
        const BAYER: [[u8; 4]; 4] =
            [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        let bounds = size(px(width as f32), px(height as f32));
        let mut pixels = image::RgbaImage::new(width, height);

        for y in (0..height).step_by(2) {
            for x in (0..width).step_by(2) {
                let index = self.cover_index(bounds, (x + 1) as f32, (y + 1) as f32);
                let [r, g, b, a] = dither_color(
                    self.colors[index],
                    BAYER[y as usize / 2 % 4][x as usize / 2 % 4],
                );
                for dy in 0..2.min(height - y) {
                    for dx in 0..2.min(width - x) {
                        pixels.put_pixel(x + dx, y + dy, image::Rgba([b, g, r, a]));
                    }
                }
            }
        }
        pixels
    }

    fn sample_cover(&self, bounds: gpui::Size<Pixels>, x: f32, y: f32) -> u8 {
        self.pixels[self.cover_index(bounds, x, y)]
    }

    fn cover_index(&self, bounds: gpui::Size<Pixels>, x: f32, y: f32) -> usize {
        let width = f32::from(bounds.width).max(1.0);
        let height = f32::from(bounds.height).max(1.0);
        let source_width = self.width as f32;
        let source_height = self.height as f32;
        let scale = (width / source_width).max(height / source_height);
        let visible_width = width / scale;
        let visible_height = height / scale;
        let source_x = ((source_width - visible_width) * 0.5 + x / scale)
            .clamp(0.0, source_width - 1.0) as u32;
        let source_y = ((source_height - visible_height) * 0.5 + y / scale)
            .clamp(0.0, source_height - 1.0) as u32;
        (source_y * self.width + source_x) as usize
    }
}

fn dither_color([r, g, b, a]: [u8; 4], threshold: u8) -> [u8; 4] {
    let peak = r.max(g).max(b) as f32;
    let bright = peak / 255.0 > (threshold as f32 + 0.5) / 16.0;
    let gain = if bright { 255.0 / peak.max(1.0) } else { 0.08 };
    [
        (r as f32 * gain).round() as u8,
        (g as f32 * gain).round() as u8,
        (b as f32 * gain).round() as u8,
        a,
    ]
}

fn background_luminance(path: &Path, cx: &mut App) -> Option<Arc<BackgroundLuminance>> {
    type Source = Arc<Mutex<Option<Arc<BackgroundLuminance>>>>;
    type Cache = Vec<(PathBuf, Source)>;
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));

    if let Some(source) = cache
        .lock()
        .ok()?
        .iter()
        .find_map(|(key, source)| (key == path).then(|| source.clone()))
    {
        return source.lock().ok()?.clone();
    }

    let pending = Arc::new(Mutex::new(None));
    {
        let mut cache = cache.lock().ok()?;
        cache.push((path.to_path_buf(), pending.clone()));
        if cache.len() > 4 {
            cache.remove(0);
        }
    }

    let path = path.to_path_buf();
    cx.spawn(async move |cx| {
        let source = cx
            .background_executor()
            .spawn(async move {
                let bytes = std::fs::read(path).ok()?;
                let proxy = decode(&bytes).ok()?.thumbnail(2048, 2048);
                let gray = proxy.to_luma8();
                Some(Arc::new(BackgroundLuminance {
                    width: gray.width(),
                    height: gray.height(),
                    pixels: gray.into_raw().into_boxed_slice(),
                    colors: proxy.to_rgba8().pixels().map(|pixel| pixel.0).collect(),
                    effects: Mutex::new(Vec::new()),
                }))
            })
            .await;
        cx.update(|cx| {
            *pending.lock().unwrap() = source;
            cx.refresh_windows();
        });
    })
    .detach();

    None
}

/// Prepares the rasterized image for the given effect.
/// Asynchronously generates and caches the image. Safe to call every frame.
pub fn prepare(
    effect: NewThreadBackgroundEffect,
    theme: &Theme,
    path: &Path,
    cx: &mut App,
) -> Option<Arc<RenderImage>> {
    let light = !theme.is_dark;
    background_luminance(path, cx).and_then(|source| source.raster_image(effect, light, cx))
}

pub fn install_new_thread_composer_background(
    source: &Path,
    cx: &mut App,
) -> Result<NewThreadComposerBackground, String> {
    let bytes = std::fs::read(source).map_err(|e| format!("Failed to read file: {e}"))?;
    decode(&bytes).map_err(|_| {
        "This background image is unsupported or damaged. Choose a valid PNG, JPEG, or WebP image."
            .to_string()
    })?;

    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("background.png")
        .to_string();

    let data_dir = crate::identity::desktop_data_dir();
    let backgrounds_dir = data_dir.join(NEW_THREAD_BACKGROUND_DIR);
    std::fs::create_dir_all(&backgrounds_dir).map_err(|_| {
        "Unable to create backgrounds directory. Check permissions and try again.".to_string()
    })?;

    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png");

    let destination = backgrounds_dir.join(format!(
        "new-thread-background-{}.{}",
        uuid::Uuid::new_v4(),
        extension
    ));

    std::fs::write(&destination, &bytes).map_err(|_| {
        "Unable to save background image. Check folder permissions and try again.".to_string()
    })?;

    cx.refresh_windows();

    Ok(NewThreadComposerBackground {
        path: destination.to_string_lossy().into_owned(),
        name: file_name,
    })
}

pub fn remove_managed_new_thread_background(
    background: Option<&NewThreadComposerBackground>,
) {
    if let Some(bg) = background {
        let path = Path::new(&bg.path);
        let data_dir = crate::identity::desktop_data_dir().join(NEW_THREAD_BACKGROUND_DIR);
        if path.starts_with(&data_dir) && path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_image_bytes() -> Vec<u8> {
        let img = image::RgbaImage::from_fn(16, 16, |x, y| {
            image::Rgba([(x * 16) as u8, (y * 16) as u8, 128, 255])
        });
        let mut bytes = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        img.write_with_encoder(encoder).unwrap();
        bytes
    }

    #[test]
    fn decode_valid_png() {
        let bytes = sample_image_bytes();
        let decoded = decode(&bytes);
        assert!(decoded.is_ok());
        let img = decoded.unwrap();
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
    }

    #[test]
    fn decode_invalid_bytes_returns_error() {
        assert!(decode(b"not an image").is_err());
    }

    #[test]
    fn retro_effects_preserve_valid_dimensions() {
        let bytes = sample_image_bytes();
        let dynamic = decode(&bytes).unwrap();
        let gray = dynamic.to_luma8();
        let luminance = BackgroundLuminance {
            width: gray.width(),
            height: gray.height(),
            pixels: gray.into_raw().into_boxed_slice(),
            colors: dynamic.to_rgba8().pixels().map(|p| p.0).collect(),
            effects: Mutex::new(Vec::new()),
        };

        let scanlines = luminance.scanline_pixels(false);
        assert_eq!(scanlines.width(), 16);
        assert_eq!(scanlines.height(), 16);

        let ascii = luminance.ascii_pixels(false);
        assert_eq!(ascii.width(), 16);
        assert_eq!(ascii.height(), 16);

        let halftone = luminance.halftone_pixels(16, 16, false);
        assert_eq!(halftone.width(), 16);
        assert_eq!(halftone.height(), 16);

        let dither = luminance.dither_pixels(16, 16);
        assert_eq!(dither.width(), 16);
        assert_eq!(dither.height(), 16);
    }

    #[test]
    fn remove_managed_background_ignores_external_paths() {
        let external = NewThreadComposerBackground {
            path: "/tmp/some_external_image.png".to_string(),
            name: "external.png".to_string(),
        };
        // Calling remove should not panic or delete files outside of .anastasia/backgrounds
        remove_managed_new_thread_background(Some(&external));
        remove_managed_new_thread_background(None);
    }
}
