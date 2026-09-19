//! Standard picker acceptance exercises public methods, routed input, and exact native filesystem values.
use super::*;
use neocogi_ui::EventHandler;
use neocogi_ui::events::completed_file_path;

/// Deterministic drawable keeps the full modal visible while testing its real flex layout.
const SIZE: Size = Size { width: 900, height: 600 };
/// Plain background window remains independent of the picker modal.
const OWNER_RECT: Rect = Rect { x: 0, y: 0, width: 800, height: 550 };

/// Public client fixture keeps semantic picker observations without borrowing core storage.
struct Fixture {
    /// Ordinary static facade used for all input, lifecycle and request calls.
    ui: neocogi_ui::Ui,
    /// Original canonical runtime projection permits a second static client without constructing a second COM identity.
    runtime: ComPtr<UiRuntimeAbi>,
    /// Actual ordinary owner authenticated by the production file-picker factory.
    owner: ComPtr<IWindowAbi>,
    /// Public component client; request state never lives in this wrapper.
    handle: neocogi_ui::FileDialog,
    /// Same runtime-owned concrete controller, retained only for test observations of fixed roles.
    controller: Rc<Dialog>,
}
impl Fixture {
    /// Uses public production runtime and node-provider construction, exactly as plugin clients do.
    fn new() -> Self {
        let bundle = crate::SkinBundle::from_atlas(crate::test_support::test_atlas());
        let source = crate::test_support::assets::export_bundle(&bundle).unwrap();
        let compositor = neocogi_gfx_impl::GfxCompositorObject::create(source.as_com_ref()).unwrap();
        let runtime = neocogi_ui_runtime::create_runtime_from_assets(source.as_com_ref(), compositor.as_com_ref()).unwrap();
        let ui = crate::bind_builtin_nodes(runtime.clone()).unwrap();
        let body = ui.widgets().button("background").unwrap();
        let owner = ui.window("owner", OWNER_RECT, &body).unwrap();
        let services = crate::factory::NodeServices::bind(runtime.as_com_ref()).unwrap();
        let controller = Dialog::new(services, owner.as_com_ref()).unwrap();
        let interface = FileDialog::from_value(FileDialog { dialog: controller.clone() }).query().unwrap();
        let handle = neocogi_ui::FileDialog::from_interface(interface);
        Self { ui, runtime, owner, handle, controller }
    }
    /// Queues one real key press with explicit modifier policy.
    fn key(&self, key: KeyCode, modifiers: u8) {
        self.ui
            .input(
                UiInput::Key(KeyStroke {
                    key,
                    value: 0,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }),
                "",
            )
            .unwrap();
    }
    /// Traverses actual keyboard-eligible COM controls using ordinary Tab input.
    fn tabs(&self, count: usize) {
        for _ in 0..count {
            self.key(KeyCode::Tab, 0);
        }
    }
    /// Opens a native directory through the statically linked request/path adapter.
    fn open(&self, path: &Path) {
        self.handle.open(&neocogi_ui::FileDialogRequest::new(path)).unwrap();
    }
    /// Returns the actual current dynamic file root to verify transactional replacement and handle preservation.
    fn file_root(&self) -> ComPtr<IWidgetAbi> {
        self.controller.session.state().unwrap().files.content().unwrap().node_interface().unwrap()
    }
}
/// Concrete completion consumer owns exact native outcomes independently of dialog/UI lifetime.
struct Record;
impl EventHandler<Vec<Option<PathBuf>>, FileDialogCompletionMessage> for Record {
    /// Clones only the immutable completed native path or cancellation value.
    fn handle(&mut self, completed: &mut Vec<Option<PathBuf>>, event: &FileDialogCompletionMessage) -> Result<(), NcStatus> {
        completed.push(completed_file_path(event)?);
        Ok(())
    }
}
/// Attaches a normal application-owned completion lease to the real picker source.
fn session(fixture: &Fixture) -> EventSession<Vec<Option<PathBuf>>> {
    let events = EventSession::new(Vec::new());
    events.subscribe(fixture.handle.completed(), Record).unwrap();
    events
}

/// Failed initial reads and overlapping requests leave activity, native directory and actual dynamic roots unchanged.
#[test]
fn failed_open_and_overlapping_requests_preserve_previous_state_and_expose_native_diagnostics() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("present"), "file").unwrap();
    let original = fixture.file_root();
    let missing = directory.path().join("missing");
    assert_eq!(fixture.handle.open(&neocogi_ui::FileDialogRequest::new(&missing)), Err(UI_E_IO));
    assert!(!fixture.handle.active().unwrap());
    assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(fixture.file_root().as_ptr(), original.as_ptr());
    assert!(fixture.handle.diagnostic().unwrap().contains("missing"));
    fixture.open(directory.path());
    let live = fixture.file_root();
    assert_ne!(live.as_ptr(), original.as_ptr());
    assert_eq!(fixture.handle.open(&neocogi_ui::FileDialogRequest::new(&missing)), Err(UI_E_BUSY));
    assert_eq!(fixture.file_root().as_ptr(), live.as_ptr());
    assert!(fixture.handle.diagnostic().unwrap().is_empty());
    assert_eq!(fixture.controller.session.state().unwrap().directory, directory.path());
    fixture.handle.close().unwrap();
    assert_eq!(fixture.handle.active(), Err(UI_E_CLOSED));
}

/// A dangling entry fails the complete candidate instead of publishing a partial directory.
#[cfg(unix)]
#[test]
fn broken_entry_rejects_the_complete_directory_candidate() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("readable"), b"ok").unwrap();
    std::os::unix::fs::symlink("missing-target", directory.path().join("broken-link")).unwrap();
    let previous = fixture.file_root();
    assert_eq!(fixture.handle.open(&neocogi_ui::FileDialogRequest::new(directory.path())), Err(UI_E_IO));
    assert_eq!(fixture.file_root().as_ptr(), previous.as_ptr());
    assert_eq!(fixture.handle.active(), Ok(false));
}

/// Empty acceptance stays pending; separate picker capabilities retain independent request queues.
#[test]
fn empty_acceptance_and_independent_picker_requests_use_actual_controls() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&fixture);
    fixture.open(directory.path());
    // Up, Home, Path, Go, Parent, Filename, Cancel, Open: activate Open with an empty filename.
    fixture.tabs(8);
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(fixture.handle.active(), Ok(true));
    assert!(events.state().unwrap().is_empty());
    assert_eq!(fixture.handle.cancel(), Ok(true));
    assert_eq!(fixture.handle.cancel(), Ok(false));

    let second = fixture.ui.file_dialog(fixture.owner.as_com_ref()).unwrap();
    let second_events = EventSession::new(Vec::new());
    second_events.subscribe(second.completed(), Record).unwrap();
    second.open(&neocogi_ui::FileDialogRequest::new(directory.path())).unwrap();
    second.cancel().unwrap();
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    second_events.dispatch().unwrap();
    assert_eq!(&*events.state().unwrap(), &[None]);
    assert_eq!(&*second_events.state().unwrap(), &[None]);
    second.close().unwrap();
}

/// Actual keyboard text entry preserves leading/trailing spaces and completes in the same runtime transaction.
#[test]
fn typed_filename_acceptance_uses_real_controls_and_completes_before_update_returns() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    // Only the parent folder row is focusable: Up, Home, Path, Go, Parent, Filename, Cancel, Open.
    let events = session(&fixture);
    fixture.open(directory.path());
    fixture.tabs(6);
    fixture.ui.input(UiInput::Text, " file name ").unwrap();
    fixture.tabs(2);
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(&*events.state().unwrap(), &[Some(directory.path().join(" file name "))]);
    assert_eq!(fixture.handle.active(), Ok(false));
    assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(fixture.controller.session.state().unwrap().filename.text().unwrap(), " file name ");
    assert_eq!(fixture.handle.cancel(), Ok(false));
    fixture.handle.close().unwrap();
}

/// Native row data survives identical replacement-character display labels, including the same-transaction Open action.
#[cfg(unix)]
#[test]
fn selected_non_utf8_file_preserves_exact_native_identity_across_display_collisions() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join(OsString::from_vec(b"file-\x80".to_vec()));
    let second = directory.path().join(OsString::from_vec(b"file-\x81".to_vec()));
    std::fs::write(&first, "first").unwrap();
    std::fs::write(&second, "second").unwrap();
    assert_eq!(first.to_string_lossy(), second.to_string_lossy());
    let events = session(&fixture);
    fixture.open(directory.path());
    // Four toolbar stops, parent folder, then the first exact native file row.
    fixture.tabs(6);
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(fixture.controller.session.state().unwrap().selected.as_ref(), Some(&first));
    // Second file, filename, Cancel, Open.
    fixture.tabs(4);
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(&*events.state().unwrap(), &[Some(first)]);
    fixture.handle.close().unwrap();
}

/// Failed navigation retains the exact selected path and both live columns instead of publishing a partial listing.
#[test]
fn unreadable_navigation_preserves_selection_directory_and_dynamic_widget_identity() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    let file = directory.path().join("file");
    std::fs::write(&file, "data").unwrap();
    fixture.open(directory.path());
    fixture.tabs(7); // Four toolbar stops, parent folder, child folder, file.
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.controller.session.state().unwrap().selected.as_ref(), Some(&file));
    let root = fixture.file_root();
    std::fs::remove_dir(&folder).unwrap();
    fixture.key(KeyCode::Tab, 4); // Shift+Tab returns to the now-unreadable retained folder row.
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.file_root().as_ptr(), root.as_ptr());
    let model = fixture.controller.session.state().unwrap();
    assert_eq!(model.selected.as_ref(), Some(&file));
    assert_eq!(model.directory, directory.path());
    assert!(model.error.text().unwrap().contains("Cannot read"));
    drop(model);
    fixture.handle.close().unwrap();
}

/// Explicit cancel/reopen ignores an old hide notification; owner hide and destruction settle exactly one active request.
#[test]
fn lifecycle_cancellation_is_once_per_request_and_stale_hide_does_not_cancel_reopening() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&fixture);
    fixture.open(directory.path());
    assert_eq!(fixture.handle.cancel(), Ok(true));
    fixture.open(directory.path());
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(&*events.state().unwrap(), &[None]);
    assert_eq!(fixture.handle.active(), Ok(true));
    fixture.owner.set_visibility(Visibility::Hidden).unwrap();
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(events.state().unwrap().len(), 2);
    assert_eq!(fixture.handle.active(), Ok(false));
    fixture.owner.set_visibility(Visibility::Visible).unwrap();
    fixture.open(directory.path());
    fixture.owner.close().unwrap();
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(events.state().unwrap().len(), 3);
    assert_eq!(fixture.handle.active(), Err(UI_E_CLOSED));
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(events.state().unwrap().len(), 3);
}

/// Final runtime release queues cancellation without invoking application code or retaining the runtime through its internal handlers.
#[test]
fn runtime_release_preserves_active_request_terminal_cancellation_without_a_controller_cycle() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&fixture);
    fixture.open(directory.path());
    let host = fixture.runtime.construction_host().unwrap().host().unwrap();
    let handle = fixture.handle.clone();
    drop(fixture);
    assert_eq!(host.check_open(), Err(UI_E_CLOSED));
    assert!(events.state().unwrap().is_empty());
    assert_eq!(handle.active(), Err(UI_E_CLOSED));
    assert_eq!(events.dispatch(), Ok(1));
    assert_eq!(&*events.state().unwrap(), &[None]);
    assert_eq!(events.dispatch(), Ok(0));
}

/// Successful folder navigation replaces only dynamic rows, keeps controls/scroll models and clamps against new content.
#[test]
fn successful_navigation_reuses_fixed_controls_and_replaces_the_complete_dynamic_listing() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let child = directory.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(child.join("new file"), "data").unwrap();
    for index in 0..60 {
        std::fs::write(directory.path().join(format!("old-{index:02}")), "data").unwrap();
    }
    fixture.open(directory.path());
    let old_root = fixture.file_root();
    let path = fixture.controller.session.state().unwrap().path.clone();
    let mut scroll = fixture.controller.session.state().unwrap().files.clone();
    scroll.set_offset(Point { x: 0, y: i32::MAX }).unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert!(scroll.info().unwrap().requested.y > 0);
    fixture.tabs(6); // Four toolbar stops, parent folder, then the child directory.
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_ne!(fixture.file_root().as_ptr(), old_root.as_ptr());
    assert_eq!(fixture.controller.session.state().unwrap().directory, child);
    assert_eq!(path.text().unwrap(), child.to_string_lossy());
    assert_eq!(scroll.info().unwrap().requested.y, 0);
    assert!(fixture.controller.session.state().unwrap().selected.is_none());
    fixture.handle.close().unwrap();
}

/// Application handler data explicitly identifies the reusable picker and its exact native directory.
struct OpenPicker {
    /// Existing component is fully subscribed before the application begins dispatch.
    picker: neocogi_ui::FileDialog,
    /// Exact native directory included in the request without display conversion.
    directory: PathBuf,
}
impl EventHandler<Vec<Option<PathBuf>>, Activation> for OpenPicker {
    /// Opens through the ordinary COM method while the runtime context is available between input phases.
    fn handle(&mut self, _state: &mut Vec<Option<PathBuf>>, _event: &Activation) -> Result<(), NcStatus> {
        self.picker.open(&neocogi_ui::FileDialogRequest::new(&self.directory))
    }
}

/// A popup-opening app callback exposes the new modal to later already-queued native input in the same update.
#[test]
fn application_callback_opens_picker_and_later_input_reaches_its_new_modal_scope() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    // A separate activator lets the picker retain its original ordinary owner and tests cross-window modal routing.
    let button = fixture.ui.widgets().button("open picker").unwrap();
    let activator = fixture.ui.window("activator", OWNER_RECT, &button).unwrap();
    let events = session(&fixture);
    events
        .subscribe(
            button.activated(),
            OpenPicker {
                picker: fixture.handle.clone(),
                directory: directory.path().to_owned(),
            },
        )
        .unwrap();
    fixture.tabs(1);
    fixture.key(KeyCode::Space, 0);
    fixture.tabs(6);
    fixture.ui.input(UiInput::Text, "after callback").unwrap();
    fixture.tabs(2);
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update_with_events(SIZE, &events).unwrap();
    assert_eq!(&*events.state().unwrap(), &[Some(directory.path().join("after callback"))]);
    assert_eq!(fixture.handle.active(), Ok(false));
    fixture.handle.close().unwrap();
    activator.close().unwrap();
}

/// Explicit application state owns the ordinary runtime client used during manual completion delivery.
struct ReentryState {
    /// Manual EventSession dispatch does not own an outer runtime update transaction.
    ui: neocogi_ui::Ui,
    /// Every already committed request must survive component retirement during its first callback.
    completions: usize,
}
/// Concrete callback deliberately updates the runtime while the completion lease owns a bounded multi-event delivery window.
struct Reenter;
impl EventHandler<ReentryState, FileDialogCompletionMessage> for Reenter {
    /// Updates ordinary UI state without reentering this event session; delivery must retain all committed outcomes.
    fn handle(&mut self, state: &mut ReentryState, event: &FileDialogCompletionMessage) -> Result<(), NcStatus> {
        assert!(matches!(event, FileDialogCompletionMessage::Cancelled));
        state.completions += 1;
        state.ui.update(SIZE)
    }
}

/// Request completion is a committed fact even after the closed controller is retired during an active delivery window.
#[test]
fn closed_picker_preserves_all_completed_requests_during_manual_handler_runtime_reentry() {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = EventSession::new(ReentryState {
        ui: crate::bind_builtin_nodes(fixture.runtime.clone()).unwrap(),
        completions: 0,
    });
    events.subscribe(fixture.handle.completed(), Reenter).unwrap();
    for _ in 0..2 {
        fixture.open(directory.path());
        fixture.handle.cancel().unwrap();
    }
    fixture.handle.close().unwrap();
    assert_eq!(events.dispatch(), Ok(2));
    assert_eq!(events.state().unwrap().completions, 2);
    assert_eq!(events.dispatch(), Ok(0));
}
