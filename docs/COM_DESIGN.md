**COM migration proposal for microui-redux**

This is the accepted design and implementation plan. Progress is tracked in COM_MIGRATION.md. It incorporates the requested statically linked generic Rust façade over dynamically linked COM implementations, and importing nc-com into this workspace so it can be modified here.

The analysis used microui-redux `0.9-alpha.0` at `6b4fd86` and nc-com at `fbb66e6`, located at `/home/aifu/projects/neocogi/nc-com`. The `neo-cogi` spelling in the original path differs from the actual directory.

**1. Recommended overall design**

Use nc-com itself as the COM foundation. Import its source, preserve its crate boundaries, and make focused changes there. Introduce one small UI contract crate and turn the existing public microui-redux crate into the statically linked Rust façade. Move the retained implementation behind that façade into a UI plugin.

Keep three plugin groups:

| Plugin group | Contents | Typical deployment |
|---|---|---|
| UI | UI runtime, window manager, widgets, containers, menus, standard dialogs, layout, input routing, display-list generation and geometry expansion | Required |
| Platform/rendering | Native host/window integration and the selected rendering backend; application GPU extensions compiled into the same binary | Required; normally build one backend |
| Assets | Atlas building, font rasterization, PNG decoding, optional JSON theme loading | Optional; omitted when supplying compiled assets |

The nc-com runtime shared library is infrastructure, not another UI plugin. A normal native installation would therefore contain the executable, the nc-com runtime library, the UI plugin, a platform/rendering plugin, and optionally the assets plugin.

Existing backend/helper crates can remain statically linked dependencies inside the platform plugin. A Cargo crate does not need its own plugin descriptor or shared library. In particular, do not make every widget, container, renderer-support helper, painter, or event type a separately loaded plugin.

The native architecture is:

```text
application and application state
    |
    | statically linked Rust generics, concrete proxy types, local borrows
    v
microui-redux façade  -------->  nc-com façade
    |                              |
    | generated COM calls          | class loading and activation
    v                              v
microui-contracts             one nc-com runtime library
    |                              |
    +------------------------------+--------------------+
    |                              |                    |
UI plugin                  platform/GPU plugin     optional assets plugin
retained UI engine          native window + GPU     builder / PNG / JSON
```

COM boundaries are between separately compiled binaries. Generics, Rust helper traits, `Vec`, `String`, and private resource ownership remain useful within each binary. The façade can be generic without making any COM interface generic.

Default planning assumption: preserve the current browser demo through a statically linked build of the same COM interfaces. This is a separate deployment mode from the statically linked façade used by native dynamic deployments. Browser dynamic plugin loading is outside the initial design.

**2. What nc-com already provides**

The existing project already has most of the necessary foundation:

- `#[com_interface]`: a Rust implementation trait, explicit C-shaped interface header/vtable, safe owning-pointer call helpers, interface GUID and ABI hash.
- `#[com_class]`: one generated allocation containing the Rust implementation, interface headers, canonical IUnknown support, and an atomic reference count.
- `ComPtr<T>`: one owned reference; cloning adds a reference and dropping releases it.
- `ComRef<'_, T>`: a borrowed interface parameter. A recipient can retain the object by explicitly acquiring an owned reference.
- `#[com_struct]`, `com_int_enum!`, and `#[com_enum]`: fixed-layout values and tagged payloads suitable for ABI contracts.
- Explicit copy-buffer argument modes and standard immutable string/byte interfaces.
- Descriptor validation, class activation, plugin dependency ordering information, and inspection tools.
- Default thread-local pointers, which are neither Send nor Sync. This fits SDL, UI objects, and GPU context ownership.

Its current rules also matter:

- Interface methods use `&self` and `Result<T, NcStatus>`; generic interface methods, associated types, arbitrary Rust references and containers do not cross the ABI.
- The public `Runtime` facade is process-global and expects a separately built nc-com runtime shared library.
- Loaded plugins remain mapped until process exit. Live activation failures abort; plugin dependency entries do not discover or recursively load files.
- Panics are not converted into error statuses. A panic at the C boundary aborts.
- There is no existing general COM weak-reference API.
- COM object destruction happens at final Release. It cannot act as a fallible submit/shutdown operation.

Relevant sources: [nc-com usage and ABI rules](../crates/com/README.md#L261), [ownership and threading](../crates/com/README.md#L837), [startup loading](../crates/com/README.md#L700), [runtime linkage](../crates/com/nc-com-runtime-sys/src/lib.rs#L1).

**3. Importing nc-com into this workspace**

Import the six existing crates, with their names preserved:

```text
crates/com/nc-com-core/
crates/com/nc-com-macros/
crates/com/nc-com/
crates/com/nc-com-runtime-sys/
crates/com/nc-com-runtime/
tools/nc-com-inspect/
```

These become workspace members with path-based workspace dependencies. This is an owned source import, not a dependency on another developer's home-directory checkout and not an unpinned remote dependency.

Preserve MIT licensing, attribution, source provenance, tests, ABI fixtures, and the canonical COM documentation. Import package metadata explicitly: nc-com currently inherits its own version, license, rust-version and documentation metadata from its workspace, which differ from microui's metadata. Do not accidentally give the imported crates the UI package's composite license or UI version.

Move or adapt nc-com's build/test scripts, workflow-example fixtures and path assumptions. Record the imported revision so future changes can be compared with the original. Do not inherit its root toolchain pin across the UI workspace: select toolchains explicitly for COM compatibility tests and nightly min-size builds.

Only the executable/bootstrap path needs the runtime-loading facade. Implementation plugins should normally depend on contracts and nc-com's object/macros support, rather than linking their own nc-com-runtime implementations. Dependencies or services needed by an object should be supplied through typed constructor/factory interfaces. Ordinary method calls should not perform global registry lookups.

**4. nc-com changes to make here**

There are several concrete additions worth making before migrating the retained engine.

| Change | Reason | Priority |
|---|---|---|
| Support recursive interface relationships safely | Natural object APIs can reference themselves or one another | Resolve before freezing UI contracts |
| Generate safe call helpers for `ComRef` as well as `ComPtr` | Current borrowed-object examples must acquire an owned reference to call generated helpers | Early; measure refcount savings |
| Separate plugin descriptors from exporting the dynamic entrypoint | Current `plugin!` always exports the same unmangled symbol; multiple static plugins need descriptor-only registration | Required for static/browser mode |
| Add a supported static runtime binding mode | The current facade explicitly links a runtime shared library | Required for static/browser mode |
| Support generic, unregistered implementation classes where needed | Lets a static façade implement reusable typed event adapters without generic ABI interfaces | After the first event adapter proves the need |
| Preserve/check error and ownership behavior through new adapters | Pointer validation, final release, invalid status normalization, and panic behavior remain COM responsibilities | Every COM change |

The recursive-interface issue is verified, not hypothetical. This fails to compile against the current nc-com checkout with E0391:

```rust
#[com_interface(auto_guid = "microui.plan.RecursiveNode:v1")]
pub trait IRecursiveNode {
    fn child(&self, index: u32)
        -> Result<ComPtr<IRecursiveNodeAbi>, NcStatus>;
}
```

The generated ABI hash includes hashes of referenced interfaces, and IID currently defaults to a projection of the complete interface identity. A self-reference or mutual reference creates a constant-evaluation cycle. The failure is preserved in `recursive-check.log` beside this plan. Source: [referenced-interface hashing](../crates/com/nc-com-macros/src/interface.rs#L617).

Since this is now an owned pre-alpha COM layer, I would fix this rather than shape every UI interface around the limitation. My preferred design to investigate is:

1. Generate an IID constant independently of the full interface hash.
2. Hash the local vtable contract. Referenced COM pointers contribute their IID and ownership/threading passing mode, rather than recursively embedding the referenced interface's complete hash.
3. Add interface identity metadata to the common vtable prefix, preferably a pointer to one shared static `NcInterfaceId` record.
4. Generated thunks validate every incoming COM pointer against the full identity expected by the receiving implementation; generated caller helpers validate owned COM return pointers before exposing a typed `ComPtr`.
5. On a rejected owned return, release the transferred reference through the common IUnknown prefix. Borrowed arguments are never released merely because validation failed.
6. Update core ABI generation/version checks, manual built-in string/byte interfaces, descriptor/hash validation, inspection, C fixtures and compatibility tests together.

This changes where transitive compatibility is checked; it does not remove that check. Simply dropping nested ABI hashes without checking the actual interface pointers at the call boundary would weaken safety and is not acceptable. The proposed metadata/checking change has not been implemented or benchmarked. It should be its own reviewed COM prerequisite with self-reference, mutual-reference, mismatched-argument, mismatched-return and release-count tests.

A cycle-aware hash over an explicitly declared interface graph is another possible design, but it requires considerably more contract-group/descriptor machinery. The local-hash plus checked-pointer approach is the simpler candidate to prototype first. Coherent capability interfaces should still be separate even after recursive references work.

For borrowed call helpers, reuse the existing generated call body for a borrowed pointer. Do not create another object model or require AddRef/Release just to perform a synchronous borrowed call.

For static plugins, a simple API would be descriptor generation plus an explicit export macro at the final dynamic-library root:

```rust,ignore
nc_com::plugin! { /* identity, dependencies, class descriptors */ }

#[cfg(feature = "dynamic-plugin")]
nc_com::export_plugin!(); // proposed addition
```

Both deployment modes use the same descriptor and class table. Only a dynamic library exports `nc_get_plugin_descriptor`. Source of the present unconditional export: [plugin macro](../crates/com/nc-com-macros/src/plugin.rs#L204).

The static runtime mode must reuse the existing catalog and API-table implementation. It must not create a second registry design. A native dynamic application links one runtime shared library; a fully static/browser application links one runtime implementation. Do not embed independent process-global registries in each plugin.

Generic implementation-class support must keep the interfaces monomorphic. A `ButtonSink<H>` may be a host-local monomorphization implementing the fixed `IButtonSink` ABI. Initially restrict generic COM classes to objects constructed locally and returned through interfaces; do not silently assign one registered CLSID to unrelated generic instantiations. Objects must own their state; generic lifetimes must not permit borrowed stack state to escape through an unqualified ComPtr.

**5. UI crate and plugin boundaries**

The proposed product crates are:

| Crate | Kind | Responsibility |
|---|---|---|
| `microui-contracts` | Small statically linked contract library | Interfaces, records, enums, IDs and status codes; preferably no_std |
| `microui-redux` | Statically linked Rust façade | Generic proxies, typed constructors, string/path conversion and scoped convenience APIs |
| `microui-ui` | UI plugin implementation | Existing retained engine, widgets, containers, menus, window manager and standard dialogs |
| `microui-platform` | Platform/render plugin assembly | Native host/window + selected backend and app GPU extension composition |
| Existing renderer crates | Private static implementation libraries | Glow/Vulkan/WGPU/WebGL algorithms and native resource management inside the platform binary |
| Existing renderer-support | Private static helper library | Shared renderer helpers; no independent plugin |
| `microui-assets` | Optional plugin implementation | Builder, PNG/font loading and optional theme JSON |
| Demo crates / xtask | Applications and tools | Application state, packaging, backend selection and validation |

This keeps three logical plugin groups. Normally the platform plugin is compiled with one selected renderer. If simultaneous backend plugins become a requirement, package backend-specific variants with distinct plugin identities using the same interfaces; do not require loading every backend into the ordinary demo.

The platform plugin deliberately combines OS-window and GPU ownership. Today a renderer receives Rust `sdl2::Window`, `Arc<glow::Context>`, or native WGPU/Ash types. Those cannot be passed through the proposed COM boundary as ordinary Rust values. Keep their creation and ownership in one binary. A native window/session object can expose both `IPlatformWindow` and `IRenderer` on the same COM identity. Multiple native windows create multiple such objects.

The reusable renderer crates still contain the backend implementation; the platform plugin supplies the actual host/window integration. An embedding application can provide a different platform plugin. Supporting pre-existing external native windows would require an explicit platform-specific native-surface capability and its lifetime contract; it should not be implemented as a public `usize`/void-pointer escape hatch.

Distinguish the OS window from microui's logical retained windows. Logical windows, child windows, dialogs and popups remain managed by the UI plugin's window manager.

Classes do not all need registry entries. Register roots that a host must activate—such as a UI runtime/factory, a platform factory, and an asset loader. A runtime-bound `IWidgetFactory` can return concrete button/slider/container COM objects directly. Returned objects still implement normal COM interfaces without giving every ephemeral object a public constructible class descriptor. A third-party plugin can expose its own typed factory or a constructible widget class.

**6. The static generic façade**

This is the requested generic/static-to-COM/dynamic split:

```rust
use nc_com::{
    ComInterface, ComPtr, NcCopyFromBufferOffset, NcStatus,
};

/// Statically linked typed proxy; it contains no widget implementation.
pub struct Widget<I: ComInterface> {
    /// One owned, hash-checked COM interface reference.
    interface: ComPtr<I>,
}

impl<I: ComInterface> Widget<I> {
    /// Accepts an interface obtained through checked activation or QueryInterface.
    pub fn from_interface(interface: ComPtr<I>) -> Self {
        Self { interface }
    }

    /// Queries the common node behavior when attaching the object to the UI.
    pub fn node_interface(&self) -> Result<ComPtr<IWidgetAbi>, NcStatus> {
        self.interface.query()
    }
}

/// Concrete public button name backed by the generic proxy implementation.
pub type Button = Widget<IButtonAbi>;

impl Widget<IButtonAbi> {
    /// Presents a Rust string API while using an explicit ABI buffer for the call.
    pub fn set_label(&self, label: &str) -> Result<(), NcStatus> {
        self.interface.set_label(NcCopyFromBufferOffset::new(
            0, label.as_bytes(),
        ))
    }
}
```

The corresponding contract is concrete and small:

```rust
use nc_com::{
    ComPtr, INcStringAbi, NcCopyFromBufferOffset, NcStatus, com_interface,
};

#[com_interface(auto_guid = "microui.plan.Button:v1")]
/// Button editing capability, independent of its implementation and event transport.
pub trait IButton {
    /// Replaces the complete UTF-8 label. Offset must be zero; copy before returning.
    fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>)
        -> Result<(), NcStatus>;

    /// Returns an owned immutable label snapshot.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
}

#[com_interface(auto_guid = "microui.plan.UiFactory:v1")]
/// Illustrative factory; the complete factory uses typed initialization records.
pub trait IUiFactory {
    /// Creates an unmounted button that the caller can configure and attach.
    fn create_button(&self) -> Result<ComPtr<IButtonAbi>, NcStatus>;
}
```

Specialized widget interfaces inherit the common contract (`IButton: IWidget`).
The class lists its specialized interface; nc-com supplies every base projection
in the same allocation. A concrete plugin class contains its private state directly:

```rust,ignore
#[com_class(interfaces(IButton, ILeafLayout, IButtonEvents))]
/// One button allocation; no legacy trait-object wrapper or public downcast is needed.
pub struct ButtonObject {
    /// Plugin-local semantic state and pending geometry invalidation.
    state: RefCell<ButtonState>,
    /// Typed queued submission state, containing no application callback.
    submitted: EventSource<ButtonSubmitted>,
}
```

`ButtonState`, `EventSource<E>` and their Rust fields are implementation helpers inside the plugin. Their layout is never part of the ABI. The façade does not contain a second button state, text-layout cache, event queue or shadow widget tree.

The complete compile-tested sketch is [src/lib.rs](/tmp/microui-com-plan-r10f3k23/src/lib.rs). It compiles the separate Widget/Children/Button/event capabilities and tests that a generic proxy edits a COM object, reads its text and observes the same geometry invalidation flag. This validates the contract/façade pattern with current nc-com; it is not a dynamic UI implementation or proof of the proposed nc-com ABI changes.

Generics may also remain in a local `ListModel<T>`, renderer implementation or callback adapter. Arbitrary T does not become a wire payload. A generic list model keeps T in the application and sends scoped typed row keys to the UI; its queue/removal policy must retain or reject stale rows consistently. If payload ownership must cross plugins, define a domain interface such as `IFileEntry` with explicit path operations. Do not introduce `IValue`, `get_property(string)`, `Any`, or an IUnknown payload bag as a substitute for typed contracts.

**7. Interface families and runtime ownership**

I would divide the public capabilities as follows:

| Family | Examples | State owner |
|---|---|---|
| UI coordination | `IUiRuntime`, `IWindowManager`, `IWidgetFactory` | One UI runtime instance |
| Common widget behavior | `IWidget` | Concrete widget/container object |
| Geometry | `ILeafLayout`, `IContainerLayout`, `IChildren` | Widget algorithm plus runtime-owned derived node state |
| Concrete editing | `IButton`, `ISlider`, `ITextbox`, `ILinear`, `IGrid` | Concrete object |
| Events | `IButtonEvents`, `IButtonSink`, `IEventSubscription` and equivalent typed families | Source queue, external subscription owner and application handler |
| Platform and rendering | `IPlatformWindow`, `IRenderer`, texture/render-item capabilities | Platform/GPU plugin |
| Resources | `IAssetBundle`, `IAssetLoader`, optional `IThemeLoader` | Immutable bundle or asset operation |

One object can implement multiple capabilities. In particular, a UI runtime can expose window management and widget construction on the same COM identity, with one authoritative forest. There is no need for an independently synchronized runtime tree and window-manager tree.

Keep one internal node record per mounted object: its COM widget reference, cached geometry interface capabilities, interaction state, committed layout and measurement cache. Cache queried capabilities when mounting instead of running QueryInterface during every update or paint. The former `Rc<RefCell<WidgetStorage<dyn Widget>>>` representation should be replaced, not permanently nested inside another COM object. Reuse existing algorithms as private methods/helper structs during conversion.

COM ownership and UI attachment are distinct. The application can keep a ComPtr after a window or subtree is removed. Specify the states explicitly:

- Constructed/unmounted: configuration is allowed.
- Mounted: one UI owner and one structural parent; the runtime owns a reference.
- Removed/closed: no routing, measurement, painting or new ordinary events; editing APIs return a documented closed-object status.
- Final COM release: frees the remaining allocation and resources.

For the first migration, retain mount-once semantics: removed nodes cannot be reattached. This matches the current tree's simple topology model and avoids introducing reparenting as a side effect of the COM conversion.

The runtime must reject duplicate mounting, cycles and cross-runtime attachment before publishing a topology mutation. Failure leaves the original tree unchanged and leaves the caller's owned reference usable. COM cloning does not authorize a second parent.

Do not assume that `static AtomicU64` in a statically linked helper crate is process-global across multiple shared libraries. IDs must be scoped to an authoritative owner and checked there, or allocated by one central service. Use canonical IUnknown identity when comparing live object identity; interface-header addresses for different interfaces on the same object can differ.

Avoid strong-reference cycles deliberately. Parent ownership is downward; back-links should be local Weak references, scoped IDs or a small state/capability object that does not own the tree. A widget must not retain the complete runtime merely to mark itself dirty. The existing terminal-event semantics must depend on logical closure, not waiting for COM reference count zero.

A general weak-COM extension is not required for the first migration. Existing Rc/Weak helpers may remain within a single plugin; they never cross the ABI. Add general weak COM only if a demonstrated cross-plugin ownership requirement needs it.

**8. Update, measurement, layout and painting**

Preserve the agreed geometry semantics:

- `update` returns Changed or NoChange for measurement/placement inputs.
- Programmatic setters record sticky geometry invalidation themselves.
- Resize constraints, skin revision, topology and child changes invalidate independently.
- NoChange does not mean no events, no paint or no descendant work.
- Application/widget authors remain responsible for externally mutated state, truthful change reporting, hidden/clipped updates and observational painting.

The COM migration removes the old typed-handle mutation hook, so it cannot silently remain responsible for dirtying setters. Put a small concrete `WidgetState` helper inside each implementation. Setters mark its pending change; update returns its own geometry result. A runtime dirty-collection pass drains pending setter changes, including nodes whose update was skipped, and propagates them to ancestors before committing layout. Check pending changes before returning a parent cache hit. No per-widget heap-allocated invalidator object is needed just for this flag.

Keep measurement constraints distinct from allocated rectangles. Translate the current finite/unbounded constraints, child participation, viewport, scroll transform and content extent to explicit records/tagged ABI values. Linear/grid sizing belongs to parent-child edges, not a generic property bag on every node.

External container implementations need bounded child-measurement and child-placement capabilities. They should be able to measure/place indexed children without borrowing runtime Node storage or editing topology during layout. Keep the existing algorithms and cache policy; move their private data to the new object representation rather than designing a new layout engine.

The difficult lifetime issue is that ComRef does not provide the same exclusive, non-retainable lifetime as `WidgetPaintCtx<'_>` or `ContainerLayoutCtx<'_>`. A callee can acquire an owned reference. A COM adapter must therefore never hide a stack `&mut Painter`, borrowed `Children`, or borrowed frame object behind a reference-counted object.

Use an owned recording/layout service and a non-reused invocation token for ephemeral authority:

```rust
#[com_struct]
/// Authority for one paint invocation; the recorder expires it when that call ends.
pub struct PaintScope {
    /// Non-reused invocation identity. Zero is never an active scope.
    pub invocation: u64,
}

#[com_interface(auto_guid = "microui.plan.Painter:v1")]
/// Recording service with owned storage and explicit invocation validation.
pub trait IPainter {
    /// Records a local rectangle while this exact invocation is active.
    fn fill(&self, scope: PaintScope, rect: Rect, rgba: u32)
        -> Result<(), NcStatus>;
}
```

The service is reusable; do not allocate a COM object per primitive. Private guards expire the scope on every exit. A retained old interface/token pair remains memory-safe and fails after its invocation. Tokens must not become valid again in a later frame. Layout uses the same principle with explicit nesting/parent restoration during recursive child calls.

The static façade can wrap the interface and token in `Painter<'a>` or `LayoutCtx<'a>` so widget authors write ordinary `painter.fill(...)` calls. The token is mechanical ABI plumbing; the façade hides it, while the implementation enforces it for direct COM callers too.

Release mutable RefCell/lock borrows before invoking another component. UI mutation during paint/layout or reentrant update must produce a clear contract error rather than unwind across the ABI. Normal setters and topology changes remain allowed at defined input/dispatch boundaries.

**9. Typed events and application state**

Application state stays in the statically linked application. The COM UI runtime does not become `UiRuntime<State>` and does not receive a raw pointer to generic Rust state.

Expose concrete event capabilities. For example:

```rust
#[com_interface(auto_guid = "microui.plan.ButtonSink:v1")]
/// Application-owned handler for one concrete event family.
pub trait IButtonSink {
    /// Receives a submission snapshot after traversal releases its mutable borrows.
    fn submitted(&self, label: ComRef<'_, INcStringAbi>)
        -> Result<(), NcStatus>;
}

#[com_interface(auto_guid = "microui.plan.ButtonEvents:v1")]
/// Typed subscription endpoint implemented by a button.
pub trait IButtonEvents {
    /// Connects the exclusive subscriber and returns its cancellation/ownership lease.
    fn subscribe(&self, sink: ComRef<'_, IButtonSinkAbi>)
        -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus>;
}
```

Source queues contain event snapshots, not application callback ownership. A subscription owns its typed sink and queue reference. The statically linked event session/driver owns subscriptions. The UI runtime borrows the event dispatcher while processing an input transaction and does not retain it. This permits handlers to retain UI capabilities without the runtime owning a path back to itself through the handler.

Keep deterministic registration-order sweeps, FIFO order per source, the finite cascade limit, exclusive subscription, silent programmatic setters, and the existing distinction between ordinary events and one terminal fact. Dispatch snapshots before invoking handlers, and never invoke application code with a producer or tree borrow held.

A generic static adapter can use an owned `Rc<RefCell<State>>` and a concrete handler trait. If closure convenience is retained for the previously approved callback API, the closure stays in that local adapter. Neither closure layout nor a borrowed `&mut State` crosses the ABI. Adapter code must not manufacture a ComPtr whose implementation borrows stack state that a callee can retain.

Dropping or cancelling a subscription disconnects without calling user code. Source closure discards ordinary pending events and retains the documented terminal fact. The event session can deliver that fact after the producer has logically closed. A bare COM Release should not become a substitute for an explicit UI close operation.

File dialogs retain native path semantics. Do not convert PathBuf through a UTF-8 label. Define an owned path capability with explicit platform encoding (Unix native bytes; Windows UTF-16 code units) and separate display text. The Rust façade presents Path/PathBuf. Keep the existing transactional directory reads, lossy-display selection preservation, hide/destruction cancellation and request completion tests.

**10. Rendering and application GPU work**

The common dynamic renderer boundary should submit a whole prepared frame rather than make a COM call per vertex, glyph or triangle:

```rust,ignore
#[com_interface(auto_guid = "microui.Renderer:v1")]
/// Backend owning native GPU resources and presentation.
pub trait IRenderer {
    /// Executes one complete frame and reports submission/presentation failure.
    fn render(&self, info: FrameInfo, commands: ComRef<'_, IRenderStreamAbi>)
        -> Result<(), NcStatus>;
}
```

`IRenderStream` is a bounded, typed view of ordered batches, vertex buffers, resource references and custom render items. Use existing explicit copy-buffer modes for bulk reads into reusable backend-owned storage. Fixed-size render records belong in the contract crate. Do not send serialized arbitrary commands or Rust display-list enums through byte blobs.

Keep high-level text/nine-patch expansion, clipping and tessellation in the UI plugin so each backend consumes the same final geometry and ordering. Preserve the source display-list ordering across UI batches, images and custom draws. Keep buffer allocation reuse and batching; benchmark boundary call and copy counts before selecting chunk sizes.

Prefer a synchronous complete-frame COM method. The backend can keep its existing native `RendererFrame<'_>` and consuming `finish(self)` privately inside that call. The Rust façade can also retain a consuming `Frame<'a>` wrapper for application ergonomics. This avoids exposing a cloneable COM frame merely to model one submission. If an external COM frame becomes necessary, it needs an explicit Open/Finished/Abandoned state and repeat-call rejection; dropping it must abandon, never submit.

Textures and render items are owned capabilities. Validate their backend/owner before use, retain resources for as long as queued GPU work requires, and preserve the native window/context lifetime until GPU destruction finishes. Atlas replacement remains transactional: validate logical resource identities and prepare upload before publishing the new bundle.

The most substantial renderer change is application GPU work. The current API lends `&mut wgpu::RenderPass`, `wgpu::CommandEncoder`, `Arc<glow::Context>` and Ash state. A static generic façade does not make these Rust-owned objects ABI-safe between independent binaries. Keep backend-specific application rendering code in the same platform/GPU plugin as the backend. Its concrete Rust command enums and associated types remain private.

Expose application-specific scene interfaces or render-item capabilities outward. For example, the full demo's GPU plugin can expose a typed scene interface for mesh upload/camera updates and return a render item that the UI inserts at the custom-widget position. Its private renderer still performs actual native 3D preparation and rendering. This preserves native GPU functionality rather than replacing it with CPU-generated UI triangles.

An independently loaded GPU extension that must access native internals would need an explicitly designed backend-specific C-shaped capability API. WGPU's Rust RenderPass is not such an API. Do not introduce that large secondary abstraction merely to complete the initial COM migration.

**11. Assets, themes and value types**

Keep JSON parsing and font/PNG construction in the optional asset plugin. The size investigation found roughly 167 KiB attributable to bundled JSON theme loading in the current demo; loading this code only when requested is a useful packaging boundary, but dynamic libraries still count toward total installed size.

A runtime asset bundle should be immutable and expose bulk typed metadata plus owned pixel data. The UI imports/caches font glyph tables, icon metadata and resolved theme roles once per bundle revision; avoid a COM call for every glyph during normal measurement or drawing. The builder and JSON loader produce the same runtime bundle form. Prebuilt tools produce that form without linking fontdue/Serde into the shipped UI plugin.

Use shared ABI-safe small values as canonical UI geometry/color/input records wherever practical. Avoid parallel Rect/ComRect/ForeignRect families and repetitive conversion layers. Keep rs-math3d-specific 3D types local to the demo/GPU implementation, or give app-specific scene records an explicit contract representation.

`INcString` is UTF-8 text, not a native filesystem path. Buffers specify element type, offset semantics, limits and validity for the call. Renderer/asset loaders must validate dimensions, multiplication overflow, counts, resource ownership, enum tags and UTF-8 before mutating live state.

Recoverable failures use NcStatus with documented UI-specific negative codes; rich diagnostics can use an owned typed diagnostic interface. Preserve backend error details through that diagnostic path. Panics indicate implementation faults and abort under nc-com's existing policy; do not promise exception recovery from arbitrary plugin code.

**12. Native build, packaging and browser support**

Use xtask or an equivalent outer build driver to:

1. Build the matching nc-com runtime library for the chosen target/profile.
2. Build implementation plugins with the desired backend/asset features.
3. Build the application/static façade with the runtime link directory supplied.
4. Inspect descriptors and write an explicit dependency-first plugin load list.
5. Stage the executable, runtime library, selected plugins and assets in one install layout.
6. Run the packaged executable from that layout, including outside the source directory.

Keep this orchestration outside build.rs. Do not restore nested Cargo builds. Native runtime search paths and import libraries must be packaged correctly on Linux/macOS/Windows; library build order alone does not make a runtime shared library discoverable by the OS loader.

Plugins have declarative descriptors; startup constructs explicit root services. Loading dependencies is host policy, not a plugin initializer that recursively loads more modules. Keep required-plugin failure behavior explicit, and use offline inspection for diagnostics before activation. Hot unloading/reloading is not part of this migration.

For the browser, statically register the required descriptors and use one statically linked runtime implementation. The WebGL/DOM platform implementation uses the same UI interfaces. No dlopen or native shared-library discovery is involved. If that mode cannot be completed early, make it an explicit milestone with a working browser gate, not an unnoticed regression in the native conversion.

**13. Incremental implementation and commits**

Use a fresh implementation branch after this plan is accepted. Keep each cohesive item in its own descriptive commit, with the promised comments on public/private definitions and the non-obvious parts of bodies.

| Order | Commit-sized item | Required evidence |
|---:|---|---|
| 1 | Import nc-com crates, documentation, fixtures and workspace metadata | Existing core/macros/runtime tests and imported package builds |
| 2 | Add recursive-interface support with complete pointer identity validation | Self/mutual-reference success, mismatched argument/return rejection, ownership counts, native C and ABI fixtures |
| 3 | Add borrowed-interface helper calls | Calls through ComRef perform no ownership transfer; lifetime/compile tests |
| 4 | Separate descriptor generation and dynamic export | Two plugins statically linked together; independent dynamic loading still works |
| 5 | Add static runtime binding mode | One shared catalog in static mode; native shared mode unchanged; browser-target compile |
| 6 | Introduce microui-contracts and the thin generic façade | Compile tests for records, proxy typing, class/IID mismatch and prohibited ABI types |
| 7 | Establish a real dynamic vertical slice | Platform window, renderer, UI runtime and one button work across separately built binaries |
| 8 | Move runtime/window-manager ownership to COM objects | Window hierarchy, layering, modal/popup policy, destruction and ownership tests |
| 9 | Migrate common widget state, input and geometry invalidation | Update/Changed, setter invalidation, resize, hidden-node and stale-cache regressions |
| 10 | Migrate built-in leaves and concrete editing interfaces | Existing widget/input behavior tests through the façade |
| 11 | Migrate container layout and structural edits | Linear/grid/scroll/disclosure tests, duplicate mount/cycle rejection and transactional edits |
| 12 | Migrate typed events and application adapters | FIFO/cascade/lifecycle/cancellation/reentrancy tests; add generic local COM adapters only if required |
| 13 | Migrate menus and file dialogs completely | Existing native-path, request completion, popup and keyboard tests |
| 14 | Finish all renderer/application-extension integrations | Glow, Vulkan, WGPU and custom 3D demo behavior; ownership and explicit finish/error checks |
| 15 | Split optional assets/theme loading and update exporters | Runtime/prebuilt equivalence, missing resource rejection, optional JSON size/build checks |
| 16 | Complete browser/packaging workflows | Static WebGL demo, browser input/IME checks, staged native startup outside the repo |
| 17 | Remove transitional APIs and representations; finish documentation and budgets | No obsolete core dependency through the façade; final feature/test/size/performance matrix |

The vertical slice is a design gate. It should exercise real separate-binary loading, mutation, input, measurement, rendering and teardown before converting all widgets. A temporary adapter may keep the branch buildable, but give it a specific removal milestone. Do not preserve two authoritative trees or two implementations of every control as the final architecture.

The migration is substantial production work. Most layout, text editing, painting, clipping and backend algorithms can be reused. The ownership and public integration APIs cannot be replaced by just adding attributes to current structs. Measure the slice before estimating the remaining effort or promising a smaller implementation.

**14. Acceptance tests and documentation obligations**

Run existing UI behavior tests against the new façade and add tests where the new boundary creates a new failure mode:

- A separately compiled custom widget plugin, with no dependency on microui-ui implementation internals, measures, updates, paints, emits events and attaches as a child.
- A separately compiled custom container controls child layout through the bounded capability API.
- Two independent UI runtime instances share the COM registry but not windows, focus, styles, captures, resources or event queues.
- Foreign-runtime widgets and renderer resources are rejected.
- Destroyed windows/subtrees become logically closed even when an application retains ComPtr handles.
- Subscription cancellation and terminal delivery do not form strong-reference cycles.
- Reentrant updates and retained old paint/layout capabilities fail safely; stale scopes never become valid again.
- Wrong interface hashes, recursive interface references, malformed enum tags, invalid UTF-8 and invalid copy-buffer counts are rejected at their respective boundaries.
- Every transferred COM reference is released exactly once on success, failure and rejected-return paths.
- Plugin class/interface names remain stable across ordinary Rust module reorganizations; breaking draft ABI changes are deliberate and update their validation fixtures.
- Native GPU renderers retain window/context ownership and preserve custom draw ordering, depth, clip/state restoration and explicit completion errors.
- Static/browser and native dynamic feature selections both build independently.

Size/performance reporting must include executable size, each shipped shared library, total installed size, loaded memory, startup/activation time, steady-state allocations and AddRef/Release counts. Also measure update/layout/render time for deep and wide trees. A smaller executable alone does not prove a smaller system: class descriptors retain code, dynamic boundaries limit LTO, and helper/std code can be duplicated between libraries.

Document the following as explicit component/app-author contracts: UI thread affinity; recoverable error versus panic behavior; source and callback ownership; logical closure versus final Release; mount-once/one-parent rules; update change reporting; external shared-state synchronization; hidden/clipped update limitations; observational painting; scope expiry; native GPU extension boundaries; native path encoding; required plugin startup order; and no dynamic unload/hot reload.

**15. What was verified during planning**

- Read the nc-com macros, object pointers, runtime binding/loader code, contract example, and relevant microui ownership/layout/rendering interfaces.
- Reproduced the recursive-interface E0391 compiler failure against nc-com at the analyzed revision.
- Compiled and ran the generic façade/COM button test successfully using current nc-com. This checks a concrete COM object and proxy in one binary; actual separate-binary UI integration remains the vertical-slice milestone.
- Confirmed the unconditional plugin export and shared-runtime-only facade from source.
- No implementation files or branch state in either repository were changed by this planning work. Probes and this document are under `/tmp/microui-com-plan-r10f3k23`.
