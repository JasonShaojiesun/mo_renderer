#![cfg_attr(debug_assertions, allow(dead_code))]

pub mod application;
pub mod color;
pub mod utils;

pub use application::{App, AppError};
