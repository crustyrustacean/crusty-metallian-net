// src/routes/api.rs

// module declarations
pub mod galleries;
pub mod health_check;
pub mod photos;
pub mod rotation;
pub mod stats;

// re-exports
pub use galleries::*;
pub use health_check::*;
pub use photos::*;
pub use rotation::*;
pub use stats::*;
