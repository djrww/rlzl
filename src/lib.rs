//! # welzl_macro
//!
//! 在 Rust 宏中實現 Welzl 最小包覆圓算法，並在編譯期把它推到極限。
#![allow(clippy::needless_return)]

pub mod adaptive;
pub mod delaunay;
pub mod exact;
pub mod interval;
pub mod kernel;
pub mod macros;
pub mod predicates;
pub mod boolops;
pub mod segdist;
pub mod sphere;
pub mod welzl;

pub use adaptive::Solution;
pub use exact::{Circle, Pt, EMPTY};
