//! Shared theme authoring policy for tools, offline exporters, and the Skin plugin.
//!
//! The document compiler consumes typed GFX services. It owns no UI, renderer,
//! plugin registration, image decoder, or atlas packer. The optional `builder`
//! feature is only an adapter for offline callers that instantiate GFX locally.
#![deny(missing_docs)]

mod encoding;
mod role_keys;

/// Versioned theme decoding and compilation through supplied GFX capabilities.
pub mod theme;
