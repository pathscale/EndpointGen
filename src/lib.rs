pub mod asyncapi;
pub mod definitions;
pub mod docs;
pub mod error_codes;
pub mod openapi;
pub mod rust;
pub mod service;
pub mod spec_common;

// Re-export the derive macro for convenience
pub use endpoint_gen_macros::DefinitionVariant;
