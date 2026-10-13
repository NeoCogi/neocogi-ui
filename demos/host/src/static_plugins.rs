//! Assembly selection over concrete linked factory descriptors, before runtime registration.
use neocogi_com::NcPluginDescriptor;
use std::collections::BTreeSet;

/// One available implementation; its package name matches the assembly component declaration.
pub struct StaticPlugin {
    /// Cargo package selected by the packager, independent of the plugin's backend-specific ID.
    pub package: &'static str,
    /// Actual generated descriptor passed to registration, never a parallel factory-name list.
    pub description: &'static NcPluginDescriptor,
}

/// Selects the exact assembled set from available linked implementations, preserving catalog order.
///
/// `requested` is the compile-time `NEOCOGI_ASSEMBLY_PLUGINS` value set by cargo-neocogi.
/// Direct Cargo/browser builds pass `None` and register the complete linked catalog. Validation
/// finishes before any registration: unknown or duplicate packages cannot publish a partial set.
/// Available implementations omitted by the assembly remain unregistered, which supports embedded
/// builds that link shared code but do not deploy the external theme-loader plugin.
pub fn select<'a>(available: &'a [StaticPlugin], requested: Option<&str>) -> Result<Vec<&'a StaticPlugin>, String> {
    let mut catalog = BTreeSet::new();
    for plugin in available {
        if !catalog.insert(plugin.package) {
            return Err(format!("duplicate linked static plugin package {:?}", plugin.package));
        }
    }
    let Some(requested) = requested else { return Ok(available.iter().collect()) };
    let mut selected = BTreeSet::new();
    for package in requested.split(',').filter(|package| !package.is_empty()) {
        if !selected.insert(package) {
            return Err(format!("duplicate assembly plugin package {package:?}"));
        }
        if !catalog.contains(package) {
            return Err(format!("assembly requires static plugin {package:?}, but the application's selected features did not link its factory"));
        }
    }
    Ok(available.iter().filter(|plugin| selected.contains(plugin.package)).collect())
}

#[cfg(all(test, feature = "monolith"))]
mod tests {
    use super::*;

    /// Uses the actual generated runtime and nodes descriptors selected by host registration.
    fn catalog() -> [StaticPlugin; 2] {
        [
            StaticPlugin { package: "neocogi-ui-runtime", description: nc_ui::plugin_descriptor() },
            StaticPlugin { package: "neocogi-ui-nodes", description: nc_ui_nodes::plugin_descriptor() },
        ]
    }

    /// Selection preserves real descriptor identity and omits implementations excluded by assembly.
    #[test]
    fn assembly_selection_uses_linked_descriptors_and_catalog_order() {
        let available = catalog();
        let selected = select(&available, Some("neocogi-ui-nodes,neocogi-ui-runtime")).unwrap();
        assert!(std::ptr::eq(selected[0].description, nc_ui::plugin_descriptor()));
        assert!(std::ptr::eq(selected[1].description, nc_ui_nodes::plugin_descriptor()));
        assert_eq!(select(&available, Some("neocogi-ui-nodes")).unwrap().len(), 1);
        assert!(select(&available, Some("")).unwrap().is_empty());
        assert_eq!(select(&available, None).unwrap().len(), 2);
    }

    /// Drift is diagnosed before registration rather than surfacing as a later activation failure.
    #[test]
    fn missing_or_duplicate_packages_reject_the_complete_selection() {
        let available = catalog();
        assert!(select(&available, Some("missing-plugin")).err().unwrap().contains("did not link"));
        assert!(select(&available, Some("neocogi-ui-nodes,neocogi-ui-nodes")).is_err());
        let duplicate = [
            StaticPlugin { package: "same", description: available[0].description },
            StaticPlugin { package: "same", description: available[1].description },
        ];
        assert!(select(&duplicate, None).is_err());
    }
}
