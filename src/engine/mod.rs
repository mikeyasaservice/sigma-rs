//! High-performance Sigma engine implementation with SIMD optimizations

pub mod simd_engine;
pub mod zero_copy;
pub mod standard;
pub mod optimized_batch_processor;
// pub mod hybrid;  // Temporarily disabled while migrating to new compiler

// Re-export all engine types
pub use simd_engine::{SimdSigmaEngine, SimdEngineConfig, EngineMetrics};
pub use zero_copy::{ZeroCopyEvent, EventBatch, BufferPool};
pub use standard::SigmaEngine;
// pub use hybrid::{HybridSigmaEngine, HybridEngineConfig};
