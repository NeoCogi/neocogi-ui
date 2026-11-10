//! One stable application base and one retained resource build for live editing.
use super::*;

/// Reuses unchanged resource construction while compiling explicit document revisions.
pub struct ThemeCompiler {
    /// Original application catalog; edits never derive from their own generated images.
    base: ComPtr<IAssetBundleAbi>,
    /// Supplied GFX construction service, independent of this crate's optional offline adapter.
    loader: ComPtr<AssetLoaderAbi>,
    /// Supplied GFX validator and immutable revision publisher.
    publisher: ComPtr<AssetPublisherAbi>,
    /// At most one completed build; concrete recipe equality determines reuse.
    resources: Option<(ResourceRecipe, BuiltResources)>,
}

impl ThemeCompiler {
    /// Retains the stable base and the two explicit construction capabilities.
    pub fn new(base: ComPtr<IAssetBundleAbi>, loader: ComPtr<AssetLoaderAbi>, publisher: ComPtr<AssetPublisherAbi>) -> Self {
        Self { base, loader, publisher, resources: None }
    }

    /// Invalidates source contents after import/replacement at an unchanged authored path.
    pub fn invalidate_resources(&mut self) {
        // The host calls this when its byte map or selected file contents change.
        // No filesystem watcher or implicit polling participates in frame updates.
        self.resources = None;
    }

    /// Compiles the latest document without rereading or repacking unchanged resources.
    pub fn compile(&mut self, document: &ThemeDocument, path: &Path, source: &mut impl AssetSource) -> Result<ComPtr<IAssetBundleAbi>, ThemeLoadError> {
        document.validate_header()?;
        let mut resolved = document.clone();
        resolved.resolve_paths(path.parent().unwrap_or_else(|| Path::new(".")));
        let recipe = resolved.into_recipe(self.base.as_com_ref())?;
        match &self.resources {
            Some((previous, resources)) if previous == &recipe.resources => {
                // Slices, tint, geometry, and colors use the same packed placements.
                // Publication remains the sole validation and immutable bundle boundary.
                ThemeRecipe::compile(&recipe.skin, &recipe.appearances, &resources.images)?.publish(self.publisher.as_com_ref(), resources.bundle.as_com_ref())
            }
            _ => {
                // A failed build or appearance publication leaves the previous cache
                // intact. The host separately retains its last installed preview.
                let resources = recipe.resources.clone().execute(self.base.as_com_ref(), self.loader.as_com_ref(), source)?;
                let bundle = ThemeRecipe::compile(&recipe.skin, &recipe.appearances, &resources.images)?
                    .publish(self.publisher.as_com_ref(), resources.bundle.as_com_ref())?;
                self.resources = Some((recipe.resources, resources));
                Ok(bundle)
            }
        }
    }
}
