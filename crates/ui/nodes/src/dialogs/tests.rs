//! Standard picker acceptance exercises public methods, routed input, and exact native filesystem values.
use super::*;
use neocogi_com::NcValueRef;
use neocogi_ui::events::EventOrigin;
use neocogi_ui_support::test_support::events::TestApp;

mod allocations;
/// Terminal recorder extracts the native path while it owns the event.
fn completion(event: &ComBox<neocogi_ui::events::IEventAbi>) -> Result<Option<PathBuf>, NcStatus> {
    match event.query_weak::<IFileAcceptedEventAbi>() {
        Ok(event) => neocogi_ui::path::decode(event.path()?.as_com_ref()).map(Some),
        Err(neocogi_com::NC_E_NOINTERFACE) => {
            event.query_weak::<IFileCancelledEventAbi>()?;
            Ok(None)
        }
        Err(status) => Err(status),
    }
}

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
    /// Same runtime-owned concrete widget, retained only for test observations of fixed roles.
    widget: WeakLocalBox<FileDialog>,
    /// Default completion inbox is owned by this fixture until a test consumes it.
    messages: Option<std::sync::mpsc::Receiver<Option<PathBuf>>>,
}
impl Fixture {
    /// Uses public production runtime and node-provider construction, exactly as plugin clients do.
    fn new() -> Self {
        let (send, messages) = std::sync::mpsc::channel();
        let queue = neocogi_ui::test_event_queue!(neocogi_ui::IFileDialogAbi, Option<PathBuf>, send, |_queue, _origin, event| completion(event)?).unwrap();
        let mut fixture = Self::with_queue(queue);
        fixture.messages = Some(messages);
        fixture
    }
    /// Supplies specialized test mappings before the widget and its window exist.
    fn with_queue(queue: ComPtr<neocogi_ui::events::IEventReceiverAbi>) -> Self {
        Self::with_completion(Some(queue))
    }
    /// Exercises the same factory with either an upstream receiver or no observation.
    fn with_completion(queue: Option<ComPtr<neocogi_ui::events::IEventReceiverAbi>>) -> Self {
        let bundle = crate::SkinBundle::from_atlas(crate::test_support::test_atlas());
        let source = crate::test_support::assets::export_bundle(&bundle).unwrap();
        let gfx = neocogi_ui_support::test_support::gfx_factory(crate::test_support::host_runtime());
        let runtime = nc_ui::create_runtime_from_assets(source.as_com_ref(), gfx.as_com_ref()).unwrap();
        let ui = crate::bind_builtin_nodes(runtime.clone()).unwrap();
        let body = ui.widgets().button_unobserved("background").unwrap();
        let owner = ui.window("owner", OWNER_RECT, body).unwrap();
        let services = crate::factory::NodeServices::bind(runtime.as_com_ref()).unwrap();
        let widget = FileDialog::new(services, owner.as_com_ref(), queue).unwrap();
        let interface = widget.query().unwrap();
        let handle = neocogi_ui::FileDialog::from_interface(interface);
        Self {
            ui,
            runtime,
            owner,
            handle,
            widget,
            messages: None,
        }
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
    fn open(&mut self, path: &Path) {
        self.handle.open(&neocogi_ui::FileDialogRequest::new(path)).unwrap();
    }
    /// Returns the actual current dynamic file root to verify transactional replacement and handle preservation.
    fn file_root(&self) -> WeakComBox<IWidgetAbi> {
        self.widget
            .with(|dialog| dialog.model.files.content().unwrap().node_interface().unwrap())
            .unwrap()
    }
}
/// Connects the real picker completion protocol to a consumer-owned inbox.
fn session(fixture: &mut Fixture) -> std::sync::mpsc::Receiver<Option<PathBuf>> {
    fixture.messages.take().expect("one consumer per constructed picker")
}

/// Both observations cross real generated interfaces while traversal owns the runtime context.
type TraversalAttempt = (Result<bool, NcStatus>, Result<Visibility, NcStatus>);

/// An ordinary modal-owned popup leaf can request cancellation during its update.
struct CancelDuringTraversal {
    /// Weak public editing capability, independent of the popup's exclusive leaf owner.
    picker: neocogi_ui::FileDialog,
    /// Only the outer test observes the result, after traversal has returned its context.
    report: std::sync::mpsc::Sender<TraversalAttempt>,
    /// A single attempt makes the subsequent outside-traversal retry unambiguous.
    attempted: bool,
}
impl neocogi_ui::LeafContent for CancelDuringTraversal {
    fn measure(&self, _: LayoutLimits, _: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
        Ok(Size { width: 32, height: 24 })
    }
    fn paint(&mut self, _: WidgetPaint, _: &mut neocogi_ui::Painter<'_>) -> Result<(), NcStatus> {
        Ok(())
    }
    fn update(&mut self, _: WidgetUpdate, _: WidgetEvent, _: &str) -> Result<GeometryChange, NcStatus> {
        if !self.attempted {
            self.attempted = true;
            let visibility = self.picker.window()?.snapshot().map(|snapshot| snapshot.visibility);
            self.report.send((self.picker.cancel(), visibility)).unwrap();
        }
        Ok(GeometryChange::NoChange)
    }
}

/// Actual context checkout must reject completion before Idle or event creation.
#[test]
fn cancellation_during_modal_popup_traversal_preserves_request_until_retry() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let messages = session(&mut fixture);
    fixture.open(directory.path());
    let modal = fixture.handle.window().unwrap();
    let (report, result) = std::sync::mpsc::channel();
    let leaf = fixture
        .ui
        .widgets()
        .leaf(
            CancelDuringTraversal {
                picker: fixture.handle.clone(),
                report,
                attempted: false,
            },
            WidgetOptions::NONE,
        )
        .unwrap();
    let mut popup = fixture.ui.popup(modal.as_com_ref(), "cancel during update", leaf).unwrap();
    popup.show_at(Rect { x: 30, y: 30, width: 32, height: 24 }).unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(result.try_recv().unwrap(), (Err(UI_E_BUSY), Err(UI_E_BUSY)));
    assert_eq!(fixture.handle.active(), Ok(true));
    assert_eq!(modal.snapshot().unwrap().visibility, Visibility::Visible);
    assert!(messages.try_recv().is_err());
    assert_eq!(fixture.handle.cancel(), Ok(true));
    assert_eq!(fixture.handle.active(), Ok(false));
    assert_eq!(modal.snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(messages.try_recv(), Ok(None));
    assert_eq!(fixture.handle.cancel(), Ok(false));
    assert!(messages.try_recv().is_err());
    fixture.handle.close().unwrap();
}

/// Accepts the real completion ABI without allocating an application message.
#[com_object(interfaces(neocogi_ui::events::IEventReceiver))]
struct DiscardingCompletion;
impl neocogi_ui::events::IEventReceiver for DiscardingCompletion {
    /// An unhandled fact is consumed and destroyed synchronously by this receiver.
    fn post(
        &mut self,
        _origin: NcValueRef<'_, EventOrigin>,
        _event: ComBox<neocogi_ui::events::IEventAbi>,
    ) -> Result<(), (NcStatus, ComBox<neocogi_ui::events::IEventAbi>)> {
        Ok(())
    }
}

/// Measures actual built-in construction through both completion paths. Pre-hiding
/// avoids a new window notification; a measured idempotent visibility edit accounts
/// for the manager's own work. The production call still commits an Active request.
#[test]
fn active_completions_construct_and_drop_snapshots_with_or_without_a_receiver() {
    use allocations::{Counts, measure};
    let path = PathBuf::from("selected file");
    let (_, cancelled) = measure(|| drop(neocogi_ui::abi::core::events::file_cancelled().unwrap()));
    assert_eq!(cancelled, Counts { allocations: 1, deallocations: 1 });
    let (_, accepted) = measure(|| {
        drop(neocogi_ui::abi::core::events::file_accepted(neocogi_ui::NativePath::new(&path).into_interface()).unwrap());
    });
    let (_, path_copy) = measure(|| drop(path.clone()));
    for observed in [false, true] {
        let queue = observed.then(|| DiscardingCompletion::from_value(DiscardingCompletion).query().unwrap());
        let mut fixture = Fixture::with_completion(queue);
        let directory = tempfile::tempdir().unwrap();
        fixture.open(directory.path());
        fixture.handle.window().unwrap().set_visibility(Visibility::Hidden).unwrap();
        let window = fixture.handle.window().unwrap();
        let (result, hide) = measure(|| window.set_visibility(Visibility::Hidden));
        assert_eq!(result, Ok(()));
        assert_eq!(fixture.handle.active(), Ok(true));
        let (result, counts) = measure(|| fixture.handle.cancel());
        assert_eq!(result, Ok(true));
        assert_eq!(
            counts,
            Counts {
                allocations: hide.allocations + cancelled.allocations,
                deallocations: hide.deallocations + cancelled.deallocations,
            },
            "observed={observed}: cancel must construct and release its one event"
        );
        assert_eq!(fixture.handle.active(), Ok(false));
        let (result, counts) = measure(|| fixture.handle.cancel());
        assert_eq!(result, Ok(false));
        assert_eq!(counts, Counts::default(), "Idle has no cancellation occurrence");

        fixture.open(directory.path());
        fixture.handle.window().unwrap().set_visibility(Visibility::Hidden).unwrap();
        fixture.widget.with_mut(|dialog| dialog.model.selected = Some(path.clone())).unwrap();
        let (result, counts) = measure(|| fixture.widget.with_mut(|dialog| dialog.model.accept()));
        assert_eq!(result, Ok(Ok(())));
        assert_eq!(
            counts,
            Counts {
                allocations: hide.allocations + path_copy.allocations + accepted.allocations,
                deallocations: hide.deallocations + path_copy.deallocations + accepted.deallocations,
            },
            "observed={observed}: accept must capture and release its native-path event"
        );
        assert_eq!(fixture.handle.active(), Ok(false));
        let (result, counts) = measure(|| fixture.widget.with_mut(|dialog| dialog.model.accept()));
        assert_eq!(result, Ok(Ok(())));
        assert_eq!(counts, path_copy, "Idle must not construct another acceptance event");
        fixture.handle.close().unwrap();
    }
}

/// A counted real COM value distinguishes destruction from weak storage lifetime.
#[com_object(interfaces(IFileCancelledEvent), ownership = exclusive)]
struct CountedCancellation {
    /// Shared test observation requires Cell because Drop has no exclusive counter owner.
    drops: Rc<std::cell::Cell<usize>>,
}
impl neocogi_ui::events::IEvent for CountedCancellation {}
impl IFileCancelledEvent for CountedCancellation {}
impl Drop for CountedCancellation {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

/// Direct posting consumes exactly one owner for absence, real queue delivery and
/// ABI rejection. Phase is already committed and cannot be changed by transport.
#[test]
fn direct_completion_post_preserves_or_destroys_the_supplied_owner_once() {
    let fixture = Fixture::with_completion(None);
    let origin = EventOrigin::Widget(fixture.handle.as_interface().clone().into_unknown());
    let (queue, messages) = neocogi_ui::events::channel();
    let refusing = RefusingCompletion::from_value(RefusingCompletion).query().unwrap();
    for (destination, expected, queued) in [
        (None, Ok(()), false),
        (Some(queue), Ok(()), true),
        (Some(refusing), Err(NC_E_INVALID_ARG), false),
    ] {
        let drops = Rc::new(std::cell::Cell::new(0));
        let event = CountedCancellation::into_box(CountedCancellation { drops: drops.clone() })
            .query::<neocogi_ui::events::IEventAbi>()
            .unwrap();
        let observer = event.downgrade();
        let mut request = Request {
            phase: RequestPhase::Idle,
            completed: destination.map(|queue| (queue, origin.clone())),
        };
        assert_eq!(request.post(event), expected);
        assert_eq!(request.phase, RequestPhase::Idle);
        assert_eq!(drops.get(), usize::from(!queued));
        assert_eq!(observer.is_alive(), queued);
        if queued {
            let message = messages.try_recv().unwrap();
            assert_eq!(message.event.as_raw().object, observer.as_raw().object);
            assert!(
                message
                    .origin
                    .widget::<IFileDialogAbi>()
                    .unwrap()
                    .unwrap()
                    .same_object(fixture.handle.as_interface())
            );
            drop(message);
        }
        assert_eq!(drops.get(), 1);
        assert!(!observer.is_alive());
    }
}

/// Explicit out-of-traversal completion queues a weak dialog identity and an owned
/// snapshot that remains readable after the reusable picker is removed.
#[test]
fn queued_completion_keeps_its_snapshot_after_dialog_destruction() {
    let (queue, messages) = neocogi_ui::events::channel();
    let mut fixture = Fixture::with_queue(queue);
    let directory = tempfile::tempdir().unwrap();
    fixture.open(directory.path());
    fixture.handle.cancel().unwrap();
    fixture.open(directory.path());
    let path = directory.path().join("captured path");
    fixture
        .widget
        .with_mut(|dialog| {
            dialog.model.selected = Some(path.clone());
            dialog.model.accept().unwrap();
        })
        .unwrap();
    fixture.handle.close().unwrap();
    for expected in [None, Some(path)] {
        let message = messages.try_recv().unwrap();
        let origin = message.origin.widget::<IFileDialogAbi>().unwrap().unwrap();
        assert!(origin.same_object(fixture.handle.as_interface()));
        assert!(!origin.is_alive());
        assert_eq!(completion(&message.event).unwrap(), expected);
    }
    assert!(messages.try_recv().is_err());
}

/// Save suggestions retain exact paths, accept new names with Enter, and do not leak into later Open requests.
#[test]
fn save_request_preserves_suggested_filename_and_rejects_directory_components() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&mut fixture);
    let mut request = neocogi_ui::FileDialogRequest::new(directory.path());
    request.mode = FileDialogMode::Save;
    request.title = "Save theme".into();
    request.filename = "../outside.json".into();
    assert_eq!(fixture.handle.open(&request), Err(NC_E_INVALID_ARG));
    assert!(!fixture.handle.active().unwrap());
    request.filename = "new theme.json".into();
    fixture.handle.open(&request).unwrap();
    assert_eq!(fixture.widget.with(|dialog| dialog.model.accept.label().unwrap()).unwrap(), "Save");
    assert_eq!(fixture.widget.with(|dialog| dialog.model.filename.text().unwrap()).unwrap(), "new theme.json");
    // Traverse the real modal and submit its suggested filename, without requiring
    // an existing file or manufacturing a completion in the test.
    fixture.tabs(6);
    fixture.key(KeyCode::Enter, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(events.try_recv().unwrap(), Some(directory.path().join("new theme.json")));
    assert!(!fixture.handle.active().unwrap());
    fixture.open(directory.path());
    assert_eq!(fixture.widget.with(|dialog| dialog.model.accept.label().unwrap()).unwrap(), "Open");
    assert_eq!(fixture.widget.with(|dialog| dialog.model.filename.text().unwrap()).unwrap(), "");
    assert!(fixture.widget.with(|dialog| dialog.model.selected.is_none()).unwrap());
}

/// Failed initial reads and overlapping requests leave activity, native directory and actual dynamic roots unchanged.
#[test]
fn failed_open_and_overlapping_requests_preserve_previous_state_and_expose_native_diagnostics() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("present"), "file").unwrap();
    let original = fixture.file_root();
    let missing = directory.path().join("missing");
    assert_eq!(fixture.handle.open(&neocogi_ui::FileDialogRequest::new(&missing)), Err(UI_E_IO));
    assert!(!fixture.handle.active().unwrap());
    assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(fixture.file_root().as_raw().object, original.as_raw().object);
    assert!(fixture.handle.diagnostic().unwrap().contains("missing"));
    fixture.open(directory.path());
    let live = fixture.file_root();
    assert_ne!(live.as_raw().object, original.as_raw().object);
    assert_eq!(fixture.handle.open(&neocogi_ui::FileDialogRequest::new(&missing)), Err(UI_E_BUSY));
    assert_eq!(fixture.file_root().as_raw().object, live.as_raw().object);
    assert!(fixture.handle.diagnostic().unwrap().is_empty());
    assert_eq!(fixture.widget.with(|dialog| dialog.model.directory.clone()).unwrap(), directory.path());
    fixture.handle.close().unwrap();
    assert_eq!(fixture.handle.active(), Err(neocogi_com::NC_E_DROPPED));
}

/// A dangling entry fails the complete candidate instead of publishing a partial directory.
#[cfg(unix)]
#[test]
fn broken_entry_rejects_the_complete_directory_candidate() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("readable"), b"ok").unwrap();
    std::os::unix::fs::symlink("missing-target", directory.path().join("broken-link")).unwrap();
    let previous = fixture.file_root();
    assert_eq!(fixture.handle.open(&neocogi_ui::FileDialogRequest::new(directory.path())), Err(UI_E_IO));
    assert_eq!(fixture.file_root().as_raw().object, previous.as_raw().object);
    assert_eq!(fixture.handle.active(), Ok(false));
}

/// Empty acceptance stays pending; separate picker capabilities retain independent request queues.
#[test]
fn empty_acceptance_and_independent_picker_requests_use_actual_controls() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&mut fixture);
    let app = TestApp::new(Vec::new(), events, |outcomes, message| {
        outcomes.push(message);
        Ok(())
    });
    fixture.open(directory.path());
    // Up, Home, Path, Go, Parent, Filename, Cancel, Open: activate Open with an empty filename.
    fixture.tabs(8);
    fixture.key(KeyCode::Space, 0);
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(fixture.handle.active(), Ok(true));
    assert!(app.state().is_empty());
    assert_eq!(fixture.handle.cancel(), Ok(true));
    assert_eq!(fixture.handle.cancel(), Ok(false));

    let (second_events_tx, second_events) = std::sync::mpsc::channel();
    let mut second = fixture
        .ui
        .file_dialog_with_queue(
            fixture.owner.as_com_ref(),
            Some(
                neocogi_ui::test_event_queue!(
                    neocogi_ui::IFileDialogAbi,
                    Option<PathBuf>,
                    second_events_tx.clone(),
                    |_queue, _origin, event| completion(event)?
                )
                .unwrap()
                .as_com_ref(),
            ),
        )
        .unwrap();
    let mut second_outcomes = Vec::new();

    second.open(&neocogi_ui::FileDialogRequest::new(directory.path())).unwrap();
    second.cancel().unwrap();
    app.update(&fixture.ui, SIZE).unwrap();
    for message in second_events.try_iter() {
        second_outcomes.push(message);
    }
    assert_eq!(&*app.state(), &[None]);
    assert_eq!(&*second_outcomes, &[None]);
    second.close().unwrap();
}

/// Actual keyboard text entry preserves leading/trailing spaces and completes in the same runtime transaction.
#[test]
fn typed_filename_acceptance_uses_real_controls_and_completes_before_update_returns() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    // Only the parent folder row is focusable: Up, Home, Path, Go, Parent, Filename, Cancel, Open.
    let events = session(&mut fixture);
    let app = TestApp::new(Vec::new(), events, |outcomes, message| {
        outcomes.push(message);
        Ok(())
    });
    fixture.open(directory.path());
    fixture.tabs(6);
    fixture.ui.input(UiInput::Text, " file name ").unwrap();
    fixture.tabs(2);
    fixture.key(KeyCode::Space, 0);
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(&*app.state(), &[Some(directory.path().join(" file name "))]);
    assert_eq!(fixture.handle.active(), Ok(false));
    assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(fixture.widget.with(|dialog| dialog.model.filename.text().unwrap()).unwrap(), " file name ");
    assert_eq!(fixture.handle.cancel(), Ok(false));
    fixture.handle.close().unwrap();
}

/// Native row data survives identical replacement-character display labels, including the same-transaction Open action.
#[cfg(unix)]
#[test]
fn selected_non_utf8_file_preserves_exact_native_identity_across_display_collisions() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join(OsString::from_vec(b"file-\x80".to_vec()));
    let second = directory.path().join(OsString::from_vec(b"file-\x81".to_vec()));
    std::fs::write(&first, "first").unwrap();
    std::fs::write(&second, "second").unwrap();
    assert_eq!(first.to_string_lossy(), second.to_string_lossy());
    let events = session(&mut fixture);
    let app = TestApp::new(Vec::new(), events, |outcomes, message| {
        outcomes.push(message);
        Ok(())
    });
    fixture.open(directory.path());
    // Four toolbar stops, parent folder, then the first exact native file row.
    fixture.tabs(6);
    fixture.key(KeyCode::Space, 0);
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(fixture.widget.with(|dialog| dialog.model.selected.clone()).unwrap().as_ref(), Some(&first));
    // Second file, filename, Cancel, Open.
    fixture.tabs(4);
    fixture.key(KeyCode::Space, 0);
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(&*app.state(), &[Some(first)]);
    fixture.handle.close().unwrap();
}

/// Failed navigation retains the exact selected path and both live columns instead of publishing a partial listing.
#[test]
fn unreadable_navigation_preserves_selection_directory_and_dynamic_widget_identity() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    let file = directory.path().join("file");
    std::fs::write(&file, "data").unwrap();
    fixture.open(directory.path());
    fixture.tabs(7); // Four toolbar stops, parent folder, child folder, file.
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.widget.with(|dialog| dialog.model.selected.clone()).unwrap().as_ref(), Some(&file));
    let root = fixture.file_root();
    std::fs::remove_dir(&folder).unwrap();
    fixture.key(KeyCode::Tab, 4); // Shift+Tab returns to the now-unreadable retained folder row.
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.file_root().as_raw().object, root.as_raw().object);
    fixture
        .widget
        .with(|dialog| {
            let model = &dialog.model;
            assert_eq!(model.selected.as_ref(), Some(&file));
            assert_eq!(model.directory, directory.path());
            assert!(model.error.text().unwrap().contains("Cannot read"));
        })
        .unwrap();
    fixture.handle.close().unwrap();
}

/// Explicit cancel/reopen ignores an old hide notification; owner hide and destruction settle exactly one active request.
#[test]
fn lifecycle_cancellation_is_once_per_request_and_stale_hide_does_not_cancel_reopening() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&mut fixture);
    let app = TestApp::new(Vec::new(), events, |outcomes, message| {
        outcomes.push(message);
        Ok(())
    });
    fixture.open(directory.path());
    assert_eq!(fixture.handle.cancel(), Ok(true));
    fixture.open(directory.path());
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(&*app.state(), &[None]);
    assert_eq!(fixture.handle.active(), Ok(true));
    fixture.owner.set_visibility(Visibility::Hidden).unwrap();
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(app.state().len(), 2);
    assert_eq!(fixture.handle.active(), Ok(false));
    fixture.owner.set_visibility(Visibility::Visible).unwrap();
    fixture.open(directory.path());
    fixture.owner.close().unwrap();
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(app.state().len(), 2);
    assert_eq!(fixture.handle.active(), Err(neocogi_com::NC_E_DROPPED));
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(app.state().len(), 2);
}

/// Final runtime release destroys the picker without synthesizing another completion.
#[test]
fn runtime_release_destroys_active_picker_without_posting_from_an_expired_origin() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let events = session(&mut fixture);
    let mut outcomes = Vec::new();
    fixture.open(directory.path());
    let host = fixture.runtime.construction_host().unwrap().host().unwrap();
    let handle = fixture.handle.clone();
    drop(fixture);
    assert_eq!(host.check_open(), Err(UI_E_CLOSED));
    assert!(outcomes.is_empty());
    assert_eq!(handle.active(), Err(neocogi_com::NC_E_DROPPED));
    for message in events.try_iter() {
        outcomes.push(message);
    }
    assert!(outcomes.is_empty(), "destruction does not synthesize a completion from an expired widget");
    assert!(events.try_recv().is_err());
}

/// Successful folder navigation replaces only dynamic rows, keeps controls/scroll models and clamps against new content.
#[test]
fn successful_navigation_reuses_fixed_controls_and_replaces_the_complete_dynamic_listing() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let child = directory.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(child.join("new file"), "data").unwrap();
    for index in 0..60 {
        std::fs::write(directory.path().join(format!("old-{index:02}")), "data").unwrap();
    }
    fixture.open(directory.path());
    let old_root = fixture.file_root();
    let path = fixture.widget.with(|dialog| dialog.model.path.clone()).unwrap();
    let mut scroll = fixture.widget.with(|dialog| dialog.model.files.clone()).unwrap();
    scroll.set_offset(Point { x: 0, y: i32::MAX }).unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert!(scroll.info().unwrap().requested.y > 0);
    fixture.tabs(6); // Four toolbar stops, parent folder, then the child directory.
    fixture.key(KeyCode::Space, 0);
    fixture.ui.update(SIZE).unwrap();
    assert_ne!(fixture.file_root().as_raw().object, old_root.as_raw().object);
    assert_eq!(fixture.widget.with(|dialog| dialog.model.directory.clone()).unwrap(), child);
    assert_eq!(path.text().unwrap(), child.to_string_lossy());
    assert_eq!(scroll.info().unwrap().requested.y, 0);
    assert!(fixture.widget.with(|dialog| dialog.model.selected.is_none()).unwrap());
    fixture.handle.close().unwrap();
}

/// A popup-opening app callback exposes the new modal to later already-queued native input in the same update.
#[test]
fn application_callback_opens_picker_and_later_input_reaches_its_new_modal_scope() {
    let directory = tempfile::tempdir().unwrap();
    // A separate activator lets the picker retain its original ordinary owner and tests cross-window modal routing.
    let (events_tx, events) = std::sync::mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::IFileDialogAbi, AppMessage, events_tx.clone(), |_queue, _origin, event| {
        AppMessage::Completed(completion(event)?)
    })
    .unwrap();
    let mut fixture = Fixture::with_queue(queue);
    let button = fixture
        .ui
        .widgets()
        .button_with_queue(
            "open picker",
            Some(
                neocogi_ui::test_event_queue!(neocogi_ui::ButtonAbi, AppMessage, events_tx.clone(), |_queue, _origin, _| { AppMessage::Open })
                    .unwrap()
                    .as_com_ref(),
            ),
        )
        .unwrap();
    let activator = fixture.ui.window("activator", OWNER_RECT, button).unwrap();
    /// Both request activation and completion share the application's FIFO.
    enum AppMessage {
        /// Requests that the reusable dialog become visible.
        Open,
        /// Captured native completion.
        Completed(Option<PathBuf>),
    }

    let app = TestApp::new(
        (fixture.handle.clone(), directory.path().to_owned(), Vec::new()),
        events,
        |(picker, directory, outcomes), message| match message {
            AppMessage::Open => picker.open(&neocogi_ui::FileDialogRequest::new(directory)),
            AppMessage::Completed(event) => {
                outcomes.push(event);
                Ok(())
            }
        },
    );
    fixture.tabs(1);
    fixture.key(KeyCode::Space, 0);
    fixture.tabs(6);
    fixture.ui.input(UiInput::Text, "after callback").unwrap();
    fixture.tabs(2);
    fixture.key(KeyCode::Space, 0);
    app.update(&fixture.ui, SIZE).unwrap();
    assert_eq!(&app.state().2, &[Some(directory.path().join("after callback"))]);
    assert_eq!(fixture.handle.active(), Ok(false));
    fixture.handle.close().unwrap();
    activator.close().unwrap();
}

/// Request completion is a committed fact even after the closed widget is retired during an active delivery window.
#[test]
fn closed_picker_preserves_all_completed_requests_during_manual_handler_runtime_reentry() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let ui = crate::bind_builtin_nodes(fixture.runtime.clone()).unwrap();
    let events = session(&mut fixture);
    let mut completions = 0;
    for _ in 0..2 {
        fixture.open(directory.path());
        fixture.handle.cancel().unwrap();
    }
    fixture.handle.close().unwrap();
    for message in events.try_iter() {
        assert!(message.is_none());
        completions += 1;
        ui.update(SIZE).unwrap();
    }
    assert_eq!(completions, 2);
    assert!(events.try_recv().is_err());
}

/// A foreign mapper's reentrant observation reports a checked borrow conflict, never panics.
#[test]
fn completion_mapping_reentry_uses_the_widget_com_gate() {
    let directory = tempfile::tempdir().unwrap();
    let (events_tx, events) = std::sync::mpsc::channel();
    let queue = neocogi_ui::test_event_queue!(neocogi_ui::IFileDialogAbi, Option<PathBuf>, events_tx, |_queue, origin, message| {
        assert_eq!(origin.active(), Err(neocogi_com::NC_E_BUSY));
        completion(message)?
    })
    .unwrap();
    let mut fixture = Fixture::with_queue(queue);
    fixture.open(directory.path());
    assert_eq!(fixture.handle.cancel(), Ok(true));
    assert!(matches!(events.try_recv().ok(), Some(None)));
    // Closing releases the queue; the receiver never retains the producer.
    fixture.handle.close().unwrap();
}

/// Dropping client projections leaves a mounted owner alive until its view is removed.
#[test]
fn mounted_dialog_outlives_client_handles_and_retires_with_its_view() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let messages = session(&mut fixture);
    fixture.open(directory.path());
    let window = fixture.handle.window().unwrap();
    let Fixture { ui, runtime, owner, handle, widget, .. } = fixture;
    drop(handle);
    drop(widget);
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Visible);
    ui.update(SIZE).unwrap();
    assert!(messages.try_recv().is_err());
    owner.close().unwrap();
    ui.update(SIZE).unwrap();
    assert!(matches!(messages.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)));
    assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
    drop(runtime);
}

/// All private producer kinds project the same queue object, and retired rows
/// cannot become commands merely because a client still retains their COM value.
#[test]
fn shared_child_queue_discards_events_from_replaced_directory_rows() {
    use neocogi_com::NcValueRef;
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let replacement = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("old.txt"), b"old").unwrap();
    fixture.open(directory.path());
    let (mut queue, old) = fixture
        .widget
        .with(|dialog| {
            let queue = dialog.model.nodes.queue.query::<neocogi_ui::events::IEventReceiverAbi>().unwrap();
            let buttons = dialog.model.nodes.queue.query::<neocogi_ui::events::IEventReceiverAbi>().unwrap();
            let text = dialog.model.nodes.queue.query::<neocogi_ui::events::IEventReceiverAbi>().unwrap();
            assert!(queue.as_com_ref().same_object(buttons.as_com_ref()));
            assert!(queue.as_com_ref().same_object(text.as_com_ref()));
            let old = dialog
                .model
                .rows
                .iter()
                .find(|(_, action)| matches!(action, Command::File(_)))
                .unwrap()
                .0
                .clone();
            (queue, old)
        })
        .unwrap();
    // Post through the real generated ABI before replacing the row's membership.
    let event = neocogi_ui::events::activated().unwrap();
    queue
        .post(NcValueRef::new(&EventOrigin::Widget(old.clone().into_unknown())).unwrap(), event)
        .unwrap();
    fixture
        .widget
        .with_mut(|dialog| {
            let listing = dialog.model.candidate(replacement.path()).unwrap();
            dialog.model.install(replacement.path().to_owned(), listing, true).unwrap();
            while IWidgetDispatcher::dispatch_next(&mut *dialog).unwrap() {}
            assert!(dialog.model.selected.is_none());
            assert_eq!(dialog.model.filename.text().unwrap(), "");
        })
        .unwrap();
    // The old row has been destroyed; queued facts retain only its weak identity.
    assert!(!old.is_alive());
}

/// Refuses cancellation delivery through a real generated COM entry.
#[com_object(interfaces(neocogi_ui::events::IEventReceiver))]
struct RefusingCompletion;
impl neocogi_ui::events::IEventReceiver for RefusingCompletion {
    /// Distinguishes delivery refusal from the window's temporary busy status.
    fn post(
        &mut self,
        _origin: NcValueRef<'_, EventOrigin>,
        event: neocogi_com::ComBox<neocogi_ui::events::IEventAbi>,
    ) -> Result<(), (NcStatus, neocogi_com::ComBox<neocogi_ui::events::IEventAbi>)> {
        Err((NC_E_INVALID_ARG, event))
    }
}

/// One retryable window operation rejected before its real mutation.
#[derive(Clone, Copy, Eq, PartialEq)]
enum WindowOperation {
    /// Refuse observation without refusing the actual visibility operation.
    Snapshot,
    /// Refuse the first hide, before the request may commit Idle.
    Hide,
    /// Refuse the first removal, after cancellation has committed.
    Remove,
}

/// Delegates to the real modal while refusing one selected operation once.
#[com_object(interfaces(IWindow))]
struct RefusingWindow {
    /// The original window remains the actual owner of the picker tree.
    window: ComPtr<IWindowAbi>,
    /// Selects the failure boundary without creating a second window implementation.
    operation: WindowOperation,
    /// Distinct injected statuses make cleanup error precedence observable.
    status: NcStatus,
    /// Shared IWindow methods require synchronized counting across COM projections.
    attempts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
impl RefusingWindow {
    /// Refuses only the selected operation's first attempt; all other calls delegate.
    fn refuse(&self, operation: WindowOperation) -> bool {
        self.operation == operation && self.attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0
    }
}
impl IWindow for RefusingWindow {
    /// Preserves observation of the real modal after refused removal.
    fn snapshot(&self) -> Result<WindowSnapshot, NcStatus> {
        if self.refuse(WindowOperation::Snapshot) {
            Err(self.status)
        } else {
            self.window.snapshot()
        }
    }
    /// Preserves title observation.
    fn title(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        self.window.title()
    }
    /// Delegates ordinary title editing.
    fn set_title(&self, title: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.window.set_title(title)
    }
    /// A rejected hide leaves the real modal visible and the request Active.
    fn set_visibility(&self, visibility: Visibility) -> Result<(), NcStatus> {
        if visibility == Visibility::Hidden && self.refuse(WindowOperation::Hide) {
            Err(self.status)
        } else {
            self.window.set_visibility(visibility)
        }
    }
    /// Delegates ordinary presentation changes.
    fn set_options(&self, options: WindowOptions) -> Result<(), NcStatus> {
        self.window.set_options(options)
    }
    /// Delegates layer validation to the real modal.
    fn set_layer(&self, layer: u8) -> Result<(), NcStatus> {
        self.window.set_layer(layer)
    }
    /// Preserves normal stacking behavior.
    fn raise(&self) -> Result<(), NcStatus> {
        self.window.raise()
    }
    /// Preserves parent-owned clipping behavior.
    fn set_child_clip(&self, clip: ChildClip) -> Result<(), NcStatus> {
        self.window.set_child_clip(clip)
    }
    /// Delegates normal allocation changes.
    fn set_rect(&self, rect: Rect) -> Result<(), NcStatus> {
        self.window.set_rect(rect)
    }
    /// The first refusal preserves ownership; the next call removes it normally.
    fn close(&self) -> Result<(), NcStatus> {
        if self.refuse(WindowOperation::Remove) {
            Err(self.status)
        } else {
            self.window.close()
        }
    }
}

/// Installs a temporary failure boundary without replacing the actual modal tree.
fn refuse_first(fixture: &Fixture, operation: WindowOperation) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    refuse_first_with_status(fixture, operation, UI_E_BUSY)
}

/// Allows nested proxies to refuse hide and removal with distinguishable failures.
fn refuse_first_with_status(fixture: &Fixture, operation: WindowOperation, status: NcStatus) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let window = fixture.handle.window().unwrap();
    let proxy = RefusingWindow::from_value(RefusingWindow {
        window,
        operation,
        status,
        attempts: attempts.clone(),
    })
    .query()
    .unwrap();
    fixture.widget.clone().with_mut(|dialog| dialog.model.window = Some(proxy)).unwrap();
    attempts
}

/// Hide refusal is pre-commit: both acceptance and cancellation leave the active
/// request and visible modal intact, then retry with one eventual completion.
#[test]
fn failed_hide_preserves_the_active_request_until_completion_can_commit() {
    for accept in [false, true] {
        let mut fixture = Fixture::new();
        let directory = tempfile::tempdir().unwrap();
        let messages = session(&mut fixture);
        fixture.open(directory.path());
        let path = directory.path().join("selected");
        fixture.widget.with_mut(|dialog| dialog.model.selected = Some(path.clone())).unwrap();
        let attempts = refuse_first(&fixture, WindowOperation::Hide);
        let complete = |fixture: &mut Fixture| {
            if accept {
                fixture.widget.with_mut(|dialog| dialog.model.accept()).unwrap()
            } else {
                fixture.handle.cancel().map(|_| ())
            }
        };
        assert_eq!(complete(&mut fixture), Err(UI_E_BUSY));
        assert_eq!(fixture.handle.active(), Ok(true));
        assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Visible);
        assert!(messages.try_recv().is_err());
        assert_eq!(complete(&mut fixture), Ok(()));
        assert_eq!(fixture.handle.active(), Ok(false));
        assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Hidden);
        assert_eq!(messages.try_recv().unwrap(), accept.then_some(path));
        assert!(messages.try_recv().is_err());
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
        fixture.handle.close().unwrap();
    }
}

/// Observation failure is irrelevant when the real hide operation succeeds.
#[test]
fn completion_does_not_query_a_snapshot_before_hiding() {
    for accept in [false, true] {
        let mut fixture = Fixture::new();
        let directory = tempfile::tempdir().unwrap();
        let messages = session(&mut fixture);
        fixture.open(directory.path());
        let modal = fixture.handle.window().unwrap();
        let path = directory.path().join("exact selection");
        fixture.widget.with_mut(|dialog| dialog.model.selected = Some(path.clone())).unwrap();
        let attempts = refuse_first(&fixture, WindowOperation::Snapshot);
        if accept {
            fixture.widget.with_mut(|dialog| dialog.model.accept()).unwrap().unwrap();
        } else {
            assert_eq!(fixture.handle.cancel(), Ok(true));
        }
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(fixture.handle.active(), Ok(false));
        assert_eq!(modal.snapshot().unwrap().visibility, Visibility::Hidden);
        assert_eq!(messages.try_recv().unwrap(), accept.then_some(path));
        assert!(messages.try_recv().is_err());
        fixture.handle.close().unwrap();
    }
}

/// A close that cannot hide or remove has not committed cancellation; removal's error wins.
#[test]
fn close_preserves_active_when_hide_and_removal_both_refuse() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let messages = session(&mut fixture);
    fixture.open(directory.path());
    let modal = fixture.handle.window().unwrap();
    let hide = refuse_first(&fixture, WindowOperation::Hide);
    let remove = refuse_first_with_status(&fixture, WindowOperation::Remove, NC_E_INVALID_ARG);
    assert_eq!(fixture.handle.close(), Err(NC_E_INVALID_ARG));
    assert_eq!(fixture.handle.active(), Ok(true));
    assert_eq!(modal.snapshot().unwrap().visibility, Visibility::Visible);
    assert!(messages.try_recv().is_err());
    assert_eq!(hide.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(remove.load(std::sync::atomic::Ordering::SeqCst), 1);
    fixture.handle.close().unwrap();
    assert!(!fixture.handle.as_interface().is_alive());
    assert_eq!(messages.try_recv(), Ok(None));
    assert!(messages.try_recv().is_err());
}

/// Delivery refusal is post-commit: it leaves an idle reusable modal, and an Idle
/// retry cannot post again. Both accepted and cancelled completions obey this rule.
#[test]
fn refused_completion_stays_idle_hidden_and_reusable() {
    for accept in [false, true] {
        let queue = RefusingCompletion::from_value(RefusingCompletion).query().unwrap();
        let mut fixture = Fixture::with_queue(queue);
        let directory = tempfile::tempdir().unwrap();
        fixture.open(directory.path());
        fixture
            .widget
            .with_mut(|dialog| dialog.model.selected = Some(directory.path().join("selected")))
            .unwrap();
        let result = if accept {
            fixture.widget.with_mut(|dialog| dialog.model.accept()).unwrap()
        } else {
            fixture.handle.cancel().map(|_| ())
        };
        assert_eq!(result, Err(NC_E_INVALID_ARG));
        assert_eq!(fixture.handle.active(), Ok(false));
        assert_eq!(fixture.handle.window().unwrap().snapshot().unwrap().visibility, Visibility::Hidden);
        assert_eq!(fixture.handle.cancel(), Ok(false));
        fixture.open(directory.path());
        assert_eq!(fixture.handle.active(), Ok(true));
        assert_eq!(fixture.handle.cancel(), Err(NC_E_INVALID_ARG));
        assert_eq!(fixture.handle.cancel(), Ok(false));
        fixture.handle.close().unwrap();
    }
}

/// Delivery failure must not strand a terminal request inside a still-visible modal.
#[test]
fn close_removes_dialog_even_when_cancellation_queue_refuses_delivery() {
    let queue = RefusingCompletion::from_value(RefusingCompletion).query().unwrap();
    let mut fixture = Fixture::with_queue(queue);
    let directory = tempfile::tempdir().unwrap();
    fixture.open(directory.path());
    let window = fixture.handle.window().unwrap();
    assert_eq!(fixture.handle.close(), Err(NC_E_INVALID_ARG));
    assert!(!fixture.handle.as_interface().is_alive());
    assert_eq!(window.snapshot(), Err(UI_E_CLOSED));
    assert_eq!(fixture.handle.active(), Err(neocogi_com::NC_E_DROPPED));
}

/// A pre-commit removal refusal is retryable and does not duplicate cancellation.
#[test]
fn close_retries_refused_removal_without_reposting_the_completed_request() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    fixture.open(directory.path());
    let events = session(&mut fixture);
    let attempts = refuse_first(&fixture, WindowOperation::Remove);
    assert_eq!(fixture.handle.close(), Err(UI_E_BUSY));
    assert_eq!(fixture.handle.active(), Ok(false));
    assert!(fixture.handle.as_interface().is_alive());
    assert!(events.try_recv().unwrap().is_none());
    assert_eq!(fixture.handle.close(), Ok(()));
    assert!(!fixture.handle.as_interface().is_alive());
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(events.try_recv().is_err());
}

/// Failure to remove takes priority over delivery failure, preserving an actionable retry.
#[test]
fn close_reports_removal_failure_when_both_cleanup_steps_fail() {
    let queue = RefusingCompletion::from_value(RefusingCompletion).query().unwrap();
    let mut fixture = Fixture::with_queue(queue);
    let directory = tempfile::tempdir().unwrap();
    fixture.open(directory.path());
    let attempts = refuse_first(&fixture, WindowOperation::Remove);
    assert_eq!(fixture.handle.close(), Err(UI_E_BUSY));
    assert_eq!(fixture.handle.active(), Ok(false));
    assert_eq!(fixture.handle.close(), Ok(()));
    assert!(!fixture.handle.as_interface().is_alive());
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
}

/// Reopening starts a fresh interaction: old caption intent cannot cancel the
/// new request. A current intent cancels once while retaining the dialog widget.
#[test]
fn caption_intent_cancels_current_request_without_destroying_or_crossing_reopen() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    let completions = session(&mut fixture);
    fixture.open(directory.path());
    let window = fixture.handle.window().unwrap();
    let mut queue = fixture
        .widget
        .with(|dialog| dialog.model.nodes.queue.query::<neocogi_ui::events::IEventReceiverAbi>())
        .unwrap()
        .unwrap();
    queue
        .post(
            NcValueRef::new(&EventOrigin::Service(window.downgrade().unwrap().into_unknown())).unwrap(),
            neocogi_ui::events::window_close_requested().unwrap(),
        )
        .unwrap();
    assert_eq!(fixture.handle.cancel(), Ok(true));
    fixture.open(directory.path());
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.handle.active(), Ok(true), "old caption intent belongs to the superseded request");
    assert_eq!(completions.try_iter().count(), 1, "only explicit cancellation completed the old request");
    queue
        .post(
            NcValueRef::new(&EventOrigin::Service(window.downgrade().unwrap().into_unknown())).unwrap(),
            neocogi_ui::events::window_close_requested().unwrap(),
        )
        .unwrap();
    fixture.ui.update(SIZE).unwrap();
    assert_eq!(fixture.handle.active(), Ok(false));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Hidden);
    assert_eq!(completions.try_iter().count(), 1);
    assert!(
        fixture.handle.as_interface().is_alive(),
        "request cancellation does not destroy its reusable widget"
    );
    fixture.handle.close().unwrap();
    assert!(!fixture.handle.as_interface().is_alive());
}

#[com_object(interfaces(INativePath))]
/// A foreign path canary proves invalid titles reject before either decoder calls out.
struct TitlePathCanary {
    /// All path operations increment the same test-owned observation.
    calls: Rc<std::cell::Cell<usize>>,
}
impl TitlePathCanary {
    /// A normal provider error is distinguishable from title admission by its count.
    fn called(&self) -> NcStatus {
        self.calls.set(self.calls.get() + 1);
        NC_E_INVALID_ARG
    }
}
impl INativePath for TitlePathCanary {
    /// Decoder metadata must remain untouched for rejected title input.
    fn encoding(&self) -> Result<PathEncoding, NcStatus> {
        Err(self.called())
    }
    /// Rejects a unit count if the decoder unexpectedly proceeds.
    fn unit_count(&self) -> Result<u64, NcStatus> {
        Err(self.called())
    }
    /// Never fabricates native bytes for this ordering probe.
    fn unix_bytes(&self, _: neocogi_com::NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        Err(self.called())
    }
    /// Never fabricates native units for this ordering probe.
    fn windows_units(&self, _: neocogi_com::NcOutputWriter<'_, u16>) -> Result<(), NcStatus> {
        Err(self.called())
    }
    /// Display is also foreign work and must not run during title rejection.
    fn display(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Err(self.called())
    }
}

/// Title errors preserve an earlier completed request and reject before foreign work/publication.
#[test]
fn rejected_titles_preserve_modal_rows_selection_and_both_queues_before_path_calls() {
    let mut fixture = Fixture::new();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("present"), "file").unwrap();
    let completions = session(&mut fixture);
    let mut previous = neocogi_ui::FileDialogRequest::new(directory.path());
    previous.title = "retained title".into();
    previous.mode = FileDialogMode::Save;
    previous.filename = PathBuf::from("suggested.txt");
    fixture.handle.open(&previous).unwrap();
    assert_eq!(fixture.handle.cancel(), Ok(true));
    // Drain only setup's private lifecycle facts. The completed application result
    // remains queued throughout the rejected opens below.
    fixture.widget.with(|dialog| while dialog.messages.try_recv().is_ok() {}).unwrap();
    let window = fixture.handle.window().unwrap();
    let before = window.snapshot().unwrap();
    let title = window.title().unwrap().to_string().unwrap();
    let root = fixture.file_root();
    let selected = fixture.widget.with(|dialog| dialog.model.selected.clone()).unwrap();
    let calls = Rc::new(std::cell::Cell::new(0));
    let path: ComPtr<INativePathAbi> = TitlePathCanary::from_value(TitlePathCanary { calls: calls.clone() }).query().unwrap();
    let mut interface = fixture.handle.as_interface().clone();
    let exact = "é".repeat(neocogi_ui::MAX_WINDOW_TITLE_BYTES / 2);
    let cases = [
        (0, vec![b'x'; neocogi_ui::MAX_WINDOW_TITLE_BYTES + 1], neocogi_com::NC_E_OUT_OF_RANGE),
        (0, format!("{exact}x").into_bytes(), neocogi_com::NC_E_OUT_OF_RANGE),
        (0, vec![255; neocogi_ui::MAX_WINDOW_TITLE_BYTES + 1], neocogi_com::NC_E_OUT_OF_RANGE),
        (1, vec![255; neocogi_ui::MAX_WINDOW_TITLE_BYTES + 1], NC_E_INVALID_ARG),
        (0, vec![255], NC_E_INVALID_ARG),
        (1, b"valid".to_vec(), NC_E_INVALID_ARG),
    ];
    let mut queue = fixture
        .widget
        .with(|dialog| dialog.model.nodes.queue.query::<neocogi_ui::events::IEventReceiverAbi>())
        .unwrap()
        .unwrap();
    for (offset, bytes, expected) in cases {
        queue
            .post(
                NcValueRef::new(&EventOrigin::Service(window.downgrade().unwrap().into_unknown())).unwrap(),
                neocogi_ui::events::window_close_requested().unwrap(),
            )
            .unwrap();
        assert_eq!(
            interface.open(
                NcCopyFromBufferOffset::new(offset, &bytes),
                previous.rect,
                path.as_com_ref(),
                FileDialogMode::Open,
                path.as_com_ref()
            ),
            Err(expected)
        );
        assert_eq!(calls.get(), 0, "neither native-path decoder may run for an invalid title");
        let after = window.snapshot().unwrap();
        assert_eq!(after.visibility, Visibility::Hidden);
        assert_eq!(
            (after.rect.x, after.rect.y, after.rect.width, after.rect.height),
            (before.rect.x, before.rect.y, before.rect.width, before.rect.height)
        );
        assert_eq!(window.title().unwrap().to_string().unwrap(), title);
        assert_eq!(fixture.file_root().as_raw().object, root.as_raw().object);
        assert_eq!(fixture.widget.with(|dialog| dialog.model.selected.clone()).unwrap(), selected);
        assert_eq!(fixture.handle.active(), Ok(false));
        assert_eq!(fixture.handle.cancel(), Ok(false));
        assert!(
            fixture.widget.with(|dialog| dialog.messages.try_recv().is_ok()).unwrap(),
            "failed open drained the private inbox"
        );
        assert!(fixture.widget.with(|dialog| dialog.messages.try_recv().is_err()).unwrap());
    }
    assert_eq!(completions.try_recv().unwrap(), None, "the prior completed request remains observable");
    assert!(completions.try_recv().is_err(), "rejected opens and Idle cancel create no completion");
    assert_eq!(
        interface.open(
            NcCopyFromBufferOffset::new(0, b"valid"),
            previous.rect,
            path.as_com_ref(),
            FileDialogMode::Open,
            path.as_com_ref()
        ),
        Err(NC_E_INVALID_ARG)
    );
    assert_eq!(calls.get(), 1, "the canary really observes a decoder after valid title admission");
    let mut corrected = neocogi_ui::FileDialogRequest::new(directory.path());
    corrected.title = exact;
    fixture.handle.open(&corrected).unwrap();
    assert_eq!(window.title().unwrap().byte_len().unwrap(), neocogi_ui::MAX_WINDOW_TITLE_BYTES as u64);
    assert_eq!(fixture.handle.active(), Ok(true));
    assert_eq!(window.snapshot().unwrap().visibility, Visibility::Visible);
    assert_eq!(fixture.handle.cancel(), Ok(true));
}
