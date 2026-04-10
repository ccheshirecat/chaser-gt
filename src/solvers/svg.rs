//! SVG icon captcha solver for GeeTest v4.
//!
//! The SVG captcha type presents an animated 2x2 grid of icons cycling through
//! CSS keyframes. The user must identify which icon matches a prompt image and
//! click during the correct animation frame.
//!
//! This solver:
//! 1. Parses CSS keyframes to extract frame visibility timings
//! 2. Extracts each icon from each frame as standalone SVG
//! 3. Renders icons via `resvg` (pure Rust, no Cairo dependency)
//! 4. Compares rendered icons against the prompt using binarized IoU + correlation
//! 5. Returns the best-matching cell coordinates and a realistic passtime

use crate::error::{GeekedError, Result};
use image::{DynamicImage, GrayImage, RgbaImage};
use regex::Regex;

/// Grid cell [row, col] (1-based) matching GeeTest's JS click handler.
const CELL_ROWCOLS: [[i32; 2]; 4] = [
    [1, 1], // top-left
    [1, 2], // top-right
    [2, 1], // bottom-left
    [2, 2], // bottom-right
];

/// Standard render size for icon comparison (px).
const ICON_SIZE: u32 = 96;

/// Result from SVG captcha solving.
#[derive(Debug, Clone)]
pub struct SvgSolveResult {
    /// Grid position [row, col] (1-based)
    pub userresponse: [i32; 2],
    /// Realistic response time in ms
    pub passtime: u32,
}

/// Solver for GeeTest SVG icon captchas.
pub struct SvgSolver {
    svg_text: String,
    prompt_bytes: Vec<u8>,
}

/// Parsed keyframe visibility window (start_pct, end_pct).
#[derive(Debug, Clone)]
struct FrameTiming {
    start_pct: f64,
    end_pct: f64,
}

impl SvgSolver {
    /// Create a new SVG solver.
    ///
    /// # Arguments
    /// * `svg_text` - The full SVG markup string
    /// * `prompt_bytes` - Bytes of the prompt image (PNG/JPEG)
    pub fn new(svg_text: String, prompt_bytes: Vec<u8>) -> Self {
        Self {
            svg_text,
            prompt_bytes,
        }
    }

    /// Solve the SVG captcha.
    pub fn solve(&self) -> Result<SvgSolveResult> {
        // 1. Parse CSS keyframes and frame timings
        let style_content = extract_style(&self.svg_text);
        let keyframes = parse_keyframes(&style_content);
        let (frame_timings, duration_ms) = parse_frame_timings(&style_content, &keyframes);

        // 2. Parse SVG and extract frame icons
        let frames = extract_frames_and_icons(&self.svg_text)?;
        if frames.is_empty() {
            return Err(GeekedError::ImageProcessing(
                "No animation frames found in SVG".into(),
            ));
        }

        // 3. Prepare prompt image as grayscale
        let prompt_gray = prepare_prompt(&self.prompt_bytes)?;

        // 4. Render each icon and find best match
        let mut best_score: f64 = -1.0;
        let mut best_frame_idx = frames[0].0;
        let mut best_rowcol = CELL_ROWCOLS[0];

        for (frame_idx, ref icon_svgs) in &frames {
            for (ci, icon_svg) in icon_svgs.iter().enumerate() {
                if ci >= CELL_ROWCOLS.len() {
                    break;
                }

                let icon_gray = match render_icon_svg(icon_svg) {
                    Ok(g) => g,
                    Err(_) => continue,
                };

                let score = compare_icons(&prompt_gray, &icon_gray);
                if score > best_score {
                    best_score = score;
                    best_frame_idx = *frame_idx;
                    best_rowcol = CELL_ROWCOLS[ci];
                }
            }
        }

        // 5. Calculate passtime
        let passtime = calculate_passtime(best_frame_idx, &frame_timings, duration_ms);

        Ok(SvgSolveResult {
            userresponse: best_rowcol,
            passtime,
        })
    }
}

/// Extract <style> content from SVG markup.
fn extract_style(svg: &str) -> String {
    let re = Regex::new(r"(?s)<style>(.*?)</style>").unwrap();
    re.captures(svg)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Parse @keyframes visibility windows.
fn parse_keyframes(style: &str) -> Vec<(String, FrameTiming)> {
    let kf_re = Regex::new(
        r"@keyframes\s+(geetest_frame\d+_animation_\w+)\s*\{((?:[^{}]*\{[^{}]*\})*[^{}]*)\}",
    )
    .unwrap();
    let entry_re = Regex::new(r"([\d.]+)%\s*\{\s*opacity:\s*([01])\s*;?\s*\}").unwrap();

    let mut keyframes = Vec::new();

    for kf_match in kf_re.captures_iter(style) {
        let name = kf_match[1].to_string();
        let body = &kf_match[2];

        let mut visible_start: Option<f64> = None;

        for entry in entry_re.captures_iter(body) {
            let pct: f64 = entry[1].parse().unwrap_or(0.0);
            let val = &entry[2];

            if val == "1" && visible_start.is_none() {
                visible_start = Some(pct);
            } else if val == "0" {
                if let Some(start) = visible_start {
                    keyframes.push((
                        name.clone(),
                        FrameTiming {
                            start_pct: start,
                            end_pct: pct,
                        },
                    ));
                    visible_start = None;
                }
            }
        }

        if let Some(start) = visible_start {
            keyframes.push((
                name.clone(),
                FrameTiming {
                    start_pct: start,
                    end_pct: 100.0,
                },
            ));
        }
    }

    keyframes
}

/// Map frame indices to their visibility timing + extract animation duration.
fn parse_frame_timings(
    style: &str,
    keyframes: &[(String, FrameTiming)],
) -> (Vec<(usize, FrameTiming)>, f64) {
    let frame_re = Regex::new(
        r"\.geetest_frame_(\d+)_\w+\.geetest_frame_active_\w+\s*\{([^}]+)\}",
    )
    .unwrap();
    let dur_re = Regex::new(r"([\d.]+)s").unwrap();

    let mut duration_ms = 5000.0;
    let mut timings = Vec::new();

    for cap in frame_re.captures_iter(style) {
        let frame_idx: usize = cap[1].parse().unwrap_or(0);
        let rule_body = &cap[2];

        if let Some(dur_cap) = dur_re.captures(rule_body) {
            duration_ms = dur_cap[1].parse::<f64>().unwrap_or(5.0) * 1000.0;
        }

        for (kf_name, timing) in keyframes {
            if rule_body.contains(kf_name.as_str()) {
                timings.push((frame_idx, timing.clone()));
                break;
            }
        }
    }

    (timings, duration_ms)
}

/// Extract animation frames and their icon SVG snippets.
///
/// Returns: Vec of (frame_idx, Vec<icon_svg_string>)
fn extract_frames_and_icons(svg: &str) -> Result<Vec<(usize, Vec<String>)>> {
    // Find <g> elements with geetest_frame_active class
    let frame_re = Regex::new(
        r#"(?s)<g[^>]*class="[^"]*geetest_frame_(\d+)_\w+\s+geetest_frame_active_\w+[^"]*"[^>]*>(.*?)</g>\s*(?=<g[^>]*class="[^"]*geetest_frame_|$)"#,
    ).map_err(|e| GeekedError::ImageProcessing(format!("Regex error: {e}")))?;

    // Find inner <g> groups (icon containers)
    let icon_g_re = Regex::new(r"(?s)<g[^>]*>(.*?)</g>")
        .map_err(|e| GeekedError::ImageProcessing(format!("Regex error: {e}")))?;

    let mut frames = Vec::new();

    for cap in frame_re.captures_iter(svg) {
        let frame_idx: usize = cap[1].parse().unwrap_or(0);
        let frame_content = &cap[2];

        let mut icon_svgs = Vec::new();

        for icon_cap in icon_g_re.captures_iter(frame_content) {
            let inner = &icon_cap[1];
            // Skip empty groups or groups that are just rects (grid backgrounds)
            if inner.trim().is_empty() || (inner.contains("<rect") && !inner.contains("<path")) {
                continue;
            }

            // Build standalone SVG for this icon
            let icon_svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{ICON_SIZE}" height="{ICON_SIZE}" viewBox="0 0 48 48"><rect width="48" height="48" fill="white"/>{inner}</svg>"#
            );
            icon_svgs.push(icon_svg);
        }

        if !icon_svgs.is_empty() {
            frames.push((frame_idx, icon_svgs));
        }
    }

    Ok(frames)
}

/// Render a standalone SVG string to a grayscale image.
fn render_icon_svg(svg_str: &str) -> Result<GrayImage> {
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg_str, &opts)
        .map_err(|e| GeekedError::ImageProcessing(format!("SVG parse error: {e}")))?;

    let size = tree.size();
    let width = ICON_SIZE;
    let height = ICON_SIZE;

    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| GeekedError::ImageProcessing("Failed to create pixmap".into()))?;

    // Fill with white background
    pixmap.fill(tiny_skia::Color::WHITE);

    let sx = width as f32 / size.width();
    let sy = height as f32 / size.height();
    let transform = tiny_skia::Transform::from_scale(sx, sy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert RGBA pixmap to grayscale
    let rgba_data = pixmap.data();
    let rgba_img =
        RgbaImage::from_raw(width, height, rgba_data.to_vec()).ok_or_else(|| {
            GeekedError::ImageProcessing("Failed to create RGBA image".into())
        })?;

    let dynamic = DynamicImage::ImageRgba8(rgba_img);
    Ok(dynamic.to_luma8())
}

/// Prepare prompt image bytes as a grayscale image at ICON_SIZE.
fn prepare_prompt(bytes: &[u8]) -> Result<GrayImage> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| GeekedError::ImageProcessing(format!("Failed to load prompt: {e}")))?;

    let resized = img.resize_exact(ICON_SIZE, ICON_SIZE, image::imageops::CatmullRom);
    Ok(resized.to_luma8())
}

/// Compare two grayscale icon images using IoU + pixel correlation.
fn compare_icons(prompt: &GrayImage, icon: &GrayImage) -> f64 {
    let threshold = 180u8;

    let mut p_fg_count = 0u64;
    let mut i_fg_count = 0u64;
    let mut intersection = 0u64;
    let mut union = 0u64;

    let p_pixels: Vec<bool> = prompt
        .pixels()
        .map(|p| p[0] < threshold) // foreground = dark pixels
        .collect();
    let i_pixels: Vec<bool> = icon
        .pixels()
        .map(|p| p[0] < threshold)
        .collect();

    for (&p, &i) in p_pixels.iter().zip(i_pixels.iter()) {
        if p {
            p_fg_count += 1;
        }
        if i {
            i_fg_count += 1;
        }
        if p && i {
            intersection += 1;
        }
        if p || i {
            union += 1;
        }
    }

    let iou = if union > 0 {
        intersection as f64 / union as f64
    } else {
        0.0
    };

    // Pearson correlation on binary images
    let n = p_pixels.len() as f64;
    let p_mean = p_fg_count as f64 / n;
    let i_mean = i_fg_count as f64 / n;

    let mut cov = 0.0;
    let mut p_var = 0.0;
    let mut i_var = 0.0;

    for (&p, &i) in p_pixels.iter().zip(i_pixels.iter()) {
        let pv = if p { 1.0 } else { 0.0 } - p_mean;
        let iv = if i { 1.0 } else { 0.0 } - i_mean;
        cov += pv * iv;
        p_var += pv * pv;
        i_var += iv * iv;
    }

    let corr = if p_var > 0.0 && i_var > 0.0 {
        cov / (p_var.sqrt() * i_var.sqrt())
    } else {
        0.0
    };

    iou * 0.5 + corr * 0.5
}

/// Calculate a realistic passtime based on when the matched frame is visible.
fn calculate_passtime(
    frame_idx: usize,
    frame_timings: &[(usize, FrameTiming)],
    duration_ms: f64,
) -> u32 {
    let timing = frame_timings
        .iter()
        .find(|(idx, _)| *idx == frame_idx)
        .map(|(_, t)| t.clone())
        .unwrap_or(FrameTiming {
            start_pct: 0.0,
            end_pct: 100.0,
        });

    let visible_start_ms = (duration_ms * timing.start_pct / 100.0) as u32;
    let mut visible_end_ms = (duration_ms * timing.end_pct / 100.0) as u32;

    // Human reaction time: 300-800ms after frame appears
    let reaction_ms = rand::random::<u32>() % 500 + 300;
    let mut earliest = visible_start_ms + reaction_ms;

    if earliest < 500 {
        earliest = 500;
    }
    if visible_end_ms <= earliest {
        visible_end_ms = earliest + 500;
    }

    earliest + rand::random::<u32>() % (visible_end_ms - earliest + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    #[test]
    fn test_parse_keyframes_empty() {
        let kf = parse_keyframes("");
        assert!(kf.is_empty());
    }

    #[test]
    fn test_compare_identical_images() {
        let img = GrayImage::from_fn(ICON_SIZE, ICON_SIZE, |x, _| {
            if x < ICON_SIZE / 2 {
                Luma([0])
            } else {
                Luma([255])
            }
        });
        let score = compare_icons(&img, &img);
        assert!(score > 0.9, "Identical images should score > 0.9, got {score}");
    }
}
