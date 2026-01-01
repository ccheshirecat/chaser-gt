//! # chaser-gt
//!
//! A high-performance Rust implementation of a Geetest v4 captcha solver.
//!
//! ## Features
//!
//! - **Automatic Deobfuscation**: Constants are automatically updated when Geetest
//!   releases new versions - no manual intervention required.
//! - **Multiple Captcha Types**: Supports Slide, Gobang, Icon, and AI/Invisible captchas.
//! - **TLS Fingerprinting**: Uses `rquest` for Chrome-like TLS fingerprinting.
//! - **Proxy Support**: HTTP and SOCKS5 proxy support with authentication.
//! - **Async/Await**: Built on Tokio for efficient concurrent captcha solving.
//!
//! ## Quick Start
//!
//! ```ignore
//! use chaser_gt::{Geeked, RiskType};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create a solver for slide captcha
//!     let solver = Geeked::builder("your_captcha_id", RiskType::Slide)
//!         .build()
//!         .await?;
//!
//!     // Solve the captcha
//!     let result = solver.solve().await?;
//!
//!     println!("captcha_id: {}", result.captcha_id);
//!     println!("lot_number: {}", result.lot_number);
//!     println!("pass_token: {}", result.pass_token);
//!     println!("gen_time: {}", result.gen_time);
//!     println!("captcha_output: {}", result.captcha_output);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## With Proxy
//!
//! ```ignore
//! use chaser_gt::{Geeked, RiskType};
//!
//! let solver = Geeked::builder("captcha_id", RiskType::Slide)
//!     .proxy("http://user:pass@proxy.example.com:8080")
//!     .build()
//!     .await?;
//! ```
//!
//! ## Supported Captcha Types
//!
//! - `RiskType::Slide` - Slide puzzle captcha
//! - `RiskType::Gobang` - Five-in-a-row puzzle
//! - `RiskType::Icon` - Icon selection captcha (requires `icon` feature)
//! - `RiskType::Ai` - AI/Invisible captcha
//!
//! ## Automatic Constant Updates
//!
//! Unlike other implementations that require manual updates when Geetest changes
//! their obfuscation, this library automatically:
//!
//! 1. Checks for new Geetest script versions
//! 2. Deobfuscates the script to extract constants
//! 3. Caches the constants for future use
//!
//! This means the library stays functional even when Geetest updates their anti-bot measures.

// Allow missing docs for internal types for now
#![allow(missing_docs)]

pub mod client;
pub mod crypto;
pub mod deobfuscate;
pub mod error;
pub mod models;
pub mod sign;
pub mod solvers;

// Re-exports for convenience
pub use client::{Geeked, GeekedBuilder};
pub use error::{GeekedError, Result};
pub use models::{RiskType, SecCode};

/// Initialize the library.
///
/// This is optional but can be called to pre-fetch constants before solving.
pub async fn init() -> Result<()> {
    let deobfuscator = deobfuscate::Deobfuscator::new();
    let _ = deobfuscator.get_constants().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_type_display() {
        assert_eq!(RiskType::Slide.as_str(), "slide");
        assert_eq!(RiskType::Gobang.as_str(), "gobang");
        assert_eq!(RiskType::Icon.as_str(), "icon");
        assert_eq!(RiskType::Ai.as_str(), "ai");
    }
}
