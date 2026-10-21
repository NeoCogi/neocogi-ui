//! Appearance publication has no font/image-builder dependency.
use super::*;
use crate::test_support::{test_bundle, data};

/// Real bulk publication preserves diagnostic source requests and still rejects negative slices.
#[test]
fn appearance_publication_accepts_oversized_source_slices_without_rewriting_them() {
    let base = test_bundle();
    let original = data(&base);
    let publisher: ComPtr<AssetPublisherAbi> = AssetPublisherObject::from_value(AssetPublisherObject::default()).query().unwrap();
    let mut surfaces = original.tables.surfaces.to_vec();
    let panel = SurfaceRole::Panel.raw() as usize;
    let slices = Insets::uniform(i32::MAX);
    surfaces[panel].normal.patch = AssetPatch::Image {
        icon: 1,
        source: slices,
        tint: 0xffffffff,
    };
    let revision = AssetAppearanceRevision {
        metrics: original.tables.metrics,
        chrome: original.tables.chrome,
    };
    let candidate = publisher
        .revise_appearance(
            base.as_com_ref(),
            revision,
            NcCopyFromBufferOffset::new(0, &surfaces),
            NcCopyFromBufferOffset::new(0, &original.tables.controls),
            NcCopyFromBufferOffset::new(0, &original.tables.menus),
            NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
        )
        .unwrap();
    let accepted = data(&candidate);
    assert_eq!(accepted.tables.surfaces[panel].normal.patch, surfaces[panel].normal.patch);
    assert_eq!(candidate.image().unwrap().as_raw().object, base.image().unwrap().as_raw().object);
    for source in [
        Insets { left: -1, ..slices },
        Insets { top: -1, ..slices },
        Insets { right: -1, ..slices },
        Insets { bottom: -1, ..slices },
    ] {
        surfaces[panel].normal.patch = AssetPatch::Image { icon: 1, source, tint: 0xffffffff };
        assert_eq!(
            publisher
                .revise_appearance(
                    base.as_com_ref(),
                    revision,
                    NcCopyFromBufferOffset::new(0, &surfaces),
                    NcCopyFromBufferOffset::new(0, &original.tables.controls),
                    NcCopyFromBufferOffset::new(0, &original.tables.menus),
                    NcCopyFromBufferOffset::new(0, &original.tables.window_chrome),
                )
                .unwrap_err(),
            NC_E_INVALID_ARG
        );
    }
    assert_eq!(data(&base).tables.surfaces[panel].normal.patch, original.tables.surfaces[panel].normal.patch);
    assert_eq!(
        data(&candidate).tables.surfaces[panel].normal.patch,
        accepted.tables.surfaces[panel].normal.patch
    );
}

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
    assert_eq!(candidate.catalog().unwrap().as_raw().object, base.catalog().unwrap().as_raw().object);
    assert_eq!(changed.pixels, original.pixels);
    assert_ne!(candidate.as_raw().object, base.as_raw().object);
    let image = candidate.image().unwrap();
    assert_eq!(
        image.as_raw().object,
        base.image().unwrap().as_raw().object,
        "appearance edits retain immutable pixels"
    );
    assert_eq!(image.as_raw().object, candidate.image().unwrap().as_raw().object, "accessor identity is stable");
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
    assert_ne!(rebuilt.image().unwrap().as_raw().object, image.as_raw().object);
    let mut mismatched = original.tables.clone();
    mismatched.size.width += 1;
    assert!(mismatched.into_revision(base.as_com_ref()).is_err());

    // Pixels remain readable after both bundle revisions and the publisher retire.
    drop(candidate);
    drop(rebuilt);
    drop(base);
    drop(publisher);
    let mut bytes = neocogi_com::NcOutputBuffer::new();
    image.pixels(bytes.output(0, original.pixels.len())).unwrap();
    assert_eq!(bytes.as_slice(), original.pixels.as_ref());
}
