//! Bootstrap and behavioral assertions surrounding the exact document README blocks.
//! The parent test supplies fresh generated Form fields and inserts the marked code.

#[cfg(test)]
mod tests {
    use super::generated;
    use neocogi_com::{INcStringComPtrExt, NcStatus};
    use neocogi_ui::{CrossSize, FlowDirection, GridLimits, IActivatedEventAbi, KeyCode, KeyStroke, LayoutTrack, LinearEdge, Rect, Size, Ui, UiInput};
    use neocogi_ui_document::{CustomWidgetRegistry, Document, DocumentEvent, EventBindings, ReloadIds, ReloadSlot, Resources, document_receiver};

    /// Forwards real keyboard input; it never calls the button or receiver directly.
    fn press(ui: &Ui, key: KeyCode) -> Result<(), NcStatus> {
        ui.input(
            UiInput::Key(KeyStroke {
                key,
                action: neocogi_ui::KeyAction::Pressed { repeat: false },
                modifiers: 0,
            }),
            "",
        )
    }

    /// Exercises deferred behavior, source commands and replacement admission with
    /// the production runtime, loader and nodes behind their generated COM contracts.
    #[test]
    fn documented_reload() -> Result<(), Box<dyn std::error::Error>> {
        let source = neocogi_ui_support::test_support::assets::export_bundle(&neocogi_ui_support::SkinBundle::from_atlas(
            neocogi_ui_support::test_support::test_atlas(),
        ))?;
        let gfx = neocogi_ui_support::test_support::gfx_factory(crate::host_runtime());
        let ui = nc_ui_nodes::bind_builtin_nodes(nc_ui::create_runtime_from_assets(source.as_com_ref(), gfx.as_com_ref())?)?;
        let size = Size { width: 640, height: 400 };
        let root_owner = ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
        let mut root = root_owner.handle();
        let window = ui
            .window("README", Rect { x: 20, y: 20, width: 400, height: 200 }, root_owner)
            .map_err(|(status, _root)| status)?;
        let mut ids = ReloadIds::new();
        let mut slot = ReloadSlot::new(&mut ids, ui.clone(), window.clone(), GridLimits { max_columns: 64, max_rows: 256 })?;
        root.insert(0, slot.take_widget()?, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 }))
            .map_err(|(status, _slot)| status)?;
        let document = Document::new("README events");
        let registry = CustomWidgetRegistry::new();
        let resources = Resources::new();
        let (sender, receiver) = std::sync::mpsc::channel::<DocumentEvent>();

        /* checked-example: document-reload */

        ui.update(size)?;
        press(&ui, KeyCode::Tab)?;
        press(&ui, KeyCode::Space)?;
        ui.update(size)?;
        assert_eq!(form.widget_title_1.text()?, "Hello UI", "posting defers application behavior");
        form.widget_save_2.set_label("edited after posting")?;

        /* checked-example: document-dispatch */

        assert_eq!(
            form.widget_title_1.text()?,
            "Saved",
            "dispatch applies the source command independently of display edits"
        );
        form.widget_save_2.set_label("stale")?;
        press(&ui, KeyCode::Space)?;
        ui.update(size)?;
        let previous = form.widget_save_2.as_interface().clone();
        let resources = Resources::new();

        /* checked-example: document-reload */

        assert!(!previous.is_alive(), "replacement destroys the old widget owner");

        /* checked-example: document-dispatch */

        assert_eq!(form.widget_title_1.text()?, "Hello UI", "stale delivery cannot edit the replacement");
        ui.update(size)?;
        press(&ui, KeyCode::Tab)?;
        press(&ui, KeyCode::Space)?;
        ui.update(size)?;

        /* checked-example: document-dispatch */

        assert_eq!(form.widget_title_1.text()?, "Saved", "the replacement handles its own facts");
        window.close()?;
        Ok(())
    }
}

// The fixture final executable explicitly owns its process runtime.
extern crate neocogi_com_host as _;

/// This generated walkthrough executable explicitly selects its host owner.
#[cfg(test)]
fn host_runtime() -> neocogi_com::Runtime {
    // SAFETY: its linked implementation owns the complete immutable process-lived graph.
    unsafe { neocogi_com::Runtime::__from_host_api(neocogi_com_host::initialize_process()) }.unwrap()
}
