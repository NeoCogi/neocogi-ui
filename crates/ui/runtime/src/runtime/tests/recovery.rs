//! Actual foreign COM failures cannot strand runtime-owned invalidation.
use neocogi_com::{com_object, ComPtr, ComRef, NcCopyFromBufferOffset, NcStatus, NcValueRef};
use neocogi_ui::com::widget::WidgetState;
use neocogi_ui::abi::core::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug)]
enum Failure {
    /// Reject before update commits any semantic mutation.
    Update,
    /// Fail an observational policy query after a no-change update.
    Properties,
}

#[com_object(interfaces(IWidget, ILeafLayout), ownership = exclusive)]
/// Independent provider rejects calls before commit, owing no mutation notification.
struct Provider {
    /// Standard lifetime and sticky geometry protocol.
    state: WidgetState,
    /// One-shot rejection configured by the test outside semantic mutations.
    fail_update: Rc<Cell<bool>>,
    /// One-shot observation failure, independent of update behavior.
    fail_properties: Rc<Cell<bool>>,
    /// Counts actual COM collection calls for runtime recovery.
    collections: Rc<Cell<usize>>,
}

impl IWidget for Provider {
    /// Returns standard owner authentication.
    fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
        self.state.host().try_add_ref()
    }
    /// Uses standard mount-once admission.
    fn mount(&mut self) -> Result<(), NcStatus> {
        self.state.mount()
    }
    /// Rejects one observation before returning validated copied policy.
    fn properties(&self) -> Result<WidgetProperties, NcStatus> {
        self.state.readable()?;
        if self.fail_properties.replace(false) {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        Ok(WidgetProperties {
            options: self.state.requested_flags()?,
            keyboard: 0,
            frame: WidgetFrame::Generic,
        })
    }
    /// Reads authored options.
    fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
        self.state.requested_flags()
    }
    /// Delegates authorized option replacement.
    fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.replace_flags(flags)
    }
    /// Delegates authorized option addition.
    fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.set_flags(flags)
    }
    /// Delegates authorized option removal.
    fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
        self.state.clear_flags(flags)
    }
    /// Validates input and rejects before committing any semantic state.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
        self.state.writable()?;
        state.validate()?;
        decode_widget_event(event, text)?;
        // Rejection precedes every semantic mutation, so no sticky mutation or host
        // notification is owed under the provider's documented pre-commit contract.
        if self.fail_update.replace(false) {
            return Err(neocogi_com::NC_E_INVALID_ARG);
        }
        Ok(GeometryChange::NoChange)
    }
    /// Counts real collections without inventing a provider mutation.
    fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
        self.collections.set(self.collections.get() + 1);
        self.state.take_change()
    }
    /// Records normal scoped drawing for render admission.
    fn paint(&mut self, scope: PaintScope, state: WidgetPaint, mut painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
        self.state.readable()?;
        state.validate()?;
        painter.fill(scope, state.rect, 0xff112233)
    }
}
impl ILeafLayout for Provider {
    /// Returns a valid fixed preference.
    fn measure(&mut self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        self.state.readable()?;
        Ok(Size { width: 40, height: 20 })
    }
}

#[com_object(interfaces(IRenderer))]
/// Actual renderer interface records successful admitted frames without GPU work.
struct Renderer {
    /// Test-owned count of frame submissions.
    calls: Rc<Cell<usize>>,
}
impl IRenderer for Renderer {
    /// Accepts the valid test atlas.
    fn prepare_atlas(&self, _: ComRef<'_, IRgbaImageAbi>) -> Result<(), NcStatus> {
        Ok(())
    }
    /// Records admitted frame submission.
    fn render(&self, _: RenderInfo, _: ComRef<'_, IRgbaImageAbi>, _: ComRef<'_, IRenderStreamAbi>) -> Result<(), NcStatus> {
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
}

/// Pre-commit foreign callback rejection schedules runtime-owned dirty recovery.
#[test]
fn precommit_callback_errors_recover_without_provider_notification() {
    for failure in [Failure::Update, Failure::Properties] {
        let atlas = neocogi_ui_support::test_support::test_atlas();
        let assets = neocogi_ui_support::test_support::assets::export_bundle(&neocogi_ui_support::SkinBundle::from_atlas(atlas)).unwrap();
        let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
        let mut runtime = crate::create_runtime_from_assets(assets.as_com_ref(), gfx.as_com_ref()).unwrap();
        runtime.set_render_host(neocogi_gfx_support::testing::host().as_com_ref()).unwrap();
        let host = runtime.construction_host().unwrap().host().unwrap();
        let fail_update = Rc::new(Cell::new(false));
        let fail_properties = Rc::new(Cell::new(false));
        let collections = Rc::new(Cell::new(0));
        let widget = Provider::into_box(Provider {
            state: WidgetState::new(host.clone(), WidgetOptions::NONE),
            fail_update: fail_update.clone(),
            fail_properties: fail_properties.clone(),
            collections: collections.clone(),
        })
        .query::<IWidgetAbi>()
        .unwrap();
        let manager = runtime.query::<IWindowManagerAbi>().unwrap();
        let _window = manager
            .create_window(
                NcCopyFromBufferOffset::new(0, b"review"),
                Rect { x: 0, y: 0, width: 200, height: 120 },
                widget,
                NcValueRef::new(&neocogi_ui::events::EventBinding::Unobserved).unwrap(),
            )
            .unwrap();
        let size = Size { width: 640, height: 480 };
        let render_calls = Rc::new(Cell::new(0));
        let renderer = Renderer::from_value(Renderer { calls: render_calls.clone() }).query::<IRendererAbi>().unwrap();
        runtime.update(size).unwrap();
        runtime.update(size).unwrap();
        runtime.render(size, renderer.as_com_ref()).unwrap();
        collections.set(0);
        match failure {
            Failure::Update => fail_update.set(true),
            Failure::Properties => fail_properties.set(true),
        }
        assert_eq!(runtime.update(size), Err(neocogi_com::NC_E_INVALID_ARG));
        // The provider correctly owns no mutation marker for a pre-commit rejection.
        // Runtime conservative invalidation must nevertheless retry its own collector.
        runtime.update(size).unwrap();
        runtime.render(size, renderer.as_com_ref()).unwrap();
        let recovered_collections = collections.get();
        assert!(recovered_collections > 0, "runtime dirty state must schedule collection");
        for _ in 0..2 {
            runtime.update(size).unwrap();
            runtime.render(size, renderer.as_com_ref()).unwrap();
            assert_eq!(collections.get(), recovered_collections, "settled retries must skip foreign collection");
        }
        assert_eq!(render_calls.get(), 4);
    }
}
