//! Focus delegation crosses the real facade and manager routing boundaries.
use super::*;
use neocogi_ui::{LeafContent, LinearEdge, Painter};

/// A concrete embedded surface reports routed keys and its committed focus state.
struct Scope {
    /// Observations remain owned by the test outside the widget's COM call gate.
    events: std::sync::mpsc::Sender<(bool, Option<KeyStroke>)>,
}
impl LeafContent for Scope {
    /// Focusable Tab stop with nested keyboard routing authority.
    const KEYBOARD: u16 = 1 | 2 | 512;
    /// A stable extent makes this routing fixture independent of text measurement.
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 80, height: 30 })
    }
    /// Records copied routed input without retaining a runtime or mutating another UI.
    fn update(&mut self, state: WidgetUpdate, event: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        let key = match event {
            WidgetEvent::Key(key) => Some(key),
            _ => None,
        };
        self.events.send((state.focused, key)).unwrap();
        Ok(GeometryChange::NoChange)
    }
    /// No graphics are needed to observe manager keyboard ownership.
    fn paint(&self, _: WidgetPaint, _: &mut Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
}

/// Creates a complete key press or release without implicit platform state.
fn key(code: KeyCode, value: u32, modifiers: u8, pressed: bool) -> UiInput {
    UiInput::Key(KeyStroke {
        key: code,
        value,
        modifiers,
        pressed,
        repeat: false,
    })
}

/// Tab stays inside a focused embedded scope; Ctrl+Tab exits into normal traversal.
#[test]
fn embedded_scope_receives_tab_and_menu_keys_until_explicit_exit() {
    let runtime = runtime();
    let factory = neocogi_ui_nodes::UiFactory::bind(runtime.as_com_ref()).unwrap();
    let ui = neocogi_ui::Ui::from_interfaces(runtime, factory).unwrap();
    let (send, events) = std::sync::mpsc::channel();
    let scope = ui.widgets().leaf(Scope { events: send }, WidgetOptions::NONE).unwrap();
    let next = ui.widgets().button("next").unwrap();
    let mut column = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch).unwrap();
    column.insert(0, &scope, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    column.insert(1, &next, LinearEdge::new(LayoutTrack::Content)).unwrap().finish().unwrap();
    ui.window("scope", RECT, &column).unwrap();
    ui.update(SIZE).unwrap();
    ui.input(key(KeyCode::Tab, 0, 0, true), "").unwrap();
    ui.update(SIZE).unwrap();
    assert!(events.try_iter().any(|(focused, _)| focused));
    // These manager-level keys must be delivered to the embedded widget itself.
    for (code, value, modifiers) in [(KeyCode::Tab, 0, 0), (KeyCode::Function, 10, 0), (KeyCode::Function, 6, 2)] {
        ui.input(key(code, value, modifiers, true), "").unwrap();
        ui.input(key(code, value, modifiers, false), "").unwrap();
        ui.update(SIZE).unwrap();
        let keys = events.try_iter().filter_map(|(_, key)| key).collect::<Vec<_>>();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| key.key == code && key.value == value));
    }
    ui.input(key(KeyCode::Tab, 0, 2, true), "").unwrap();
    ui.update(SIZE).unwrap();
    let updates = events.try_iter().collect::<Vec<_>>();
    assert!(updates.iter().any(|(focused, _)| !focused));
    assert!(updates.iter().all(|(_, key)| key.is_none()));
    // Ordinary Shift+Tab can return to the scope, without a sticky delegation flag.
    ui.input(key(KeyCode::Tab, 0, 4, true), "").unwrap();
    ui.update(SIZE).unwrap();
    assert!(events.try_iter().any(|(focused, _)| focused));
}
