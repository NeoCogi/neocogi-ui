//! Ownership for focused tests of concrete built-in algorithms, without an alternate retained node.

use crate::nodes::widget_object::WidgetRuntime;
use crate::nodes::WidgetOption;
use neocogi_com::ComPtr;
use neocogi_ui::abi::core::UiRuntimeAbi;
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
    /// Test dispatcher owns scope authority; the concrete widget receives only its host.
    driver: neocogi_ui::runtime::testing::OriginDriver,
}
impl<W> Algorithm<W> {
    /// Transfers the concrete value into its real COM allocation while preserving
    /// the fixture host through all local guards and final widget destruction.
    pub(crate) fn map<T>(self, wrap: impl FnOnce(W) -> T) -> Algorithm<T> {
        Algorithm {
            value: wrap(self.value),
            _owner: self._owner,
            driver: self.driver,
        }
    }
    /// Runs a direct call under the identity selected by this test's owning dispatcher.
    pub(crate) fn with_origin<R>(&mut self, origin: neocogi_ui::events::EventOrigin, call: impl FnOnce(&mut W) -> R) -> R {
        self.driver.with_origin(origin, || call(&mut self.value)).unwrap()
    }
    /// Supplies the actual host and resource imports to the normal concrete constructor.
    pub(crate) fn new(options: WidgetOption, construct: impl FnOnce(WidgetRuntime) -> W) -> Self {
        let source = super::assets::export_bundle(&crate::nodes::SkinBundle::from_atlas(super::test_atlas())).unwrap();
        let gfx = neocogi_ui::support::test_support::gfx_factory(crate::nodes::test_support::host_runtime());
        let (owner, driver) = neocogi_ui::runtime::testing::create_runtime(source.as_com_ref(), gfx.as_com_ref()).unwrap();
        let host = owner.construction_host().unwrap().host().unwrap();
        let runtime = WidgetRuntime::new(host, options).unwrap();
        Self {
            value: construct(runtime),
            _owner: owner,
            driver,
        }
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

impl<W: neocogi_com::ComObject> Algorithm<neocogi_com::LocalBox<W>> {
    /// Borrows the actual concrete object inside a synchronous owner-selected source scope.
    pub(crate) fn dispatch<R>(&mut self, call: impl FnOnce(&mut W) -> R) -> R {
        let origin = self
            .value
            .query::<neocogi_ui::abi::core::IWidgetAbi>()
            .unwrap()
            .into_unknown()
            .into_thread_local();
        self.with_origin(neocogi_ui::events::EventOrigin::Widget(origin), |value| {
            call(&mut value.try_borrow_mut().unwrap())
        })
    }
}
