pub mod definitions;
pub mod docs;
pub mod rust;
pub mod service;

// Re-export the derive macro for convenience
pub use endpoint_gen_macros::DefinitionVariant;
