//! Retained behavior is exercised through public clients and the production compositor.

use neocogi_com::{ComPtr, NcStatus};
use neocogi_ui::*;
use crate::test_support::headless::Headless;
use std::{cell::RefCell, rc::Rc};

mod persistence;
mod split_view;

/// Stable drawable bounds allow independent windows and child-family overlap tests.
const VIEW: Size = Size { width: 640, height: 400 };
/// Chromeless windows make root content coincide with explicitly authored geometry.
const PLAIN: WindowOptions = WindowOptions::from_raw(128 | 16 | 8);

/// Observable application state is kept outside the retained tree, as for an independent widget.
#[derive(Default)]
struct Observations {
    /// Every ordinary/routed update carries a copied screen-space allocation and focus state.
    updates: Vec<(WidgetUpdate, WidgetEvent)>,
    /// Paint snapshots expose effective enablement and clipping without private node inspection.
    paints: Vec<WidgetPaint>,
    /// Measurement invocations distinguish actual work from production cache hits.
    measures: usize,
    /// Desired content size can change during the next explicit update.
    preferred: Size,
    /// A requested update must truthfully invalidate retained measurement.
    change: bool,
    /// Logical closure is observable even while the application's capability remains retained.
    closed: bool,
    /// An optional unique solid identifies this leaf in the actual compositor output.
    color: Option<u32>,
}

/// Concrete local behavior is exported using the facade's standard statically typed leaf adapter.
struct Probe<const KEYBOARD: u16> {
    /// Callback observations need sharing with assertions; all mutation stays within one borrow.
    observations: Rc<RefCell<Observations>>,
}
impl<const KEYBOARD: u16> LeafContent for Probe<KEYBOARD> {
    const KEYBOARD: u16 = KEYBOARD;
    /// Measures actual application data through the same callback as an independent component.
    fn measure(&self, _: LayoutLimits, _: neocogi_com::ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        let mut observations = self.observations.borrow_mut();
        observations.measures += 1;
        Ok(observations.preferred)
    }
    /// Records event order and consumes one explicit application-state geometry change.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        let mut observations = self.observations.borrow_mut();
        observations.updates.push((state, event));
        Ok(if std::mem::take(&mut observations.change) {
            GeometryChange::Changed
        } else {
            GeometryChange::NoChange
        })
    }
    /// Records the checked paint snapshot without introducing another rendering implementation.
    fn paint(&mut self, state: WidgetPaint, painter: &mut Painter<'_>) -> Result<(), NcStatus> {
        let mut observations = self.observations.borrow_mut();
        observations.paints.push(state);
        if let Some(color) = observations.color {
            painter.fill(state.rect, color)?;
        }
        Ok(())
    }

}

impl<const KEYBOARD: u16> Drop for Probe<KEYBOARD> {
    /// Resource removal follows the unique owner, independently of test handles.
    fn drop(&mut self) { self.observations.borrow_mut().closed = true; }
}

/// Creates an ordinary COM leaf and a typed observation handle for assertions.
fn probe<const KEYBOARD: u16>(ui: &Ui, size: Size, options: WidgetOptions) -> (OwnedWidget<IWidgetAbi>, Rc<RefCell<Observations>>) {
    let observations = Rc::new(RefCell::new(Observations {
        preferred: size,
        ..Observations::default()
    }));
    let widget = ui.widgets().leaf(Probe::<KEYBOARD> { observations: observations.clone() }, options).unwrap();
    (widget, observations)
}

/// Queues one complete key transition with explicit modifiers and function/character payload.
fn key(ui: &Ui, code: KeyCode, value: u32, modifiers: u8) {
    for pressed in [true, false] {
        ui.input(
            UiInput::Key(KeyStroke {
                key: code,
                value,
                modifiers,
                pressed,
                repeat: false,
            }),
            "",
        )
        .unwrap();
    }
}

/// Queues a primary-button press or release in drawable coordinates.
fn pointer(ui: &Ui, position: Point, pressed: bool) {
    ui.input(UiInput::Pointer(position), "").unwrap();
    ui.input(
        if pressed {
            UiInput::Down { position, buttons: 1 }
        } else {
            UiInput::Up { position, buttons: 1 }
        },
        "",
    )
    .unwrap();
}

/// Uses public window creation and presentation settings, preserving all normal mount validation.
fn window<I: neocogi_com::ComInterface>(ui: &Ui, name: &str, rect: Rect, widget: OwnedWidget<I>) -> ComPtr<IWindowAbi> {
    let window = ui.window(name, rect, widget).unwrap();
    window.set_options(PLAIN).unwrap();
    window
}

/// Eventless ticks update widgets once, rendering remains observational, and cached layout persists.
#[test]
fn eventless_update_and_paint_have_separate_phase_counts() {
    let mut fixture = Headless::standard();
    let (widget, observed) = probe::<0>(&fixture.ui, Size { width: 40, height: 20 }, WidgetOptions::NONE);
    let _window = window(&fixture.ui, "phases", Rect { x: 10, y: 10, width: 100, height: 80 }, widget);
    fixture.frame(VIEW).unwrap();
    let measures = observed.borrow().measures;
    assert_eq!(observed.borrow().updates.len(), 1);
    assert_eq!(observed.borrow().paints.len(), 1);
    fixture.ui.render(VIEW, fixture.renderer.as_com_ref()).unwrap();
    assert_eq!(observed.borrow().updates.len(), 1);
    assert_eq!(observed.borrow().paints.len(), 2);
    fixture.frame(VIEW).unwrap();
    assert_eq!(observed.borrow().updates.len(), 2);
    assert_eq!(observed.borrow().measures, measures);
    assert_eq!(fixture.snapshot().calls, 3);
}

/// Invalid dimensions reject before queued input is consumed or the renderer is entered.
#[test]
fn invalid_update_dimensions_preserve_pending_input() {
    let mut fixture = Headless::standard();
    let editor_owner = fixture.ui.widgets().textbox_unobserved("").unwrap();
    let editor = editor_owner.handle();
    let _window = window(&fixture.ui, "input", Rect { x: 10, y: 10, width: 200, height: 100 }, editor_owner);
    key(&fixture.ui, KeyCode::Tab, 0, 0);
    fixture.ui.input(UiInput::Text, "kept").unwrap();
    for size in [Size { width: 0, height: 20 }, Size { width: 20, height: -1 }] {
        assert_eq!(fixture.ui.update(size), Err(neocogi_com::NC_E_INVALID_ARG));
    }
    assert_eq!(editor.text().unwrap(), "");
    fixture.frame(VIEW).unwrap();
    assert_eq!(editor.text().unwrap(), "kept");
}

/// Ordinary typed setters invalidate both their own cached preference and the enclosing auto size.
#[test]
fn auto_width_consumes_typed_measurement_invalidation_before_intrinsic_measurement() {
    let mut fixture = Headless::standard();
    let text_owner = fixture.ui.widgets().text_block("a", TextWrapping::None).unwrap();
    let mut text = text_owner.handle();
    let window = window(&fixture.ui, "auto", Rect { x: 10, y: 10, width: 300, height: 50 }, text_owner);
    window.set_options(PLAIN | WindowOptions::AUTO_WIDTH).unwrap();
    fixture.frame(VIEW).unwrap();
    let first = window.snapshot().unwrap().rect;
    text.set_text("aaaa").unwrap();
    assert_eq!(fixture.ui.render(VIEW, fixture.renderer.as_com_ref()), Err(UI_E_UPDATE_REQUIRED));
    fixture.frame(VIEW).unwrap();
    let next = window.snapshot().unwrap().rect;
    assert_eq!(next.width, first.width + 24);
    assert_eq!(next.height, 50);
}

/// Auto sizing ignores previous extents on each requested axis while preserving the other axis.
#[test]
fn auto_size_uses_intrinsic_geometry_only_on_requested_axes() {
    for (options, expected) in [
        (WindowOptions::AUTO_WIDTH, (40, 90)),
        (WindowOptions::AUTO_HEIGHT, (160, 20)),
        (WindowOptions::AUTO_SIZE, (40, 20)),
    ] {
        let mut fixture = Headless::standard();
        let (widget, _) = probe::<0>(&fixture.ui, Size { width: 40, height: 20 }, WidgetOptions::NONE);
        let window = window(&fixture.ui, "auto", Rect { x: 10, y: 10, width: 160, height: 90 }, widget);
        window.set_options(PLAIN | options).unwrap();
        fixture.frame(VIEW).unwrap();
        let rect = window.snapshot().unwrap().rect;
        assert_eq!((rect.width, rect.height), expected);
    }
}

/// Hiding a structural parent preserves descendant state; closing it ends all descendant activity.
#[test]
fn parent_visibility_and_destruction_cover_the_complete_child_family() {
    let mut fixture = Headless::standard();
    let (parent_leaf, parent_state) = probe::<0>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let parent = window(&fixture.ui, "parent", Rect { x: 10, y: 10, width: 200, height: 150 }, parent_leaf);
    let (child_leaf, child_state) = probe::<0>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let child = fixture
        .ui
        .child_window(parent.as_com_ref(), "child", Rect { x: 20, y: 20, width: 100, height: 80 }, child_leaf)
        .unwrap();
    child.set_options(PLAIN).unwrap();
    fixture.frame(VIEW).unwrap();
    let child_updates = child_state.borrow().updates.len();
    parent.set_visibility(Visibility::Hidden).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(child.snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(child_state.borrow().updates.len(), child_updates);
    assert!(!child_state.borrow().closed);
    parent.set_visibility(Visibility::Visible).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(child.snapshot().unwrap().visibility, Visibility::Visible);
    assert!(child_state.borrow().updates.len() > child_updates);
    parent.close().unwrap();
    assert!(parent_state.borrow().closed);
    assert!(child_state.borrow().closed);
    assert_eq!(child.snapshot().unwrap_err(), UI_E_CLOSED);
}

/// Hidden typed changes are collected and committed through the actual runtime boundary.
#[test]
fn hidden_widget_mutation_is_laid_out_when_its_surface_is_shown() {
    let mut fixture = Headless::standard();
    let text_owner = fixture.ui.widgets().text_block("a", TextWrapping::None).unwrap();
    let mut text = text_owner.handle();
    let window = window(&fixture.ui, "hidden", Rect { x: 10, y: 10, width: 100, height: 60 }, text_owner);
    window.set_options(PLAIN | WindowOptions::AUTO_WIDTH).unwrap();
    fixture.frame(VIEW).unwrap();
    let width = window.snapshot().unwrap().rect.width;
    window.set_visibility(Visibility::Hidden).unwrap();
    text.set_text("aaaa").unwrap();
    fixture.frame(VIEW).unwrap();
    window.set_visibility(Visibility::Visible).unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(window.snapshot().unwrap().rect.width, width + 24);
}

/// A modal dialog suspends the underlying editor and restores its remembered focus on closure.
#[test]
fn modal_activation_suspends_underlying_focus_and_restores_it_when_closed() {
    let mut fixture = Headless::standard();
    let parent_text_owner = fixture.ui.widgets().textbox_unobserved("").unwrap();
    let parent_text = parent_text_owner.handle();
    let parent = window(&fixture.ui, "parent", Rect { x: 10, y: 10, width: 200, height: 120 }, parent_text_owner);
    key(&fixture.ui, KeyCode::Tab, 0, 0);
    fixture.ui.input(UiInput::Text, "before").unwrap();
    fixture.frame(VIEW).unwrap();
    let dialog_text_owner = fixture.ui.widgets().textbox_unobserved("").unwrap();
    let dialog_text = dialog_text_owner.handle();
    let dialog = fixture
        .ui
        .dialog(parent.as_com_ref(), "dialog", Rect { x: 30, y: 30, width: 160, height: 90 }, dialog_text_owner)
        .unwrap();
    dialog.set_options(PLAIN).unwrap();
    dialog.set_visibility(Visibility::Visible).unwrap();
    key(&fixture.ui, KeyCode::Tab, 0, 0);
    fixture.ui.input(UiInput::Text, "modal").unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(parent_text.text().unwrap(), "before");
    assert_eq!(dialog_text.text().unwrap(), "modal");
    dialog.close().unwrap();
    fixture.ui.input(UiInput::Text, "after").unwrap();
    fixture.frame(VIEW).unwrap();
    assert_eq!(parent_text.text().unwrap(), "beforeafter");
}

/// Fixed bands determine hit order even after a lower window is raised repeatedly.
#[test]
fn raising_reorders_only_inside_a_fixed_layer() {
    let mut fixture = Headless::standard();
    let (low_leaf, low) = probe::<1>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let low_window = window(&fixture.ui, "low", Rect { x: 10, y: 10, width: 200, height: 120 }, low_leaf);
    let (high_leaf, high) = probe::<1>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let high_window = window(&fixture.ui, "high", Rect { x: 10, y: 10, width: 200, height: 120 }, high_leaf);
    low_window.set_layer(2).unwrap();
    high_window.set_layer(3).unwrap();
    low_window.raise().unwrap();
    fixture.frame(VIEW).unwrap();
    pointer(&fixture.ui, Point { x: 20, y: 20 }, true);
    fixture.frame(VIEW).unwrap();
    assert!(high.borrow().updates.last().unwrap().0.active);
    assert!(!low.borrow().updates.last().unwrap().0.active);
    assert_eq!(low_window.set_layer(16), Err(neocogi_com::NC_E_INVALID_ARG));
}

/// Disabled structural descendants paint as disabled, while the parent's dialog remains interactive.
#[test]
fn disabled_parent_disables_child_windows_but_not_its_modal_dialog() {
    let mut fixture = Headless::standard();
    let (root_leaf, root_state) = probe::<0>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let root = window(&fixture.ui, "disabled", Rect { x: 10, y: 10, width: 200, height: 140 }, root_leaf);
    let (child_leaf, child_state) = probe::<0>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let child = fixture
        .ui
        .child_window(root.as_com_ref(), "child", Rect { x: 20, y: 20, width: 100, height: 80 }, child_leaf)
        .unwrap();
    child.set_options(PLAIN).unwrap();
    root.set_options(PLAIN | WindowOptions::DISABLED).unwrap();
    fixture.frame(VIEW).unwrap();
    assert!(!root_state.borrow().paints.last().unwrap().enabled);
    assert!(!child_state.borrow().paints.last().unwrap().enabled);
    let (dialog_leaf, dialog_state) = probe::<0>(&fixture.ui, Size { width: 20, height: 20 }, WidgetOptions::NONE);
    let dialog = fixture
        .ui
        .dialog(root.as_com_ref(), "dialog", Rect { x: 240, y: 20, width: 100, height: 80 }, dialog_leaf)
        .unwrap();
    dialog.set_options(PLAIN).unwrap();
    dialog.set_visibility(Visibility::Visible).unwrap();
    fixture.frame(VIEW).unwrap();
    assert!(dialog_state.borrow().paints.last().unwrap().enabled);
}

mod policies;

mod resources;

mod reload;
mod document_limits;
