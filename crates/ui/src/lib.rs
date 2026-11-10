//! Static NeoCogi UI implementation with canonical contract reexports and one service root.
//!
//! Empty defaults retain the portable client surface. Features select retained UI/nodes,
//! graphics construction, appearance, JSON authoring and documents independently. The thin
//! dylib links this exact implementation; contracts and generated schemas have one owner.
#![deny(missing_docs)]
extern crate self as neocogi_ui;
extern crate alloc;

pub use neocogi_ui_contracts::*;
/// Canonical definitions, typed clients, portable values and build-support ownership.
pub use neocogi_ui_contracts as contracts;

/// Strict theme authoring and compilation through explicitly supplied graphics capabilities.
#[cfg(feature = "authoring")]
pub mod authoring;
/// Typed authored UI documents, reconstruction and replace-on-success reload.
#[cfg(feature = "document")]
pub mod document;
/// Graphics construction and composition; all ABI/value definitions remain canonical.
#[cfg(any(feature = "gfx", feature = "publisher"))]
pub mod gfx;
/// Built-in controls and containers using the same public capabilities as extensions.
#[cfg(feature = "ui")]
pub mod nodes;
/// Retained forest, routing, layout, windows and frame commit implementation.
#[cfg(feature = "ui")]
pub mod runtime;
/// Appearance construction with optional external theme loading.
#[cfg(feature = "skin")]
pub mod skin;
/// Presentation and geometry algorithms shared by runtime and concrete nodes.
#[cfg(feature = "ui")]
pub mod support;

use neocogi_com::{ComPtr, NcStatus};
use neocogi_ui_contracts::abi::library::{UiLibraryFactory, UiLibraryFactoryAbi};
use neocogi_ui_contracts::abi::nodes::NodesPluginFactoryAbi;

#[neocogi_com::com_object(interfaces(neocogi_ui_contracts::abi::library::UiLibraryFactory))]
/// Stateless root; getters create independently owned typed factories, never retain a UI tree.
struct LibraryFactory;
impl UiLibraryFactory for LibraryFactory {
    /// Returns an independently owned retained-runtime service when selected.
    fn ui(&self) -> Result<ComPtr<UiPluginFactoryAbi>, NcStatus> {
        #[cfg(feature = "ui")]
        {
            runtime::factory()
        }
        #[cfg(not(feature = "ui"))]
        {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
    /// Returns the node binder without retaining a runtime or tree in this root.
    fn nodes(&self) -> Result<ComPtr<NodesPluginFactoryAbi>, NcStatus> {
        #[cfg(feature = "ui")]
        {
            nodes::factory()
        }
        #[cfg(not(feature = "ui"))]
        {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
    /// Returns a fresh graphics factory; direct publisher-only builds omit this service.
    fn gfx(&self) -> Result<ComPtr<GfxPluginFactoryAbi>, NcStatus> {
        #[cfg(feature = "gfx")]
        {
            gfx::factory()
        }
        #[cfg(not(feature = "gfx"))]
        {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
    /// Returns a fresh appearance factory whose explicit arguments own downstream capabilities.
    fn skin(&self) -> Result<ComPtr<SkinPluginFactoryAbi>, NcStatus> {
        #[cfg(feature = "skin")]
        {
            skin::factory()
        }
        #[cfg(not(feature = "skin"))]
        {
            Err(neocogi_com::NC_E_NOT_FOUND)
        }
    }
}

/// One bootstrap identity supplies every enabled service, without implementation plugin dependencies.
#[neocogi_com::plugin(name = "neocogi.ui")]
fn acquire_root(_runtime: neocogi_com::Runtime) -> Result<ComPtr<UiLibraryFactoryAbi>, NcStatus> {
    LibraryFactory::from_value(LibraryFactory).query()
}

#[cfg(test)]
extern crate neocogi_com_host as _;

#[cfg(all(test, feature = "test-support"))]
mod test_allocator;
