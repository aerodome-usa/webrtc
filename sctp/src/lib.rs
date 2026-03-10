// This crate is vendored and planned for deprecation so just ignore any warnings for now
#![allow(clippy::all)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)]

pub mod association;
pub mod chunk;
mod error;
pub mod error_cause;
pub mod packet;
pub mod param;
pub(crate) mod queue;
pub mod stream;
pub(crate) mod timer;
pub(crate) mod util;

pub use error::Error;

#[cfg(test)]
mod fuzz_artifact_test;
