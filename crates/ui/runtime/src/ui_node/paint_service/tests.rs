//! Real generated painter calls prove the runtime's private slot/revocation boundary.
use super::*;
use neocogi_render_gpu::{command::Command, GpuDevice};

/// Installs ordinary imported UI assets, identical to the non-GPU painter tests.
fn assets() -> super::super::SharedAssets {
    let bundle = crate::SkinBundle::from_atlas(crate::test_support::test_atlas());
    let source = crate::test_support::assets::export_bundle(&bundle).unwrap();
    let imported = crate::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
    std::rc::Rc::new(std::cell::RefCell::new(std::rc::Rc::new(imported)))
}

/// A wide allocation has only its right 30-by-40 area visible in the committed client.
fn recorder(frames: std::rc::Rc<gpu::Frames>, invocation: u64) -> neocogi_com::LocalCom<Recorder> {
    let mut value = Recorder::new(assets());
    value.frames = frames;
    value.next = invocation - 1;
    value
        .begin(PaintContext {
            display_list: crate::render::DisplayList::new(),
            content_rect: crate::Recti::new(100, 200, 80, 60),
            screen_clip: crate::Recti::new(150, 220, 30, 40),
            view: None,
            gpu_error: None,
        })
        .unwrap();
    Recorder::into_local(value)
}
/// Caller-provided broad geometry cannot redefine the visible-client root.
fn scope(invocation: u64) -> PaintScope {
    PaintScope {
        invocation,
        clip: Rect { x: 0, y: 0, width: 1000, height: 1000 },
    }
}

/// Reserve order at begin, preserve trusted visible fitting, and revoke an escaped finished facade.
#[test]
fn committed_views_keep_their_original_position_and_visible_client_root() {
    let device = GpuDevice::new();
    let domain = device.create_client(RenderClientLimits::default()).unwrap();
    let resources = domain.resources().unwrap();
    let frames = std::rc::Rc::new(gpu::Frames::default());
    frames.install(device.broker().as_com_ref()).unwrap();
    let frame = frames.begin().unwrap();
    let local = recorder(frames, 1);
    let mut painter: ComPtr<IPainterAbi> = local.query().unwrap();
    let mut view = painter.begin_render_view(scope(1), resources.as_com_ref()).unwrap();
    let info = view.info().unwrap();
    assert_eq!(info.client, Rect { x: 50, y: 20, width: 30, height: 40 });
    assert_eq!(info.viewport, Rect { x: 0, y: 0, width: 30, height: 40 });
    // Ordinary output occurs before finish but remains after the position reserved at begin.
    painter.fill(scope(1), Rect { x: 50, y: 20, width: 30, height: 40 }, 0xff112233).unwrap();
    view.clear_color(0xff556677).unwrap();
    view.finish().unwrap();
    local.try_borrow_mut().unwrap().harvest().unwrap();
    let context = local.try_borrow_mut().unwrap().finish(1).unwrap();
    assert_eq!(context.display_list.ops.len(), 2);
    let crate::render::display_list::DrawKind::GpuView { block: Some(ref owner) } = context.display_list.ops[0].kind else {
        panic!("view reordered");
    };
    let block = device.block_snapshot(owner.as_com_ref()).unwrap();
    assert_eq!(block.root.target, Rect { x: 150, y: 220, width: 30, height: 40 });
    assert!(matches!(
        block.commands[1],
        Command::ClearColor {
            rgba: 0xff556677,
            clip: Rect { x: 150, y: 220, width: 30, height: 40 },
            ..
        }
    ));
    assert_eq!(view.clear_color(0), Err(RENDER_E_EXPIRED));
    frame.finish().unwrap();
}

/// Unfinished work cannot disappear after a failed new begin; payload retires and error stays sticky.
#[test]
fn unhandled_view_failure_rejects_the_whole_invocation_even_if_the_widget_retries() {
    let device = GpuDevice::new();
    let domain = device.create_client(RenderClientLimits::default()).unwrap();
    let resources = domain.resources().unwrap();
    let frames = std::rc::Rc::new(gpu::Frames::default());
    frames.install(device.broker().as_com_ref()).unwrap();
    let frame = frames.begin().unwrap();
    let local = recorder(frames, 1);
    let mut painter: ComPtr<IPainterAbi> = local.query().unwrap();
    let mut escaped = painter.begin_render_view(scope(1), resources.as_com_ref()).unwrap();
    escaped.clear_color(0xff556677).unwrap();
    assert_eq!(painter.begin_render_view(scope(1), resources.as_com_ref()).err(), Some(RENDER_E_STATE));
    assert_eq!(painter.begin_render_view(scope(1), resources.as_com_ref()).err(), Some(RENDER_E_STATE));
    assert_eq!(local.try_borrow_mut().unwrap().harvest(), Err(RENDER_E_STATE));
    assert_eq!(escaped.clear_color(0), Err(RENDER_E_EXPIRED));
    assert_eq!(device.recording_usage(resources.as_com_ref()).unwrap().0, 0);
    drop(frame); // Abort is legal and must not submit a native frame.
}

/// Cancel is explicit abandonment, while a later token never revives a previous painter identity.
#[test]
fn cancelled_views_are_empty_and_retained_painters_cannot_join_a_later_widget() {
    let device = GpuDevice::new();
    let domain = device
        .create_client(RenderClientLimits {
            frame_draws: 2,
            ..RenderClientLimits::default()
        })
        .unwrap();
    let resources = domain.resources().unwrap();
    let frames = std::rc::Rc::new(gpu::Frames::default());
    frames.install(device.broker().as_com_ref()).unwrap();
    let frame = frames.begin().unwrap();
    let first = recorder(frames.clone(), 1);
    let mut old: ComPtr<IPainterAbi> = first.query().unwrap();
    let mut view = old.begin_render_view(scope(1), resources.as_com_ref()).unwrap();
    view.cancel().unwrap();
    first.try_borrow_mut().unwrap().harvest().unwrap();
    let context = first.try_borrow_mut().unwrap().finish(1).unwrap();
    assert!(matches!(
        context.display_list.ops[0].kind,
        crate::render::display_list::DrawKind::GpuView { block: None }
    ));
    let second = recorder(frames.clone(), 2);
    assert_eq!(old.begin_render_view(scope(2), resources.as_com_ref()).err(), Some(UI_E_EXPIRED_SCOPE));
    let mut next: ComPtr<IPainterAbi> = second.query().unwrap();
    let mut view = next.begin_render_view(scope(2), resources.as_com_ref()).unwrap();
    view.finish().unwrap();
    second.try_borrow_mut().unwrap().harvest().unwrap();
    second.try_borrow_mut().unwrap().finish(2).unwrap();
    let third = recorder(frames, 3);
    let mut exhausted: ComPtr<IPainterAbi> = third.query().unwrap();
    assert_eq!(
        exhausted.begin_render_view(scope(3), resources.as_com_ref()).err(),
        Some(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert!(third.try_borrow_mut().unwrap().finish(3).unwrap().display_list.is_empty());
    frame.finish().unwrap();
}
