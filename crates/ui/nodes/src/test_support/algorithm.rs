//! Ownership for focused tests of concrete built-in algorithms, without an alternate retained node.

use crate::widget_object::WidgetRuntime;
use crate::WidgetOption;
use neocogi_com::ComPtr;
use neocogi_ui_core_contracts::UiRuntimeAbi;
use std::ops::{Deref, DerefMut};

/// Keeps a real host alive beside the production widget value under direct algorithm test.
///
/// This fixture never mounts, routes, lays out children, or renders a frame. Tests needing those
/// operations use `Headless` and the public UI factory instead. The value has exactly its deployed
/// representation; its owner lives here rather than in a conditional field on `WidgetRuntime`.
pub(crate) struct Algorithm<W> {
    /// Ordinary concrete semantic value; mutation uses exclusive Rust borrows.
    value: W,
    /// Dropped after the value so its host remains valid through semantic teardown.
    _owner: ComPtr<UiRuntimeAbi>,
}
impl<W> Algorithm<W> {
    /// Supplies the actual host and resource imports to the normal concrete constructor.
    pub(crate) fn new(options: WidgetOption, construct: impl FnOnce(WidgetRuntime) -> W) -> Self {
        let owner = super::create_runtime(super::test_atlas());
        let host = owner.construction_host().unwrap().host().unwrap();
        let runtime = WidgetRuntime::new(host, options).unwrap();
        Self { value: construct(runtime), _owner: owner }
    }
}
impl<W> Deref for Algorithm<W> {
    /// Concrete production type remains statically known to every algorithm assertion.
    type Target = W;
    /// Exposes the unchanged production algorithm for measurement and semantic assertions.
    fn deref(&self) -> &W {
        &self.value
    }
}
impl<W> DerefMut for Algorithm<W> {
    /// Grants ordinary exclusive mutation without introducing cells or weak widget proxies.
    fn deref_mut(&mut self) -> &mut W {
        &mut self.value
    }
}
