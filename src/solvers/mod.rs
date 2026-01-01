//! Captcha solvers for different Geetest risk types.

pub mod gobang;
pub mod slide;

#[cfg(feature = "icon")]
pub mod icon;

pub use gobang::GobangSolver;
pub use slide::SlideSolver;

#[cfg(feature = "icon")]
pub use icon::{BoundingBox, IconSolver};
