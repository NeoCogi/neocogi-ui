# Provider-owned widget configuration

The concrete widget constructor owns its initial requested flags. Factories
invoke that constructor; Rust convenience methods, UI documents and the designer
must not supply their own per-kind default masks. The IDL describes ordinary
COM methods and values, with no widget-specific constructor/default syntax.

## Construction and explicit changes

The public Button factory now follows the other built-in constructors:

```rust
let mut save = ui.widgets().button_unobserved("Save")?;
assert_eq!(save.requested_flags()?, WidgetOptions::FRAME);

// This is application presentation, applied explicitly after construction.
save.replace_flags(WidgetOptions::NONE)?;
save.set_flags(WidgetOptions::ALIGN_RIGHT)?;
assert_eq!(save.requested_flags()?, WidgetOptions::ALIGN_RIGHT);
```

The underlying COM call is `UiFactory.create_button(label, events)`. The concrete
Button provider selects `FRAME`; neither the factory caller nor the Rust facade
passes that mask. Internal compositions may still choose deliberate presentation
for their own private controls. That is composition policy, not a second default
for the public Button factory.

`IWidget.requested_flags()` reads the same configuration its `replace_flags`,
`set_flags` and `clear_flags` methods edit. It does not store an additional initial
snapshot. Custom COM widgets implement the getter against their own authoritative
state, just as built-ins do. Composite implementations delegate to their root
state. Normal host/readability and dropped-object checks still apply.

`IWidget.properties().options` remains effective runtime policy. For example, a
scrolling editor may report `FRAME | GRAB_SCROLL` there while its requested flags
are only `FRAME`. Reading effective flags and saving them as configuration would
freeze transient behavior into the document. The two observations have distinct
contracts and must not be substituted for one another.

## Documents preserve absence

`Style.options` is an optional **full requested-mask replacement**:

| Authored value | Loader behavior | Button / example Counter result |
| --- | --- | --- |
| `None` / JSON `null` | Leave the constructor configuration alone | Provider's `FRAME` |
| `Some(0)` / JSON `0` | Call `replace_flags(NONE)` | No requested flags |
| `Some(ALIGN_RIGHT.raw())` | Replace the entire requested mask | `ALIGN_RIGHT`, without `FRAME` |

There is no `WidgetSpec::default_options()` table. This applies equally to built-in
widgets and independently built plugins discovered through the COM catalog.
Typed compilation and binary loading share the same construction path. Saving a
component preserves the authored option, including absence, even if application
code has since changed the live widget. Reloading an absent override selects the
current provider's defaults; freezing a particular mask requires an explicit value.

Font, icon and image setters remain post-construction style operations, separate
from constructor arguments and the optional flag replacement.

## Designer behavior

The Defaults checkbox selects whether an override exists. Turning it off records
the configuration currently shown; turning it on removes the override. The first
individual flag edit starts with the constructor's requested flags when no
override exists, and with the explicit mask otherwise. Unrelated bits survive;
Center and Right remain mutually exclusive authoring choices.

For a defaulted Properties inspector or flag edit, the designer constructs a fresh,
unobserved, unmounted copy of the selected recipe subtree through the production
loader in the preview runtime. It retains constructor arguments and child layout
relationships, omits post-construction style overrides/resources, reads requested
flags through COM, then drops the temporary owner. This requires no plugin-specific
Rust linkage, property downcast, extra metadata schema, or saved defaults table.
A custom constructor must support its documented unobserved construction mode.

This construction happens during inspector rebuilds and flag edits, not every
render frame. Explicit masks skip it. The tradeoff is temporary construction work
for defaulted controls (including their descendants). There is deliberately no
cache whose validity would depend on edited custom constructor properties or
preview generations. A failed query fails the flag edit before changing source. Properties inspection
shows the error while keeping constructor fields editable, so the recipe can be
corrected instead of substituting an invented zero mask.
The Events tab reads metadata only and never performs this configuration probe.

A fresh recipe matters: Run mode can change live configuration, and multiple
accepted edits can precede preview publication. Neither the modified preview nor
an earlier constructor recipe is authoritative for the next authored flag edit.

Fixed-coordinate Form geometry samples requested flags once from the preview
provider, together with the theme's frame insets. Form's constructor contract
makes its initial flags independent of content size; explicit per-node masks
still override that sample, including zero. No literal Form default is copied
into the designer.

## Why the old workaround was removed

Commit `5c68e563` introduced a document defaults table while avoiding transient
flags being saved as authored settings. Commit `15adc941` then assigned custom
widgets `NONE`. The later requested-flag setters (`eea99856`) established the
right state boundary, but the loader still unconditionally overwrote initial
configuration from the table.

The repair keeps that requested/effective distinction, makes requested state
observable through COM, and applies document replacements only when explicitly
authored. It does not restore reads from effective properties or teach the IDL
compiler widget-specific semantics.

## Acceptance

Real generated COM tests cover direct factory and facade defaults, explicit zero
and nonzero replacements, selective edits, effective scrolling flags, dropped
observers, runtime shutdown, and Form size independence. Loader tests cover typed,
JSON and binary absence/replacement semantics and saving after live application
edits. Existing scrolling/enablement regressions remain in the runtime suite.

Offscreen designer tests cover every palette entry, first flag edits, freezing and
restoring defaults, edits before preview refresh, live-preview divergence, frame
coordinates, failed baseline resolution, and metadata-only Events inspection.
The independently compiled Counter plugin retains its constructor's `FRAME`
through compile/save/load and designer edits. Browser compilation is checked;
these tests do not claim browser execution or a binary-size measurement.
