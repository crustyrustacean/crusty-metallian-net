// src/routes/api.rs

// module declarations
pub mod health_check;
pub mod rotation;
pub mod stats;

// re-exports
pub use health_check::*;
pub use rotation::*;
pub use stats::*;
