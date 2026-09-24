use std::marker::PhantomData;
use std::time::Duration;

pub use glam;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type EngineResult<T> = Result<T, EngineError>;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("asset not found: {0}")]
    AssetNotFound(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("GPU out of memory")]
    OutOfMemory,
}

mod diagnostics;
mod handle;
mod time;
mod transform;
pub use diagnostics::FrameDiagnostics;
pub use handle::{AssetId, Handle};
pub use time::FixedTime;
pub use transform::TransformState;
#[cfg(test)]
mod tests;
