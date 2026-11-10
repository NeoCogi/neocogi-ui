//! Run real archived providers and independently generated COM calls outside the checkout.
extern crate self as neocogi_ui_package_consumer;
use neocogi_com::{ComPtr, ComRef, NcStatus, com_object};
use neocogi_ui_contracts::{abi::core::*, Ui};

/// Independently generated event contracts and their ordinary public COM metadata.
pub mod contracts {
    include!(env!("IDL_RELEASE_PROBE"));
}

#[com_object(interfaces(contracts::IReleaseEvent), ownership = exclusive)]
/// Concrete event owner exposes the inherited UI event contract through checked ABI calls.
struct ReleaseEvent;
impl IEvent for ReleaseEvent {}
impl contracts::IReleaseEvent for ReleaseEvent {
    /// The value is read through independently emitted method metadata and vtable code.
    fn value(&self) -> Result<u32, NcStatus> {
        Ok(73)
    }
}

#[com_object(interfaces(IRenderer))]
/// Consumer-owned sink authenticates complete production GFX blocks on the logical test device.
struct Renderer;
impl IRenderDriverBound for Renderer {
    /// No native resource capability is fabricated by this CPU-only consumer.
    fn driver_identity(&self) -> Result<ComPtr<IRenderDriverIdentityAbi>, NcStatus> {
        neocogi_ui::gfx::testing::host().driver_identity()
    }
}
impl IRenderer for Renderer {
    /// These are actual sealed contributions emitted by the archived compositor, not copied draw fixtures.
    fn render_result(&self, info: RenderInfo, stream: ComRef<'_, IRenderStreamAbi>) -> Result<ComPtr<IRenderOutcomeAbi>, NcStatus> {
        let blocks = neocogi_ui::gfx::testing::blocks(info, stream)?;
        assert!(blocks.iter().any(|block| !block.commands().is_empty()));
        Ok(operation_result(Ok(())))
    }
}

/// Link or load the same archived implementations; loading accepts only checker-supplied trusted paths.
fn register(runtime: neocogi_com::Runtime) -> Result<(), NcStatus> {
    let libraries: Vec<_> = std::env::args_os().skip(1).collect();
    if libraries.is_empty() {
        // SAFETY: the linked descriptor and every reachable function remain mapped for process lifetime.
        unsafe { runtime.register_plugin_descriptor(neocogi_ui::plugin_descriptor())?; }
    } else {
        for library in libraries {
            // SAFETY: the qualification tool supplies only exact libraries built from inspected archives.
            // The host validates descriptors and retains their code for every acquired COM object's lifetime.
            unsafe {
                runtime.load_plugin_library(std::path::PathBuf::from(library));
            }
        }
    }
    Ok(())
}

/// Acquire actual packaged factories, publish assets, create a button, render and inspect owned events.
fn run(runtime: neocogi_com::Runtime) -> Result<(), Box<dyn std::error::Error>> {
    register(runtime)?;
    let plugins = runtime.plugins()?;
    assert_eq!(plugins.len(), 1);
    for plugin in plugins {
        assert_eq!(plugin.version, "0.2.0-alpha.2");
    }
    let event = ReleaseEvent::into_local_box(ReleaseEvent)
        .into_interface::<contracts::IReleaseEventAbi>()
        .map_err(|(status, _)| status)?;
    assert_eq!(event.value()?, 73);
    let owned = event.query::<IEventAbi>().map_err(|(status, _)| status)?;
    assert!(owned.query_ref::<contracts::IReleaseEventAbi>().is_ok());
    let assets = neocogi_ui::support::test_support::test_asset_data_with_font_sizes(&[("body", 12)]).into_bundle()?;
    let gfx = neocogi_ui_contracts::services::gfx(runtime)?;
    let skin = neocogi_ui_contracts::services::skin(runtime)?;
    let publisher = gfx.asset_publisher()?;
    let compiler = skin.style_compiler(publisher.as_com_ref())?;
    let assets = compiler.derive_style(assets.as_com_ref(), assets.metrics()?, neocogi_ui_contracts::appearance::skin::PaletteEdit::Preserve)?;
    let factory = neocogi_ui_contracts::services::ui(runtime)?;
    let mut core = factory.create_runtime(assets.as_com_ref(), gfx.as_com_ref())?;
    core.set_render_host(
        neocogi_ui::gfx::testing::host().as_com_ref(),
        neocogi_ui::gfx::testing::framebuffer().as_com_ref(),
    )?;
    let nodes = neocogi_ui_contracts::services::nodes(runtime)?;
    let widgets = nodes.bind(core.as_com_ref())?;
    let ui = Ui::from_interfaces(core, widgets)?;
    let (queue, messages) = neocogi_ui_contracts::events::channel();
    let button = ui.widgets().button_with_queue("Archived providers", Some(queue.as_com_ref()))?;
    let observed = button.as_interface().downgrade();
    let window = ui
        .window("Package consumer", Rect { x: 0, y: 0, width: 300, height: 160 }, button)
        .map_err(|(status, _)| status)?;
    let size = Size { width: 400, height: 240 };
    for key in [KeyCode::Tab, KeyCode::Space] {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                action: neocogi_ui_contracts::KeyAction::Pressed { repeat: false },
                modifiers: 0,
            }),
            "",
        )?;
    }
    ui.update(size)?;
    let message = messages.try_recv()?;
    assert!(message.query::<IActivatedEventAbi>()?.is_some());
    let renderer: ComPtr<IRendererAbi> = Renderer::from_value(Renderer).query()?;
    ui.render(size, renderer.as_com_ref())?;
    window.close()?;
    assert!(!observed.is_alive());
    assert!(message.query::<IActivatedEventAbi>()?.is_some(), "owned facts outlive their removed widget");
    ui.close()?;
    println!("PASS: independently generated COM event, archived provider activation, input, rendering and event lifetime");
    Ok(())
}

/// The executable owns the only process runtime used by linked and dynamically loaded providers.
#[neocogi_com::com_main]
fn main(runtime: neocogi_com::Runtime) -> std::process::ExitCode {
    match run(runtime) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
