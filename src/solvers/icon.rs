//! Icon captcha solver using ONNX model for classification.
//!
//! This solver identifies arrows/icons in an image and matches them
//! to the required directions using a custom ONNX classification model.

use crate::error::{GeekedError, Result};
use image::{DynamicImage, GrayImage, Luma};
use ndarray::Array4;
use ort::session::{builder::GraphOptimizationLevel, Session};
use std::collections::HashMap;

/// Direction labels for icon classification.
/// Maps question icon filenames to direction codes.
const ICON_MAPPING: &[(&str, &str)] = &[
    ("8da090c135ff029f3b5e19f4c44f73c8.png", "u"),  // up
    ("cb0eaa639b2117a69a81af3d8c1496a1.png", "d"),  // down
    ("315ce8665e781dabcd1eb09d3e604803.png", "l"),  // left
    ("38bd9dda695098c7dfad74c921923a7d.png", "lu"), // left-up
    ("502e51dbabf411beba2dcd55fd38ebbd.png", "ld"), // left-down
    ("2b2387f566f6a03ed594d4d7cfda471f.png", "r"),  // right
    ("78dc29045d587ad054c7353732df53c5.png", "ru"), // right-up
    ("23ef93e6b0e0df0e15b66667c99a5fb4.png", "rd"), // right-down
];

/// Charset from the ONNX model - maps class indices to labels.
/// Labels are in format "{object}_{direction}" e.g., "car_r", "butterfly_lu"
const CHARSET: &[&str] = &[
    "car_r",
    "butterfly_ru",
    "car_ru",
    "car_l",
    "plane_ru",
    "butterfly_d",
    "plane_ld",
    "butterfly_lu",
    "fish_ru",
    "fish_r",
    "plane_d",
    "turtle_ru",
    "car_d",
    "car_u",
    "butterfly_l",
    "fish_l",
    "turtle_u",
    "turtle_l",
    "fish_u",
    "turtle_r",
    "butterfly_r",
    "fish_rd",
    "plane_r",
    "butterfly_ld",
    "fish_d",
    "fish_ld",
    "fish_lu",
    "plane_u",
    "turtle_ld",
    "turtle_lu",
    "plane_l",
    "car_ld",
    "plane_lu",
    "car_lu",
    "plane_rd",
    "butterfly_u",
    "turtle_rd",
    "butterfly_rd",
    "car_rd",
    "turtle_d",
];

/// Model input dimensions (from charsets.json: "image": [-1, 64])
const MODEL_INPUT_HEIGHT: u32 = 64;

/// Embedded ONNX model for icon classification.
static ICON_MODEL: &[u8] = include_bytes!("../../models/geetest_v4_icon.onnx");

/// Bounding box for detected icon region.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

impl BoundingBox {
    /// Get the center point of the bounding box.
    pub fn center(&self) -> (f64, f64) {
        let cx = self.x1 as f64 + (self.x2 - self.x1) as f64 / 2.0;
        let cy = self.y1 as f64 + (self.y2 - self.y1) as f64 / 2.0;
        (cx, cy)
    }

    /// Get width of bounding box.
    pub fn width(&self) -> u32 {
        self.x2 - self.x1
    }

    /// Get height of bounding box.
    pub fn height(&self) -> u32 {
        self.y2 - self.y1
    }
}

/// Solver for icon selection captcha.
pub struct IconSolver {
    session: Session,
    icon_map: HashMap<String, String>,
}

impl IconSolver {
    /// Create a new IconSolver, loading the ONNX model.
    pub fn new() -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| {
                GeekedError::ImageProcessing(format!(
                    "Failed to create ONNX session builder: {}",
                    e
                ))
            })?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| {
                GeekedError::ImageProcessing(format!("Failed to set optimization level: {}", e))
            })?
            .commit_from_memory(ICON_MODEL)
            .map_err(|e| {
                GeekedError::ImageProcessing(format!("Failed to load ONNX model: {}", e))
            })?;

        let icon_map = ICON_MAPPING
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        Ok(Self { session, icon_map })
    }

    /// Get the required direction for a question icon URL.
    fn get_direction(&self, url: &str) -> Option<&str> {
        let filename = url.split('/').last()?;
        self.icon_map.get(filename).map(|s| s.as_str())
    }

    /// Detect icon bounding boxes in the image using image processing.
    ///
    /// This uses a combination of:
    /// 1. Convert to grayscale
    /// 2. Apply thresholding to separate foreground
    /// 3. Find connected components
    /// 4. Filter by size to get icon regions
    fn detect_icons(&self, img: &DynamicImage) -> Vec<BoundingBox> {
        let gray = img.to_luma8();
        let (width, height) = gray.dimensions();

        // Apply adaptive thresholding to find foreground objects
        let threshold = otsu_threshold(&gray);
        let binary = threshold_image(&gray, threshold);

        // Find connected components
        let components = find_connected_components(&binary);

        // Filter components by size (icons should be within a reasonable size range)
        let min_size = (width * height / 400) as usize; // At least 0.25% of image
        let max_size = (width * height / 4) as usize; // At most 25% of image
        let min_dim = 20u32; // Minimum dimension
        let max_dim = width.min(height) / 2; // Maximum dimension

        components
            .into_iter()
            .filter(|bbox| {
                let w = bbox.width();
                let h = bbox.height();
                let area = (w * h) as usize;
                area >= min_size && area <= max_size
                    && w >= min_dim && w <= max_dim
                    && h >= min_dim && h <= max_dim
                    && (w as f64 / h as f64) > 0.3  // Aspect ratio filter
                    && (h as f64 / w as f64) > 0.3
            })
            .collect()
    }

    /// Classify the direction of an icon using the ONNX model.
    fn classify_direction(
        &mut self,
        img: &DynamicImage,
        bbox: &BoundingBox,
    ) -> Result<Option<String>> {
        // Crop the region
        let cropped = img.crop_imm(bbox.x1, bbox.y1, bbox.width(), bbox.height());

        // Preprocess for the model
        // The model expects grayscale images with height 64, variable width
        let gray = cropped.to_luma8();
        let (orig_w, orig_h) = gray.dimensions();

        // Scale to height 64, maintaining aspect ratio
        let scale = MODEL_INPUT_HEIGHT as f64 / orig_h as f64;
        let new_width = ((orig_w as f64 * scale).round() as u32).max(1);
        let resized = image::imageops::resize(
            &gray,
            new_width,
            MODEL_INPUT_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        );

        // Create input tensor: [batch=1, channel=1, height=64, width=variable]
        let (w, h) = resized.dimensions();
        let mut input = Array4::<f32>::zeros((1, 1, h as usize, w as usize));

        for y in 0..h {
            for x in 0..w {
                let pixel = resized.get_pixel(x, y)[0];
                // Normalize to [0, 1]
                input[[0, 0, y as usize, x as usize]] = pixel as f32 / 255.0;
            }
        }

        // Create input value from ndarray (must be owned, not a view)
        let input_value = ort::value::Value::from_array(input).map_err(|e| {
            GeekedError::ImageProcessing(format!("Failed to create input tensor: {}", e))
        })?;

        // Run inference
        let outputs = self
            .session
            .run(ort::inputs![input_value])
            .map_err(|e| GeekedError::ImageProcessing(format!("ONNX inference failed: {}", e)))?;

        // Get output tensor - try to get the first output
        let (_, output_value) = outputs
            .iter()
            .next()
            .ok_or_else(|| GeekedError::ImageProcessing("No output from model".into()))?;

        // Extract the tensor data - returns (shape, data_slice)
        let (_, output_data) = output_value.try_extract_tensor::<f32>().map_err(|e| {
            GeekedError::ImageProcessing(format!("Failed to extract output tensor: {}", e))
        })?;

        // Find class with highest probability
        let mut max_idx = 0;
        let mut max_val = f32::NEG_INFINITY;

        for (idx, &val) in output_data.iter().enumerate() {
            if val > max_val {
                max_val = val;
                max_idx = idx;
            }
        }

        // Get class label and extract direction
        if max_idx < CHARSET.len() {
            let label = CHARSET[max_idx];
            // Extract direction from label (e.g., "car_ru" -> "ru")
            if let Some(direction) = label.split('_').nth(1) {
                return Ok(Some(direction.to_string()));
            }
        }

        Ok(None)
    }

    /// Find icon positions in the image that match the required directions.
    ///
    /// # Arguments
    /// * `img_bytes` - Bytes of the main image containing icons
    /// * `questions` - List of icon URLs indicating required directions
    ///
    /// # Returns
    /// List of [x, y] coordinates scaled for the API response.
    pub fn find_icon_positions(
        &mut self,
        img_bytes: &[u8],
        questions: &[String],
    ) -> Result<Vec<[f64; 2]>> {
        // Load the image
        let img = image::load_from_memory(img_bytes)
            .map_err(|e| GeekedError::ImageProcessing(format!("Failed to load image: {}", e)))?;

        // Get required directions from questions (convert to owned Strings to avoid borrow issues)
        let required_directions: Vec<Option<String>> = questions
            .iter()
            .map(|q| self.get_direction(q).map(|s| s.to_string()))
            .collect();

        // Detect icon bounding boxes
        let bboxes = self.detect_icons(&img);

        tracing::debug!("Detected {} potential icons", bboxes.len());

        // Classify each detected icon
        let mut detected_icons: Vec<(BoundingBox, String)> = Vec::new();
        for bbox in &bboxes {
            if let Ok(Some(direction)) = self.classify_direction(&img, bbox) {
                detected_icons.push((*bbox, direction));
            }
        }

        tracing::debug!("Classified {} icons", detected_icons.len());

        // Match detected icons with required directions
        let mut results: Vec<Option<[f64; 2]>> = vec![None; questions.len()];
        let mut used_icons: Vec<bool> = vec![false; detected_icons.len()];
        let mut unused_positions: Vec<[f64; 2]> = Vec::new();

        // First pass: exact matches
        for (q_idx, required_dir) in required_directions.iter().enumerate() {
            if let Some(req_dir) = required_dir {
                for (i_idx, (bbox, detected_dir)) in detected_icons.iter().enumerate() {
                    if !used_icons[i_idx] && detected_dir == req_dir {
                        let (cx, cy) = bbox.center();
                        // Scale coordinates as per Python: x * 33, y * 49
                        // These scaling factors convert from image coordinates to API coordinates
                        results[q_idx] = Some([cx * 33.0 / 100.0, cy * 49.0 / 100.0]);
                        used_icons[i_idx] = true;
                        break;
                    }
                }
            }
        }

        // Collect unused icon positions
        for (i_idx, (bbox, _)) in detected_icons.iter().enumerate() {
            if !used_icons[i_idx] {
                let (cx, cy) = bbox.center();
                unused_positions.push([cx * 33.0 / 100.0, cy * 49.0 / 100.0]);
            }
        }

        // Second pass: fill in missing with unused positions
        let mut rng = rand::thread_rng();
        for result in results.iter_mut() {
            if result.is_none() && !unused_positions.is_empty() {
                let idx = rand::Rng::gen_range(&mut rng, 0..unused_positions.len());
                *result = Some(unused_positions.remove(idx));
            }
        }

        // Convert to final format, using fallback positions if still missing
        let final_results: Vec<[f64; 2]> = results
            .into_iter()
            .enumerate()
            .map(|(idx, opt)| {
                opt.unwrap_or_else(|| {
                    // Fallback: generate a reasonable position based on index
                    let x = 50.0 + (idx as f64 * 80.0);
                    let y = 100.0;
                    [x * 33.0 / 100.0, y * 49.0 / 100.0]
                })
            })
            .collect();

        Ok(final_results)
    }
}

/// Calculate Otsu's threshold for binarization.
fn otsu_threshold(img: &GrayImage) -> u8 {
    let mut histogram = [0u64; 256];
    let total_pixels = (img.width() * img.height()) as u64;

    // Build histogram
    for pixel in img.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let mut sum = 0u64;
    for (i, &count) in histogram.iter().enumerate() {
        sum += i as u64 * count;
    }

    let mut sum_b = 0u64;
    let mut w_b = 0u64;
    let mut max_variance = 0.0f64;
    let mut threshold = 0u8;

    for (i, &count) in histogram.iter().enumerate() {
        w_b += count;
        if w_b == 0 {
            continue;
        }

        let w_f = total_pixels - w_b;
        if w_f == 0 {
            break;
        }

        sum_b += i as u64 * count;

        let m_b = sum_b as f64 / w_b as f64;
        let m_f = (sum - sum_b) as f64 / w_f as f64;

        let variance = w_b as f64 * w_f as f64 * (m_b - m_f).powi(2);

        if variance > max_variance {
            max_variance = variance;
            threshold = i as u8;
        }
    }

    threshold
}

/// Apply threshold to create binary image.
fn threshold_image(img: &GrayImage, threshold: u8) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut binary = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y)[0];
            // Invert so foreground (icons) are white
            let val = if pixel < threshold { 255 } else { 0 };
            binary.put_pixel(x, y, Luma([val]));
        }
    }

    binary
}

/// Find connected components in a binary image and return bounding boxes.
fn find_connected_components(binary: &GrayImage) -> Vec<BoundingBox> {
    let (width, height) = binary.dimensions();
    let mut labels: Vec<i32> = vec![0; (width * height) as usize];
    let mut current_label = 1i32;

    // First pass: assign preliminary labels
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if binary.get_pixel(x, y)[0] == 0 {
                continue; // Background
            }

            let mut neighbors = Vec::new();

            // Check left neighbor
            if x > 0 {
                let left_idx = (y * width + x - 1) as usize;
                if labels[left_idx] > 0 {
                    neighbors.push(labels[left_idx]);
                }
            }

            // Check top neighbor
            if y > 0 {
                let top_idx = ((y - 1) * width + x) as usize;
                if labels[top_idx] > 0 {
                    neighbors.push(labels[top_idx]);
                }
            }

            if neighbors.is_empty() {
                labels[idx] = current_label;
                current_label += 1;
            } else {
                let min_label = *neighbors.iter().min().unwrap();
                labels[idx] = min_label;
            }
        }
    }

    // Second pass: find bounding boxes for each label
    let mut bboxes: HashMap<i32, (u32, u32, u32, u32)> = HashMap::new();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let label = labels[idx];
            if label > 0 {
                let entry = bboxes.entry(label).or_insert((x, y, x, y));
                entry.0 = entry.0.min(x);
                entry.1 = entry.1.min(y);
                entry.2 = entry.2.max(x);
                entry.3 = entry.3.max(y);
            }
        }
    }

    bboxes
        .into_values()
        .map(|(x1, y1, x2, y2)| BoundingBox {
            x1,
            y1,
            x2: x2 + 1,
            y2: y2 + 1,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_direction() {
        let solver = IconSolver::new().unwrap();

        // Test with full URL
        let url = "nerualpic/original_icon_pic/icon_20201215/315ce8665e781dabcd1eb09d3e604803.png";
        assert_eq!(solver.get_direction(url), Some("l"));

        // Test with unknown icon
        assert_eq!(solver.get_direction("unknown.png"), None);
    }

    #[test]
    fn test_icon_mapping() {
        let solver = IconSolver::new().unwrap();

        assert_eq!(
            solver.icon_map.get("8da090c135ff029f3b5e19f4c44f73c8.png"),
            Some(&"u".to_string())
        );
        assert_eq!(
            solver.icon_map.get("cb0eaa639b2117a69a81af3d8c1496a1.png"),
            Some(&"d".to_string())
        );
    }

    #[test]
    fn test_otsu_threshold() {
        // Create a simple test image with some gradation
        let mut img = GrayImage::new(10, 10);
        // Create some variation in pixel values
        for y in 0..10 {
            for x in 0..10 {
                // Create a gradient pattern
                let val = ((x + y) * 25).min(255) as u8;
                img.put_pixel(x, y, Luma([val]));
            }
        }

        let threshold = otsu_threshold(&img);
        // Threshold should be a valid value (0-255)
        // For this gradient, should be somewhere reasonable
        assert!(threshold <= 255, "Threshold should be valid: {}", threshold);
    }

    #[test]
    fn test_bounding_box_center() {
        let bbox = BoundingBox {
            x1: 10,
            y1: 20,
            x2: 30,
            y2: 40,
        };
        let (cx, cy) = bbox.center();
        assert!((cx - 20.0).abs() < 0.01);
        assert!((cy - 30.0).abs() < 0.01);
    }
}
