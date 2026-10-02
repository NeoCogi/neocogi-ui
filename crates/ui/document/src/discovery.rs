//! COM catalog adapter for independently built custom widget plugins.
use crate::{
    CustomWidgetFactory, CustomWidgetRegistry,
    design::{DesignWidgetManifest, MAX_DESIGN_DESCRIPTOR_BYTES},
};
use neocogi_com::{IUnknown, NcValueRef, ComPtr, NcCopyFromBufferOffset, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{IWidgetAbi, Ui};
use neocogi_ui_document_contracts::IDesignWidgetCatalogAbi;
use std::rc::Rc;

/// Maximum number of widget kinds accepted from one plugin catalog.
pub const MAX_CATALOG_WIDGETS: u32 = 256;

/// One COM-backed constructor tied to a catalog index and validated description.
struct CatalogWidgetFactory {
    /// Retained catalog identity keeps the plugin factory alive for previews.
    catalog: ComPtr<IDesignWidgetCatalogAbi>,
    /// Stable ordinal supplied by this catalog's description pass.
    index: u32,
    /// Complete owned authoring metadata copied before registration.
    manifest: DesignWidgetManifest,
}

impl CustomWidgetFactory for CatalogWidgetFactory {
    /// Supplies the exact previously validated snapshot to the registry.
    fn manifest(&self) -> Result<DesignWidgetManifest, String> {
        Ok(self.manifest.clone())
    }

    /// Absence of the optional editor is distinct from failure to open a supported editor.
    fn create_editor(&self, ui: &Ui, settings: &[u8]) -> Result<Option<neocogi_com::ComBox<neocogi_ui_document_contracts::IDesignSettingsEditorAbi>>, NcStatus> {
        match self.catalog.create_editor(self.index, ui.widgets().as_com_ref(), NcCopyFromBufferOffset::new(0, settings)) {
            Ok(editor) => Ok(Some(editor)),
            Err(neocogi_com::NC_E_NOINTERFACE) => Ok(None),
            Err(status) => Err(status),
        }
    }

    /// Calls the plugin's common COM constructor with typed, encoded settings.
    fn create(&self, ui: &Ui, settings: &[u8], sink: Option<ComPtr<IUnknown>>) -> Result<neocogi_ui::OwnedWidget<IWidgetAbi>, NcStatus> {
        // The registry preflights the complete document before construction. Keep
        // this boundary bounded too, so direct adapter calls cannot allocate an
        // arbitrarily large transport buffer before discovering its limit.
        neocogi_ui_design::validate_settings_size(settings).map_err(|_| NC_E_INVALID_ARG)?;
        let host = ui.widgets().host()?;
        let binding = match sink {
            None => neocogi_ui_document_contracts::WidgetEventBinding::Unobserved,
            Some(receiver) => neocogi_ui_document_contracts::WidgetEventBinding::Observed(receiver),
        };
        let widget = self.catalog.create_widget(
            self.index,
            host.as_com_ref(),
            NcCopyFromBufferOffset::new(0, settings),
            NcValueRef::new(&binding)?,
        )?;
        Ok(neocogi_ui::OwnedWidget::from_interface(widget))
    }
}

impl CustomWidgetRegistry {
    /// Discovers and installs every widget exposed by one queried COM catalog.
    ///
    /// Metadata is copied through `describe` into bounded owned records. The
    /// registry validates every record before publishing any new palette kind.
    pub fn register_catalog(&mut self, catalog: ComPtr<IDesignWidgetCatalogAbi>) -> Result<(), String> {
        let count = catalog.widget_count().map_err(|status| format!("Widget catalog count failed: {status}"))?;
        if count > MAX_CATALOG_WIDGETS {
            return Err("Widget catalog exceeds kind limit".into());
        }
        let mut candidate = self.clone();
        for index in 0..count {
            let mut bytes = neocogi_com::NcOutputBuffer::<u8>::new();
            let required = catalog
                .describe(index, bytes.output(0, 0))
                .map_err(|status| format!("Widget catalog description length failed: {status}"))? as usize;
            if required == 0 || required > MAX_DESIGN_DESCRIPTOR_BYTES {
                return Err("Widget catalog returned an invalid description length".into());
            }
            let written = catalog
                .describe(index, bytes.output(0, required))
                .map_err(|status| format!("Widget catalog description failed: {status}"))? as usize;
            if written != required {
                return Err("Widget catalog changed a description during discovery".into());
            }
            let manifest = DesignWidgetManifest::decode(&bytes)?;
            candidate.register(Rc::new(CatalogWidgetFactory {
                catalog: catalog.clone(),
                index,
                manifest,
            }))?;
        }
        *self = candidate;
        Ok(())
    }
}

/// Copies a validated draft without allocating from an unchecked provider length.
/// No UI input is dispatched between these calls; a changed size is a protocol error.
pub fn editor_settings(editor: &neocogi_com::WeakComBox<neocogi_ui_document_contracts::IDesignSettingsEditorAbi>) -> Result<Vec<u8>, String> {
    let mut bytes = neocogi_com::NcOutputBuffer::<u8>::new();
    let required = editor.settings(bytes.output(0, 0)).map_err(|status| format!("Invalid settings draft: {status}"))? as usize;
    if required > neocogi_ui_design::MAX_SETTINGS_BYTES {
        return Err("Settings editor exceeded byte limit".into());
    }
    let written = editor.settings(bytes.output(0, required)).map_err(|status| format!("Settings snapshot failed: {status}"))? as usize;
    if required != written || bytes.len() != required {
        return Err("Settings editor changed its snapshot during copying".into());
    }
    Ok(bytes.into_vec())
}

#[cfg(test)]
mod tests {
    use neocogi_com::NcOutputWriter;
    use super::*;
    use neocogi_com::{ComBox, NC_E_OUT_OF_MEMORY};
    use neocogi_ui_document_contracts::{IDesignSettingsEditor, IDesignSettingsEditorAbi};

    /// Deliberately varied provider responses exercise real generated COM output calls.
    enum Reply {
        /// Stable data copied only when the complete buffer fits.
        Bytes(Vec<u8>),
        /// Host must reject this claim before issuing a second copy request.
        Oversized,
        /// Different count on the second call violates the snapshot protocol.
        ChangedLength,
        /// A provider failure must remain a failure, never editor absence.
        Failure,
    }
    #[neocogi_com::com_object(interfaces(neocogi_ui_document_contracts::IDesignSettingsEditor), ownership = exclusive)]
    /// This controller fixture needs no UI tree to exercise bounded snapshot acquisition.
    struct Editor {
        /// Selected protocol behavior; the object never changes it between calls.
        response: Reply,
    }
    impl IDesignSettingsEditor for Editor {
        /// Snapshot tests never mount controls.
        fn take_view(&mut self) -> Result<ComBox<IWidgetAbi>, NcStatus> { Err(neocogi_com::NC_E_NOT_FOUND) }
        /// Keeps responses deterministic without a call counter or interior mutability.
        fn settings(&self, mut output: NcOutputWriter<'_, u8>) -> Result<u32, NcStatus> {
            match &self.response {
                Reply::Bytes(bytes) => {
                    if output.capacity() >= bytes.len() { output.extend_from_slice(bytes)?; }
                    Ok(bytes.len() as u32)
                }
                Reply::Oversized => {
                    assert!(output.is_empty(), "host copied an excessive response");
                    Ok(neocogi_ui_design::MAX_SETTINGS_BYTES as u32 + 1)
                }
                Reply::ChangedLength => Ok(if output.is_empty() { 8 } else { 9 }),
                Reply::Failure => Err(NC_E_OUT_OF_MEMORY),
            }
        }
    }
    /// Invalid provider lengths/statuses cannot become accepted owned settings.
    #[test]
    fn snapshot_transport_bounds_lengths_and_preserves_errors() {
        for (response, expected) in [(Reply::Oversized, "byte limit"), (Reply::ChangedLength, "changed"), (Reply::Failure, "OUT_OF_MEMORY")] {
            let editor = Editor::into_box(Editor { response }).query::<IDesignSettingsEditorAbi>().ok().unwrap();
            assert!(editor_settings(&editor.downgrade()).unwrap_err().contains(expected));
        }
        let bytes = vec![0x55; neocogi_ui_design::MAX_SETTINGS_BYTES];
        let editor = Editor::into_box(Editor { response: Reply::Bytes(bytes.clone()) }).query::<IDesignSettingsEditorAbi>().ok().unwrap();
        assert_eq!(editor_settings(&editor.downgrade()).unwrap(), bytes);
        let observer = editor.downgrade();
        drop(editor);
        assert!(editor_settings(&observer).unwrap_err().contains("DROPPED"));
    }
}
