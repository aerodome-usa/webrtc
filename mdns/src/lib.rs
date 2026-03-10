// This crate is vendored and planned for deprecation so just ignore any warnings for now
#![allow(clippy::all)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)]

pub mod config;
pub mod conn;
mod error;
pub mod message;

pub use error::Error;
