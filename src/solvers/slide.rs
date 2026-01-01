//! Slide puzzle solver using edge detection and template matching.
//!
//! This solver finds the correct X position for a puzzle piece by:
//! 1. Converting images to grayscale
//! 2. Applying Canny edge detection
//! 3. Using template matching to find the best position

use crate::error::{GeekedError, Result};
use image::{DynamicImage, GrayImage, Luma};
use imageproc::template_matching::{find_extremes, match_template, MatchTemplateMethod};

/// Solver for slide captcha puzzles.
pub struct SlideSolver {
    puzzle_piece: DynamicImage,
    background: DynamicImage,
}

impl SlideSolver {
    /// Create a new slide solver from image bytes.
    ///
    /// # Arguments
    /// * `puzzle_piece` - Bytes of the puzzle piece image
    /// * `background` - Bytes of the background image
    pub fn from_bytes(puzzle_piece: &[u8], background: &[u8]) -> Result<Self> {
        let puzzle_piece = image::load_from_memory(puzzle_piece)
            .map_err(|e| GeekedError::ImageProcessing(format!("Failed to load puzzle piece: {}", e)))?;
        let background = image::load_from_memory(background)
            .map_err(|e| GeekedError::ImageProcessing(format!("Failed to load background: {}", e)))?;

        Ok(Self {
            puzzle_piece,
            background,
        })
    }

    /// Create a new slide solver from DynamicImage instances.
    pub fn new(puzzle_piece: DynamicImage, background: DynamicImage) -> Self {
        Self {
            puzzle_piece,
            background,
        }
    }

    /// Find the X position where the puzzle piece should be placed.
    ///
    /// # Returns
    /// The X coordinate (left edge) of the puzzle piece position.
    pub fn find_position(&self) -> f64 {
        // Convert to grayscale
        let piece_gray = self.puzzle_piece.to_luma8();
        let bg_gray = self.background.to_luma8();

        // Apply Canny edge detection
        let piece_edges = canny_edge_detection(&piece_gray, 100.0, 200.0);
        let bg_edges = canny_edge_detection(&bg_gray, 100.0, 200.0);

        // Template matching
        let result = match_template(&bg_edges, &piece_edges, MatchTemplateMethod::CrossCorrelationNormalized);
        let extremes = find_extremes(&result);

        // Get the position of maximum correlation
        let (max_x, _max_y) = extremes.max_value_location;
        let piece_width = self.puzzle_piece.width() as f64;

        // Calculate center X and subtract offset
        // The -41 offset accounts for the transparent padding on the puzzle piece
        let center_x = max_x as f64 + piece_width / 2.0;
        center_x - 41.0
    }
}

/// Apply Canny edge detection to a grayscale image.
///
/// This is a simplified implementation that:
/// 1. Applies Gaussian blur
/// 2. Computes gradients using Sobel operator
/// 3. Applies non-maximum suppression
/// 4. Uses double thresholding with hysteresis
fn canny_edge_detection(image: &GrayImage, low_threshold: f64, high_threshold: f64) -> GrayImage {
    let (width, height) = image.dimensions();

    // Apply Gaussian blur first (3x3 kernel)
    let blurred = gaussian_blur(image);

    // Compute gradients using Sobel operator
    let (gx, gy) = sobel_gradients(&blurred);

    // Compute magnitude and direction
    let mut magnitude = vec![vec![0.0f64; height as usize]; width as usize];
    let mut direction = vec![vec![0.0f64; height as usize]; width as usize];

    for x in 0..width as usize {
        for y in 0..height as usize {
            let gx_val = gx[x][y];
            let gy_val = gy[x][y];
            magnitude[x][y] = (gx_val * gx_val + gy_val * gy_val).sqrt();
            direction[x][y] = gy_val.atan2(gx_val);
        }
    }

    // Non-maximum suppression
    let suppressed = non_maximum_suppression(&magnitude, &direction, width as usize, height as usize);

    // Double thresholding and hysteresis
    let result = double_threshold_hysteresis(&suppressed, low_threshold, high_threshold, width as usize, height as usize);

    // Convert to GrayImage
    let mut output = GrayImage::new(width, height);
    for x in 0..width {
        for y in 0..height {
            let val = if result[x as usize][y as usize] { 255 } else { 0 };
            output.put_pixel(x, y, Luma([val]));
        }
    }

    output
}

/// Apply Gaussian blur (3x3 kernel).
fn gaussian_blur(image: &GrayImage) -> GrayImage {
    let kernel = [
        [1.0 / 16.0, 2.0 / 16.0, 1.0 / 16.0],
        [2.0 / 16.0, 4.0 / 16.0, 2.0 / 16.0],
        [1.0 / 16.0, 2.0 / 16.0, 1.0 / 16.0],
    ];

    let (width, height) = image.dimensions();
    let mut output = GrayImage::new(width, height);

    for x in 1..width - 1 {
        for y in 1..height - 1 {
            let mut sum = 0.0;
            for kx in 0..3 {
                for ky in 0..3 {
                    let px = (x as i32 + kx as i32 - 1) as u32;
                    let py = (y as i32 + ky as i32 - 1) as u32;
                    sum += image.get_pixel(px, py)[0] as f64 * kernel[kx][ky];
                }
            }
            output.put_pixel(x, y, Luma([sum.clamp(0.0, 255.0) as u8]));
        }
    }

    output
}

/// Compute Sobel gradients.
fn sobel_gradients(image: &GrayImage) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let (width, height) = image.dimensions();
    let mut gx = vec![vec![0.0f64; height as usize]; width as usize];
    let mut gy = vec![vec![0.0f64; height as usize]; width as usize];

    let sobel_x = [[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]];
    let sobel_y = [[-1.0, -2.0, -1.0], [0.0, 0.0, 0.0], [1.0, 2.0, 1.0]];

    for x in 1..width as usize - 1 {
        for y in 1..height as usize - 1 {
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;

            for kx in 0..3 {
                for ky in 0..3 {
                    let px = (x as i32 + kx as i32 - 1) as u32;
                    let py = (y as i32 + ky as i32 - 1) as u32;
                    let val = image.get_pixel(px, py)[0] as f64;
                    sum_x += val * sobel_x[kx][ky];
                    sum_y += val * sobel_y[kx][ky];
                }
            }

            gx[x][y] = sum_x;
            gy[x][y] = sum_y;
        }
    }

    (gx, gy)
}

/// Non-maximum suppression.
fn non_maximum_suppression(
    magnitude: &[Vec<f64>],
    direction: &[Vec<f64>],
    width: usize,
    height: usize,
) -> Vec<Vec<f64>> {
    let mut suppressed = vec![vec![0.0f64; height]; width];

    for x in 1..width - 1 {
        for y in 1..height - 1 {
            let angle = direction[x][y].to_degrees();
            let angle = if angle < 0.0 { angle + 180.0 } else { angle };

            let (neighbor1, neighbor2) = if (0.0..22.5).contains(&angle) || (157.5..180.0).contains(&angle) {
                // Horizontal
                (magnitude[x + 1][y], magnitude[x - 1][y])
            } else if (22.5..67.5).contains(&angle) {
                // Diagonal (/)
                (magnitude[x + 1][y - 1], magnitude[x - 1][y + 1])
            } else if (67.5..112.5).contains(&angle) {
                // Vertical
                (magnitude[x][y + 1], magnitude[x][y - 1])
            } else {
                // Diagonal (\)
                (magnitude[x - 1][y - 1], magnitude[x + 1][y + 1])
            };

            if magnitude[x][y] >= neighbor1 && magnitude[x][y] >= neighbor2 {
                suppressed[x][y] = magnitude[x][y];
            }
        }
    }

    suppressed
}

/// Double thresholding with hysteresis.
fn double_threshold_hysteresis(
    image: &[Vec<f64>],
    low: f64,
    high: f64,
    width: usize,
    height: usize,
) -> Vec<Vec<bool>> {
    let mut result = vec![vec![false; height]; width];
    let mut strong = vec![vec![false; height]; width];

    // Mark strong and weak edges
    for x in 0..width {
        for y in 0..height {
            if image[x][y] >= high {
                strong[x][y] = true;
                result[x][y] = true;
            } else if image[x][y] >= low {
                // Weak edge - will be connected if adjacent to strong
            }
        }
    }

    // Connect weak edges to strong edges (simplified hysteresis)
    for x in 1..width - 1 {
        for y in 1..height - 1 {
            if image[x][y] >= low && !result[x][y] {
                // Check if adjacent to a strong edge
                let has_strong_neighbor = strong[x - 1][y - 1]
                    || strong[x][y - 1]
                    || strong[x + 1][y - 1]
                    || strong[x - 1][y]
                    || strong[x + 1][y]
                    || strong[x - 1][y + 1]
                    || strong[x][y + 1]
                    || strong[x + 1][y + 1];

                if has_strong_neighbor {
                    result[x][y] = true;
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_solver_creation() {
        // Create a simple test image
        let piece = DynamicImage::new_rgb8(50, 50);
        let bg = DynamicImage::new_rgb8(300, 200);

        let solver = SlideSolver::new(piece, bg);
        let position = solver.find_position();

        // Should return some position
        assert!(position >= -50.0 && position <= 300.0);
    }
}
