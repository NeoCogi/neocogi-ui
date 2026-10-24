//! Owned nested layout scopes; neither COM storage nor retained clients borrow a runtime stack.
use crate::boundary::local_rect;
use super::HostState;
use neocogi_ui::abi::core::*;
use neocogi_com::{ComPtr, NcStatus, NC_E_INVALID_ARG, com_object};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use super::{child_store::ChildStore, SharedAssets};
use crate::Dimensioni;
use crate::ui_node::LayoutTraversal;

/// One invocation retains collection identity; it never moves children out of their owner.
struct Frame {
    /// The editor remains the owner. Each service operation takes a short
    /// transaction, returning children before calling parent policy again.
    children: Rc<ChildStore>,
    /// Placement-only recursion; None restricts this scope to measurement.
    placement: Option<LayoutTraversal>,
}

/// Retainable service storage contains only owned frames, resources and token state.
struct Storage {
    /// Zero outside a layout call; nested calls temporarily replace the parent's token.
    active: Cell<u64>,
    /// Monotonic authority never wraps to an old retained token.
    next: Cell<u64>,
    /// Methods take the frame before calling a child, permitting safe nested scopes.
    frame: RefCell<Option<Frame>>,
    /// Current imported assets borrowed consistently while measuring or placing one child.
    resources: SharedAssets,
    /// First-error authority shared with retained geometry caches.
    host: Rc<HostState>,
}

impl Storage {
    /// Validates scope before taking ownership, so stale callers cannot interfere with a later pass.
    fn take(&self, scope: LayoutScope) -> Result<Access<'_>, NcStatus> {
        if scope.invocation == 0 || scope.invocation != self.active.get() {
            return Err(UI_E_EXPIRED_SCOPE);
        }
        let frame = self.frame.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_BUSY)?;
        Ok(Access { storage: self, frame: Some(frame) })
    }

    /// Converts private copied geometry only after checking recursive component failures.
    fn size(&self, size: Dimensioni) -> Result<Size, NcStatus> {
        if let Some(error) = self.host.error.get() {
            return Err(error);
        }
        Ok(Size { width: size.width, height: size.height })
    }
}

/// One method owns its frame without holding a RefCell guard across child COM calls.
struct Access<'a> {
    /// Destination slot remains valid through the service's synchronous method call.
    storage: &'a Storage,
    /// Parent frame is absent from shared storage until nested recursion has completed.
    frame: Option<Frame>,
}
impl Access<'_> {
    /// Returns local owned state; failure to own it would be an internal guard invariant violation.
    fn frame(&mut self) -> &mut Frame {
        self.frame.as_mut().expect("active layout method owns its frame")
    }
    /// Rejects placement methods during a measurement invocation before any derived-state change.
    fn placement(&mut self) -> Result<&mut Frame, NcStatus> {
        let frame = self.frame();
        if frame.placement.is_none() { Err(UI_E_BUSY) } else { Ok(frame) }
    }
}
impl Drop for Access<'_> {
    /// Nested invocation guards restore the vacant slot before this parent frame returns.
    fn drop(&mut self) {
        *self.storage.frame.borrow_mut() = self.frame.take();
    }
}

#[com_object(interfaces(ILayoutService))]
/// Concrete geometry service; retaining it retains resources but no caller stack.
struct Layout {
    /// Reused scope storage shared with its private driver.
    storage: Rc<Storage>,
}
impl ILayoutService for Layout {
    /// Reads cardinality from this invocation's actual child sequence.
    fn child_count(&self, scope: LayoutScope) -> Result<u32, NcStatus> {
        let mut access = self.storage.take(scope)?;
        Ok(access.frame().children.take()?.len() as u32)
    }
    /// Measures through the same node cache used by ordinary runtime traversal.
    fn measure_child(&self, scope: LayoutScope, index: u32, limits: LayoutLimits) -> Result<Size, NcStatus> {
        let mut access = self.storage.take(scope)?;
        let constraints = constraints(limits)?;
        let mut children = access.frame().children.take()?;
        let node = children.get_mut(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        // Child measurement invokes foreign widgets; retain the local immutable value
        // and release the cache borrow before descending through their COM gates.
        let resources = self.storage.resources.borrow().clone();
        let size = node.measure(resources.bundle().skin(), resources.bundle().atlas(), constraints)?;
        self.storage.size(size)
    }
    /// Recurses with owned child storage and no runtime or node reference stored in COM.
    fn place_child(&self, scope: LayoutScope, index: u32, rect: Rect) -> Result<Size, NcStatus> {
        let mut access = self.storage.take(scope)?;
        if rect.width < 0 || rect.height < 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let frame = access.placement()?;
        let mut children = frame.children.take()?;
        let node = children.get_mut(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        // Placement may recurse into foreign widgets with independent receivers.
        let resources = self.storage.resources.borrow().clone();
        let size = frame.placement.as_mut().expect("placement was checked").layout_node_ref(
            node,
            resources.bundle().skin(),
            resources.bundle().atlas(),
            local_rect(rect),
        )?;
        self.storage.size(size)
    }
    /// Exposes only a copied extent, never the child's runtime layout cache.
    fn child_content_size(&self, scope: LayoutScope, index: u32) -> Result<Size, NcStatus> {
        let mut access = self.storage.take(scope)?;
        let children = access.placement()?.children.take()?;
        let node = children.get(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        self.storage.size(node.state.layout.content_size)
    }
    /// Publishes responsive participation only during placement of the direct parent.
    fn set_participation(&self, scope: LayoutScope, index: u32, participation: LayoutParticipation) -> Result<(), NcStatus> {
        let mut access = self.storage.take(scope)?;
        let participation = match participation {
            LayoutParticipation::Active => crate::ChildParticipation::Active,
            LayoutParticipation::Disabled => crate::ChildParticipation::Disabled,
            LayoutParticipation::Hidden => crate::ChildParticipation::Hidden,
            _ => return Err(NC_E_INVALID_ARG),
        };
        let mut children = access.placement()?.children.take()?;
        let node = children.get_mut(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        node.state.participation = participation;
        Ok(())
    }
    /// Records a changed shared placement dependency without exposing invalidation internals.
    fn child_placement_changed(&self, scope: LayoutScope, index: u32) -> Result<(), NcStatus> {
        let mut access = self.storage.take(scope)?;
        let mut children = access.placement()?.children.take()?;
        let node = children.get_mut(index as usize).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        node.state.invalidate_layout();
        Ok(())
    }
}

/// Private driver shares one service allocation across the runtime's complete tree.
#[derive(Clone)]
pub(crate) struct LayoutService {
    /// Actual owned authority and frame storage.
    storage: Rc<Storage>,
    /// Borrowed projection passed during calls, without a refcount operation per child query.
    service: ComPtr<ILayoutServiceAbi>,
}
impl LayoutService {
    /// Allocates one service observing the runtime's single replaceable asset value.
    pub(crate) fn new(resources: SharedAssets, host: Rc<HostState>) -> Self {
        let storage = Rc::new(Storage {
            active: Cell::new(0),
            next: Cell::new(0),
            frame: RefCell::new(None),
            resources,
            host,
        });
        let service = Layout::from_value(Layout { storage: storage.clone() })
            .query()
            .expect("Layout declares ILayoutService");
        Self { storage, service }
    }

    /// Installs an owned nested scope; the previous token/frame is restored even on errors.
    fn begin(&self, children: &Rc<ChildStore>, placement: Option<LayoutTraversal>) -> Result<Invocation<'_>, NcStatus> {
        let token = self.storage.next.get().checked_add(1).ok_or(neocogi_com::NC_E_OUT_OF_RANGE)?;
        children.open()?;
        let frame = Frame { children: children.clone(), placement };
        let previous = self.storage.frame.borrow_mut().replace(frame);
        let previous_token = self.storage.active.replace(token);
        self.storage.next.set(token);
        Ok(Invocation {
            storage: &self.storage,
            previous,
            previous_token,
            scope: LayoutScope { invocation: token },
        })
    }

    /// Measures the external policy with an owned frame, expiring its scope before returning.
    pub(super) fn measure(
        &self,
        children: &Rc<ChildStore>,
        policy: &neocogi_com::WeakComBox<IContainerLayoutAbi>,
        bounds: LayoutLimits,
    ) -> Result<Size, NcStatus> {
        let guard = self.begin(children, None)?;
        let mut policy = policy.clone();
        let result = policy.measure(guard.scope, bounds, self.service.as_com_ref());
        // A component may accidentally ignore a failed child call; its error still prevents caching.
        result.and_then(|size| {
            children.open()?;
            self.storage.size(Dimensioni::new(size.width, size.height))
        })
    }

    /// Placement returns one copied output instead of exposing parent geometry through service setters.
    pub(super) fn place(
        &self,
        children: &Rc<ChildStore>,
        policy: &neocogi_com::WeakComBox<IContainerLayoutAbi>,
        traversal: &LayoutTraversal,
        rect: Rect,
    ) -> Result<ContainerGeometry, NcStatus> {
        let guard = self.begin(children, Some(traversal.clone()))?;
        let mut policy = policy.clone();
        let geometry = policy.place(guard.scope, rect, self.service.as_com_ref())?;
        children.open()?;
        geometry.validate()?;
        if let Some(error) = self.storage.host.error.get() {
            return Err(error);
        }
        Ok(geometry)
    }
}

/// Scope cleanup owns the saved frame, never a pointer into an outer stack frame.
struct Invocation<'a> {
    /// Only the private synchronous guard borrows storage; COM owns its Rc independently.
    storage: &'a Storage,
    /// Previous owned frame, absent while a recursive service method owns its parent frame.
    previous: Option<Frame>,
    /// Parent authority becomes valid again only after the child scope ends.
    previous_token: u64,
    /// Exact token passed to the current external container.
    scope: LayoutScope,
}
impl Drop for Invocation<'_> {
    /// Expires current authority before releasing collection identity or restoring its parent.
    fn drop(&mut self) {
        self.storage.active.set(self.previous_token);
        let current = std::mem::replace(&mut *self.storage.frame.borrow_mut(), self.previous.take());
        // Frames own no child widgets; releasing scope metadata cannot defer their destruction.
        drop(current);
    }
}

pub(crate) use neocogi_ui_support::layout::{constraints, limits};
