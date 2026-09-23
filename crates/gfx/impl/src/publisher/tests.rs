//! Appearance publication has no font/image-builder dependency.
use super::*;
use crate::test_support::{test_bundle, data};
use neocogi_com::NcCopyToBufferOffset;

/// Appearance publication preserves resources and rejects malformed bulk records inside GFX.
#[test]
fn appearance_revision_is_a_validated_resource_preserving_transaction() {
    let base = test_bundle();
    let original = data(&base);
    let publisher: ComPtr<AssetPublisherAbi> = AssetPublisherObject::from_value(AssetPublisherObject::default()).query().unwrap();
    let mut metrics = original.tables.metrics;
    metrics.padding += 3;
    let revision = AssetAppearanceRevision { metrics, chrome: original.tables.chrome };
    let candidate = publisher
        .revise_appearance(
            base.as_com_ref(),
            revision,
            NcCopyFromBufferOffset::new(0, &original.tables.surfaces),
            NcCopyFromBufferOffset::new(0, &original.tables.controls),
            NcCopyFromBufferOffset::new(0, &original.tables.menus),
            NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
        )
        .unwrap();
    let changed = data(&candidate);
    assert_eq!(candidate.catalog().unwrap().as_ptr(), base.catalog().unwrap().as_ptr());
    assert_eq!(changed.pixels, original.pixels);
    assert_ne!(candidate.as_ptr(), base.as_ptr());
    let image = candidate.image().unwrap();
    assert_eq!(image.as_ptr(), base.image().unwrap().as_ptr(), "appearance edits retain immutable pixels");
    assert_eq!(image.as_ptr(), candidate.image().unwrap().as_ptr(), "accessor identity is stable");
    assert_eq!(changed.tables.fonts, original.tables.fonts);
    assert_eq!(changed.tables.icons, original.tables.icons);
    assert_eq!(changed.tables.metrics, metrics);

    // A role table is all-or-nothing. The service validates range shape before copying any
    // caller-owned record or publishing a candidate with a partially replaced appearance family.
    assert_eq!(
        publisher
            .revise_appearance(
                base.as_com_ref(),
                revision,
                NcCopyFromBufferOffset::new(1, &original.tables.surfaces),
                NcCopyFromBufferOffset::new(0, &original.tables.controls),
                NcCopyFromBufferOffset::new(0, &original.tables.menus),
                NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
            )
            .unwrap_err(),
        NC_E_INVALID_ARG
    );

    // A resource rebuild receives a fresh identity, even when its resulting bytes
    // happen to match. Consumers need no pointer/hash cache to infer pixel ownership.
    let rebuilt = original.clone().into_revision(base.as_com_ref()).unwrap();
    assert_ne!(rebuilt.image().unwrap().as_ptr(), image.as_ptr());
    let mut mismatched = original.tables.clone();
    mismatched.size.width += 1;
    assert!(mismatched.into_revision(base.as_com_ref()).is_err());

    // Pixels remain readable after both bundle revisions and the publisher retire.
    drop(candidate);
    drop(rebuilt);
    drop(base);
    drop(publisher);
    let mut bytes = vec![0; original.pixels.len()];
    image.pixels(NcCopyToBufferOffset::new(0, &mut bytes)).unwrap();
    assert_eq!(bytes, original.pixels.as_ref());
}
