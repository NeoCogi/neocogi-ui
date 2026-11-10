//! Real generated painter calls prove the runtime's private slot/revocation boundary.
use super::*;
use neocogi_platform::gpu::{command::Command};

/// Installs ordinary imported UI assets, identical to the non-GPU painter tests.
fn assets() -> super::super::SharedAssets {
    let bundle = crate::SkinBundle::from_atlas(crate::test_support::test_atlas());
    let source = crate::test_support::assets::export_bundle(&bundle).unwrap();
    let imported = crate::assets::ImportedAssets::read(source.as_com_ref(), None).unwrap();
    std::rc::Rc::new(std::cell::RefCell::new(std::rc::Rc::new(imported)))
}

/// A wide allocation has only its right 30-by-40 area visible in the committed client.
fn recorder(frames: std::rc::Rc<gpu::Frames>, invocation: u64) -> neocogi_com::LocalCom<Recorder> {
    let value = Recorder::new(
        assets(),
        frames,
        std::num::NonZeroU64::new(invocation).unwrap(),
        PaintContext {
            display_list: crate::render::DisplayList::new(),
            content_rect: crate::Recti::new(100, 200, 80, 60),
            screen_clip: crate::Recti::new(150, 220, 30, 40),
            view: None,
            gpu_error: None,
        },
    );
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
    let device = neocogi_platform::gpu::testing::device(
        neocogi_platform::render::compiler::CompilerObject::from_value(
            neocogi_platform::render::compiler::CompilerObject::new(neocogi_platform_contracts::render::shader::ShaderTarget::Wgsl).unwrap(),
        )
        .query()
        .unwrap(),
    );
    let domain = device
        .create_client(
            RenderClientLimits::default(),
            device.surface_target(RenderSurfaceKind::Framebuffer).unwrap(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = domain.resources().unwrap();
    let frames = std::rc::Rc::new(gpu::Frames::default());
    frames.install(device.broker());
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
    assert_eq!(block.root().target, Rect { x: 150, y: 220, width: 30, height: 40 });
    assert!(matches!(
        block.commands()[1],
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
    let device = neocogi_platform::gpu::testing::device(
        neocogi_platform::render::compiler::CompilerObject::from_value(
            neocogi_platform::render::compiler::CompilerObject::new(neocogi_platform_contracts::render::shader::ShaderTarget::Wgsl).unwrap(),
        )
        .query()
        .unwrap(),
    );
    let domain = device
        .create_client(
            RenderClientLimits::default(),
            device.surface_target(RenderSurfaceKind::Framebuffer).unwrap(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = domain.resources().unwrap();
    let frames = std::rc::Rc::new(gpu::Frames::default());
    frames.install(device.broker());
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

/// Client destruction releases its shell; only host harvesting decides whether work was finished.
#[test]
fn dropped_and_cancelled_rejections_remain_sticky_at_the_host_harvest_boundary() {
    for (reject, cancel) in [(false, false), (true, false), (true, true)] {
        let device = neocogi_platform::gpu::testing::device(
            neocogi_platform::render::compiler::CompilerObject::from_value(
                neocogi_platform::render::compiler::CompilerObject::new(neocogi_platform_contracts::render::shader::ShaderTarget::Wgsl).unwrap(),
            )
            .query()
            .unwrap(),
        );
        let domain = device
            .create_client(
                RenderClientLimits::default(),
                device.surface_target(RenderSurfaceKind::Framebuffer).unwrap(),
                RenderClientPermissions::default(),
            )
            .unwrap();
        let resources = domain.resources().unwrap();
        let frames = std::rc::Rc::new(gpu::Frames::default());
        frames.install(device.broker());
        let frame = frames.begin().unwrap();
        let local = recorder(frames, 1);
        let mut painter: ComPtr<IPainterAbi> = local.query().unwrap();
        let mut view = painter.begin_render_view(scope(1), resources.as_com_ref()).unwrap();
        view.clear_color(0xff112233).unwrap();
        if reject {
            assert_eq!(view.clear_depth(f32::NAN), Err(NC_E_INVALID_ARG));
        }
        if cancel {
            view.cancel().unwrap();
        }
        drop(view);
        let usage = device.recording_usage(resources.as_com_ref()).unwrap();
        assert_eq!(usage.1, 0);
        assert_eq!(usage.0 > 0, !reject);
        assert_eq!(local.try_borrow_mut().unwrap().harvest(), Err(RENDER_E_STATE));
        assert_eq!(device.recording_usage(resources.as_com_ref()).unwrap(), (0, 0));
        assert_eq!(painter.begin_render_view(scope(1), resources.as_com_ref()).err(), Some(RENDER_E_STATE));
        assert_eq!(local.try_borrow_mut().unwrap().harvest(), Err(RENDER_E_STATE));
        drop(frame);
    }
}

/// Cancel is explicit abandonment, while a later token never revives a previous painter identity.
#[test]
fn cancelled_views_are_empty_and_retained_painters_cannot_join_a_later_widget() {
    let device = neocogi_platform::gpu::testing::device(
        neocogi_platform::render::compiler::CompilerObject::from_value(
            neocogi_platform::render::compiler::CompilerObject::new(neocogi_platform_contracts::render::shader::ShaderTarget::Wgsl).unwrap(),
        )
        .query()
        .unwrap(),
    );
    let domain = device
        .create_client(
            RenderClientLimits {
                frame_draws: 2,
                ..RenderClientLimits::default()
            },
            device.surface_target(RenderSurfaceKind::Framebuffer).unwrap(),
            RenderClientPermissions::default(),
        )
        .unwrap();
    let resources = domain.resources().unwrap();
    let frames = std::rc::Rc::new(gpu::Frames::default());
    frames.install(device.broker());
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

/// Real COM calls distinguish rejected geometry from successful absence and share integer/fractional output.
#[test]
fn bounded_ui_geometry_rejects_before_publication_and_keeps_fractional_drawing() {
    let local = recorder(std::rc::Rc::default(), 1);
    let mut painter: ComPtr<IPainterAbi> = local.query().unwrap();
    let s = scope(1);
    let point = Point { x: 60, y: 30 };
    let far = Point { x: i32::MIN, y: 30 };
    assert_eq!(painter.stroke_line(s, far, point, 2, 0xffffffff), Err(neocogi_com::NC_E_OUT_OF_RANGE));
    assert_eq!(
        painter.stroke_line_f32(s, PaintPoint { x: 1e20, y: 30.0 }, PaintPoint { x: 60.0, y: 30.0 }, 2.0, 0xffffffff),
        Err(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(
        painter.fill_polygon(s, NcCopyFromBufferOffset::new(0, &[point, far, Point { x: 70, y: 35 }]), 0xffffffff),
        Err(neocogi_com::NC_E_OUT_OF_RANGE)
    );
    assert_eq!(painter.stroke_line(s, point, point, -1, 0xffffffff), Err(NC_E_INVALID_ARG));
    assert_eq!(local.try_borrow().unwrap().context.as_ref().unwrap().display_list.ops.len(), 0);
    painter.stroke_line(s, Point { x: 50, y: 30 }, Point { x: 70, y: 30 }, 2, 0xffffffff).unwrap();
    painter
        .stroke_line_f32(s, PaintPoint { x: 50.5, y: 35.0 }, PaintPoint { x: 70.5, y: 35.0 }, 1.5, 0xffffffff)
        .unwrap();
    painter
        .fill_polygon(
            s,
            NcCopyFromBufferOffset::new(0, &[Point { x: 50, y: 20 }, Point { x: 60, y: 20 }, Point { x: 50, y: 25 }]),
            0xffffffff,
        )
        .unwrap();
    let context = local.try_borrow_mut().unwrap().finish(1).unwrap();
    assert!(!context.display_list.ops.is_empty());
    let vertices = context.display_list.solid_geometry.triangles()[2..4].iter().flat_map(|t| t.vertices());
    let ys: Vec<_> = vertices.map(|v| v.position.y).collect();
    assert_eq!(
        ys.iter().copied().fold(f32::NEG_INFINITY, f32::max) - ys.iter().copied().fold(f32::INFINITY, f32::min),
        1.5
    );
    assert_eq!(
        painter.stroke_line(PaintScope { invocation: 1, ..s }, far, point, 2, 0),
        Err(UI_E_EXPIRED_SCOPE)
    );
}

/// A narrowed facade clip remains a virtual-content request and never expands the visible target.
#[test]
fn painter_facade_clips_large_virtual_content_after_scroll_translation() {
    let local = recorder(std::rc::Rc::default(), 1);
    {
        let mut owner = local.try_borrow_mut().unwrap();
        let context = owner.context.as_mut().unwrap();
        context.content_rect = crate::Recti::new(100, -500_000, 80, 500_100);
        context.screen_clip = crate::Recti::new(100, 0, 80, 100);
    }
    let painter: ComPtr<IPainterAbi> = local.query().unwrap();
    let scope = PaintScope {
        invocation: 1,
        clip: Rect { x: 0, y: 500_000, width: 80, height: 100 },
    };
    let facade = neocogi_ui::Painter::from_call(painter.as_com_ref(), scope);
    let mut clipped = facade.clipped(Rect { x: 0, y: 500_020, width: 20, height: 20 }).unwrap();
    clipped.fill(Rect { x: 0, y: 500_020, width: 20, height: 20 }, 0xffffffff).unwrap();
    let context = local.try_borrow_mut().unwrap().finish(1).unwrap();
    let clip = context.display_list.ops[0].clip;
    assert_eq!((clip.x, clip.y, clip.width, clip.height), (100, 20, 20, 20));
}
