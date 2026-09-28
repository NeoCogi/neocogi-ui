//! COM catalog adapter for independently built custom widget plugins.
use crate::{
    CustomProperty, CustomWidgetFactory, CustomWidgetRegistry,
    design::{DesignWidgetManifest, MAX_DESIGN_DESCRIPTOR_BYTES},
};
use neocogi_com::{IUnknown, NcValueRef, ComPtr, NcCopyFromBufferOffset, NcCopyToBufferOffset, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::{IWidgetAbi, Ui, Widget};
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

    /// Calls the plugin's common COM constructor with typed, encoded properties.
    fn create(&self, ui: &Ui, properties: &[CustomProperty], sink: Option<ComPtr<IUnknown>>) -> Result<Widget<IWidgetAbi>, NcStatus> {
        let encoded = serde_json::to_vec(properties).map_err(|_| NC_E_INVALID_ARG)?;
        if encoded.len() > MAX_DESIGN_DESCRIPTOR_BYTES {
            return Err(NC_E_INVALID_ARG);
        }
        let host = ui.widgets().host()?;
        let binding = match sink {
            None => neocogi_ui_document_contracts::WidgetEventBinding::Unobserved,
            Some(receiver) => neocogi_ui_document_contracts::WidgetEventBinding::Observed(receiver),
        };
        let widget = self.catalog.create_widget(
            self.index,
            host.as_com_ref(),
            NcCopyFromBufferOffset::new(0, &encoded),
            NcValueRef::new(&binding)?,
        )?;
        Ok(Widget::from_interface(widget))
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
            let required = catalog
                .describe(index, NcCopyToBufferOffset::new(0, &mut []))
                .map_err(|status| format!("Widget catalog description length failed: {status}"))? as usize;
            if required == 0 || required > MAX_DESIGN_DESCRIPTOR_BYTES {
                return Err("Widget catalog returned an invalid description length".into());
            }
            let mut bytes = vec![0; required];
            let written = catalog
                .describe(index, NcCopyToBufferOffset::new(0, &mut bytes))
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
