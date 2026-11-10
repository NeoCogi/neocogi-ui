//! Exercises the exact root README onboarding blocks with real CPU-backed providers.

/* checked-example: application-install */

#[test]
fn documented_application_onboarding() -> Result<(), Box<dyn std::error::Error>> {
    let source =
        neocogi_ui_support::test_support::assets::export_bundle(&neocogi_ui_support::SkinBundle::from_atlas(neocogi_ui_support::test_support::test_atlas()))?;
    let gfx = neocogi_ui_support::test_support::gfx_factory(crate::host_runtime());
    let ui = nc_ui_nodes::bind_builtin_nodes(nc_ui::create_runtime_from_assets(source.as_com_ref(), gfx.as_com_ref())?)?;
    let size = Size { width: 640, height: 400 };
    let hello = install(&ui)?;
    ui.update(size)?;
    hello.close()?;

    /* checked-example: application-events */

    /* checked-example: application-update */

    assert_eq!(app.try_borrow()?.processor.count, 0);
    for key in [KeyCode::Tab, KeyCode::Space] {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                action: neocogi_ui::KeyAction::Pressed { repeat: false },
                modifiers: 0,
            }),
            "",
        )?;
    }

    /* checked-example: application-update */

    assert_eq!(
        app.try_borrow()?.processor.count,
        1,
        "real input reaches the common receiver and deferred handler"
    );
    ui.input(
        UiInput::Key(KeyStroke {
            key: KeyCode::Space,
            action: neocogi_ui::KeyAction::Pressed { repeat: false },
            modifiers: 0,
        }),
        "",
    )?;
    // Deliberately route without dispatch to test the documented removed-origin policy.
    ui.update(size)?;
    window.close()?;

    /* checked-example: application-update */

    assert_eq!(app.try_borrow()?.processor.count, 1, "queued work from a removed button is ignored");
    ui.close()?;
    Ok(())
}
