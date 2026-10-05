//! Lazy cache replacement through real bundle and widget COM calls, including reentry.
use super::*;
use neocogi_com::WeakComPtr;

/// One-shot provider behavior; observers do not retain widgets or their runtime.
enum RefreshAction {
    /// Complete another widget's refresh while the first import is still reading.
    Sibling(WeakComBox<ButtonAbi>),
    /// Install a newer host revision and publish it through a sibling before returning.
    Replace {
        /// Weak observation avoids a provider-to-host reference cycle.
        runtime: WeakComPtr<UiRuntimeAbi>,
        /// Independent immutable candidate in the same original namespace.
        next: ComPtr<IAssetBundleAbi>,
        /// Concrete CPU backend used only for the authorized installation callback.
        renderer: ComPtr<IRendererAbi>,
        /// Optional distinct widget publication before returning to the outer import.
        sibling: Option<WeakComBox<ButtonAbi>>,
    },
}

#[com_object(interfaces(IAssetBundle))]
/// Immutable portable data with a controlled one-shot reentrant catalog getter.
struct ReentrantAssets {
    /// Concrete counted delegation keeps successful data immutable.
    source: CountingAssets,
    /// The test arms after installation; recursive calls take no further action.
    /// Interior mutability is required by the shared observational getter.
    action: Rc<RefCell<Option<RefreshAction>>>,
    /// Number of executed actions, independent of provider call count.
    fired: Rc<Cell<u32>>,
    /// A transient provider failure after the nested commit, not changed resource data.
    fail_after_action: Rc<Cell<bool>>,
    /// One-shot destructor reentry checks that retired owners release outside the cache.
    on_drop: Rc<RefCell<Option<WeakComBox<ButtonAbi>>>>,
    /// Concrete provider destruction count, independent of weak storage lifetime.
    drops: Rc<Cell<u32>>,
}
impl Drop for ReentrantAssets {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        let sibling = self.on_drop.borrow_mut().take();
        if let Some(mut sibling) = sibling {
            named_font(&mut sibling).unwrap();
        }
    }
}
impl IAssetBundle for ReentrantAssets {
    /// Delegates the immutable image capability after recording the lookup.
    fn image(&self) -> Result<ComPtr<IRgbaImageAbi>, NcStatus> {
        self.source.image()
    }
    /// Delegates catalog identity after recording the lookup.
    fn catalog(&self) -> Result<ComPtr<IAssetCatalogAbi>, NcStatus> {
        // Claim this call's failure before reentry so the nested getter succeeds
        // and the error is reported by the outer call after the sibling commits.
        let fail_this_read = self.fail_after_action.replace(false);
        // Take the action before calling out: the test's shared trigger cannot
        // keep its own RefCell borrowed during recursive getter execution.
        let action = self.action.borrow_mut().take();
        if let Some(action) = action {
            self.fired.set(self.fired.get() + 1);
            match action {
                RefreshAction::Sibling(mut sibling) => named_font(&mut sibling)?,
                RefreshAction::Replace { runtime, next, renderer, sibling } => {
                    let mut runtime = runtime.upgrade()?.ok_or(neocogi_com::NC_E_DROPPED)?;
                    runtime.install_assets(next.as_com_ref(), renderer.as_com_ref())?;
                    if let Some(mut sibling) = sibling {
                        named_font(&mut sibling)?;
                    }
                }
            }
        }
        if fail_this_read {
            return Err(UI_E_RENDER);
        }
        self.source.catalog()
    }
    /// Delegates bounded table metadata after recording the lookup.
    fn info(&self) -> Result<AssetInfo, NcStatus> {
        self.source.info()
    }
    /// Delegates the requested font range after recording the copy.
    fn fonts(&self, output: NcOutputWriter<'_, AssetFont>) -> Result<(), NcStatus> {
        self.source.fonts(output)
    }
    /// Delegates the requested glyph range after recording the copy.
    fn glyphs(&self, output: NcOutputWriter<'_, AssetGlyph>) -> Result<(), NcStatus> {
        self.source.glyphs(output)
    }
    /// Delegates the requested icon range after recording the copy.
    fn icons(&self, output: NcOutputWriter<'_, AssetIcon>) -> Result<(), NcStatus> {
        self.source.icons(output)
    }
    /// Delegates the requested UTF-8 name range after recording the copy.
    fn names(&self, output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        self.source.names(output)
    }
    /// Delegates scalar layout metrics after recording the lookup.
    fn metrics(&self) -> Result<AssetMetrics, NcStatus> {
        self.source.metrics()
    }
    /// Delegates window chrome policy after recording the lookup.
    fn chrome(&self) -> Result<AssetChrome, NcStatus> {
        self.source.chrome()
    }
    /// Delegates complete surface-state records after recording the copy.
    fn surface_visuals(&self, output: NcOutputWriter<'_, AssetSurfaceVisuals>) -> Result<(), NcStatus> {
        self.source.surface_visuals(output)
    }
    /// Delegates complete control-state records after recording the copy.
    fn control_visuals(&self, output: NcOutputWriter<'_, AssetControlVisuals>) -> Result<(), NcStatus> {
        self.source.control_visuals(output)
    }
    /// Delegates complete menu-state records after recording the copy.
    fn menu_visuals(&self, output: NcOutputWriter<'_, AssetMenuVisuals>) -> Result<(), NcStatus> {
        self.source.menu_visuals(output)
    }
    /// Delegates complete chrome-state records after recording the copy.
    fn chrome_visuals(&self, output: NcOutputWriter<'_, AssetChromeVisuals>) -> Result<(), NcStatus> {
        self.source.chrome_visuals(output)
    }
}
/// Two mounted production buttons from a single provider cache and a CPU renderer.
struct Fixture {
    /// Authoritative retained host; installations use its normal mutable COM entry.
    runtime: ComPtr<UiRuntimeAbi>,
    /// Retained base namespace from which every replacement is derived.
    base: ComPtr<IAssetBundleAbi>,
    /// Independently allocated root observers; windows own their concrete values.
    first: WeakComBox<ButtonAbi>,
    /// Sibling shares only the provider-local asset cache with the first button.
    sibling: WeakComBox<ButtonAbi>,
    /// Retains both mounted widget owners for every assertion and frame submission.
    windows: [ComPtr<IWindowAbi>; 2],
    /// CPU backend consumes the actual runtime's scoped render stream.
    renderer: ComPtr<IRendererAbi>,
    /// Independently owned observation of submitted frames.
    probe: Rc<RenderProbe>,
}
impl Fixture {
    /// Creates ordinary production allocations and commits their initial layout.
    fn new() -> Self {
        let bundle = crate::SkinBundle::from_atlas(crate::test_support::test_atlas());
        let base = crate::test_support::assets::export_bundle(&bundle).unwrap();
        let runtime = crate::test_support::runtime_from_assets(base.as_com_ref()).unwrap();
        let factory = nc_ui_nodes::UiFactory::bind(runtime.as_com_ref()).unwrap();
        let binding = neocogi_com::events::EventBinding::Unobserved;
        let first = factory
            .create_button(NcCopyFromBufferOffset::new(0, b"first"), neocogi_com::NcValueRef::new(&binding).unwrap())
            .unwrap();
        let sibling = factory
            .create_button(NcCopyFromBufferOffset::new(0, b"sibling"), neocogi_com::NcValueRef::new(&binding).unwrap())
            .unwrap();
        let observations = (first.downgrade(), sibling.downgrade());
        let windows = [
            mount(&runtime, first.query().unwrap()).unwrap(),
            mount(&runtime, sibling.query().unwrap()).unwrap(),
        ];
        let (renderer, probe) = renderer();
        runtime.update(SIZE).unwrap();
        Self {
            runtime,
            base,
            first: observations.0,
            sibling: observations.1,
            windows,
            renderer,
            probe,
        }
    }

    /// Produces a visibly distinct padding revision without changing original names or identity.
    fn revision(&self, padding: i32) -> ComPtr<IAssetBundleAbi> {
        let mut data = neocogi_ui::assets::AssetData::read(self.base.as_com_ref()).unwrap();
        data.tables.metrics.padding = padding;
        data.into_revision(self.base.as_com_ref()).unwrap()
    }

    /// Installs through the real transaction, then leaves the node cache lazily outdated.
    fn install(&mut self, source: &ComPtr<IAssetBundleAbi>) {
        self.runtime.install_assets(source.as_com_ref(), self.renderer.as_com_ref()).unwrap();
    }

    /// Normal update and render must reuse the published import and produce valid geometry.
    fn frame(&self) {
        self.runtime.update(SIZE).unwrap();
        self.runtime.render(SIZE, self.renderer.as_com_ref()).unwrap();
        assert!(self.probe.calls.get() > 0);
        for window in &self.windows {
            assert!(window.snapshot().is_ok());
        }
    }
}

/// Performs the exact named-font call used in the abort reproduction.
fn named_font(widget: &mut WeakComBox<ButtonAbi>) -> Result<(), NcStatus> {
    widget.set_named_font(NcCopyFromBufferOffset::new(0, b"body"))
}

/// Provider and independent observations used to arm and check actual COM calls.
struct Provider {
    /// Shared immutable bundle projection installed by the production runtime.
    source: ComPtr<IAssetBundleAbi>,
    /// One-shot action taken before outward reentry.
    action: Rc<RefCell<Option<RefreshAction>>>,
    /// Portable getter count, including the original immutable delegation.
    calls: Rc<Cell<u32>>,
    /// Number of nested actions, regardless of import success.
    fired: Rc<Cell<u32>>,
    /// Requests one error after the nested action commits.
    fail: Rc<Cell<bool>>,
    /// Destructor action armed only while a different live sibling remains.
    on_drop: Rc<RefCell<Option<WeakComBox<ButtonAbi>>>>,
    /// Number of destroyed concrete provider values.
    drops: Rc<Cell<u32>>,
}
impl Provider {
    /// Builds a counted provider with all reentry disabled until the test arms it.
    fn new(source: ComPtr<IAssetBundleAbi>) -> Self {
        let calls = Rc::new(Cell::new(0));
        let action = Rc::new(RefCell::new(None));
        let fired = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(false));
        let on_drop = Rc::new(RefCell::new(None));
        let drops = Rc::new(Cell::new(0));
        let source = ReentrantAssets::from_value(ReentrantAssets {
            source: CountingAssets { source, calls: calls.clone() },
            action: action.clone(),
            fired: fired.clone(),
            fail_after_action: fail.clone(),
            on_drop: on_drop.clone(),
            drops: drops.clone(),
        })
        .query()
        .unwrap();
        Self {
            source,
            action,
            calls,
            fired,
            fail,
            on_drop,
            drops,
        }
    }
}

/// The reproduced sibling import succeeds immediately, and stable phases do no extra import.
#[test]
fn sibling_refresh_completes_during_provider_getter_and_stabilizes() {
    let mut fixture = Fixture::new();
    let Provider { source, action, calls, fired, .. } = Provider::new(fixture.revision(9));
    fixture.install(&source);
    calls.set(0);
    *action.borrow_mut() = Some(RefreshAction::Sibling(fixture.sibling.clone()));
    named_font(&mut fixture.first).unwrap();
    assert_eq!(fired.get(), 1);
    assert!(calls.get() > 0);
    calls.set(0);
    named_font(&mut fixture.sibling).unwrap();
    named_font(&mut fixture.first).unwrap();
    fixture.frame();
    assert_eq!(calls.get(), 0, "same-source sibling publication must remain the stable cache");
}

/// An outer getter error preserves the sibling's already committed matching revision.
#[test]
fn error_after_sibling_publication_preserves_committed_cache() {
    let mut fixture = Fixture::new();
    let Provider { source, action, calls, fired, fail, .. } = Provider::new(fixture.revision(11));
    fixture.install(&source);
    *action.borrow_mut() = Some(RefreshAction::Sibling(fixture.sibling.clone()));
    fail.set(true);
    assert_eq!(named_font(&mut fixture.first), Err(UI_E_RENDER));
    assert_eq!(fired.get(), 1);
    calls.set(0);
    named_font(&mut fixture.first).unwrap();
    named_font(&mut fixture.sibling).unwrap();
    fixture.frame();
    assert_eq!(calls.get(), 0, "outer failure must not roll back the nested commit");
}

/// A stale outer candidate cannot replace a newer host/sibling publication; retry is bounded.
#[test]
fn third_revision_during_import_is_not_overwritten_by_stale_candidate() {
    let mut fixture = Fixture::new();
    let next_calls = Rc::new(Cell::new(0));
    let next = CountingAssets::from_value(CountingAssets {
        source: fixture.revision(13),
        calls: next_calls.clone(),
    })
    .query::<IAssetBundleAbi>()
    .unwrap();
    let Provider {
        source, action, calls: old_calls, fired, ..
    } = Provider::new(fixture.revision(7));
    fixture.install(&source);
    *action.borrow_mut() = Some(RefreshAction::Replace {
        runtime: fixture.runtime.downgrade().unwrap(),
        next: next.clone(),
        renderer: fixture.renderer.clone(),
        sibling: Some(fixture.sibling.clone()),
    });
    assert_eq!(named_font(&mut fixture.first), Err(NC_E_BUSY));
    assert_eq!(fired.get(), 1);
    assert_eq!(fixture.runtime.assets().unwrap().as_raw().object, next.as_raw().object);
    next_calls.set(0);
    old_calls.set(0);
    named_font(&mut fixture.first).unwrap();
    named_font(&mut fixture.sibling).unwrap();
    fixture.frame();
    assert_eq!(next_calls.get(), 0, "newer sibling publication must survive the stale outer candidate");
    assert_eq!(old_calls.get(), 0, "superseded provider must not be revisited");
}

/// Ordinary replacement imports once, then all sibling setters and frames reuse it.
#[test]
fn revision_refresh_is_shared_and_stable_without_reentry() {
    let mut fixture = Fixture::new();
    let Provider { source, calls, .. } = Provider::new(fixture.revision(10));
    fixture.install(&source);
    calls.set(0);
    named_font(&mut fixture.first).unwrap();
    assert!(calls.get() > 0);
    calls.set(0);
    named_font(&mut fixture.sibling).unwrap();
    fixture.frame();
    fixture.frame();
    assert_eq!(calls.get(), 0);
}

/// A candidate getter failure without nested publication leaves import pending for retry.
#[test]
fn failed_import_without_reentry_does_not_mark_candidate_current() {
    let mut fixture = Fixture::new();
    let Provider { source, calls, fail, .. } = Provider::new(fixture.revision(12));
    fixture.install(&source);
    fail.set(true);
    assert_eq!(named_font(&mut fixture.first), Err(UI_E_RENDER));
    calls.set(0);
    named_font(&mut fixture.sibling).unwrap();
    assert!(calls.get() > 0, "failed import must leave the previous cache installed");
    calls.set(0);
    named_font(&mut fixture.first).unwrap();
    fixture.frame();
    assert_eq!(calls.get(), 0);
}

/// If only the host changes during import, retry imports its newer source once.
#[test]
fn third_revision_without_nested_publication_imports_on_retry() {
    let mut fixture = Fixture::new();
    let next_calls = Rc::new(Cell::new(0));
    let next = CountingAssets::from_value(CountingAssets {
        source: fixture.revision(14),
        calls: next_calls.clone(),
    })
    .query::<IAssetBundleAbi>()
    .unwrap();
    let Provider { source, action, .. } = Provider::new(fixture.revision(8));
    fixture.install(&source);
    *action.borrow_mut() = Some(RefreshAction::Replace {
        runtime: fixture.runtime.downgrade().unwrap(),
        next,
        renderer: fixture.renderer.clone(),
        sibling: None,
    });
    assert_eq!(named_font(&mut fixture.first), Err(NC_E_BUSY));
    next_calls.set(0);
    named_font(&mut fixture.first).unwrap();
    assert!(next_calls.get() > 0);
    next_calls.set(0);
    named_font(&mut fixture.sibling).unwrap();
    fixture.frame();
    assert_eq!(next_calls.get(), 0);
}

/// Release of the last retired provider may call a sibling after the new cache commits.
#[test]
fn retired_provider_destructor_reenters_without_live_cache_borrow() {
    let mut fixture = Fixture::new();
    let Provider { source, on_drop, drops, .. } = Provider::new(fixture.revision(6));
    fixture.install(&source);
    named_font(&mut fixture.first).unwrap();
    let next = fixture.revision(15);
    fixture.install(&next);
    *on_drop.borrow_mut() = Some(fixture.sibling.clone());
    drop(source); // The outdated node cache now retains the last old provider reference.
    assert_eq!(drops.get(), 0);
    named_font(&mut fixture.first).unwrap();
    assert_eq!(drops.get(), 1);
    fixture.frame();
}
