//! COM menu declarations transfer the existing compact rows into the manager's sole surface forest.
use super::*;
use crate::menu::{Menu, MenuEntry, MenuItemParameters};
use nc_com::{INcStringAbi, NcString};
use std::rc::Weak;

/// One existing native declaration tree, shared only by its scoped construction capabilities.
pub(super) struct Declaration {
    /// Consumed exactly once; afterward the manager owns every native row and source.
    menu: RefCell<Option<Menu>>,
    /// Root COM destruction expires authentication even while child builders retain this allocation.
    alive: Cell<bool>,
}

#[com_object(interfaces(IMenuBuilder))]
/// A root or child construction capability addresses one subtree in the same declaration.
struct Builder {
    /// Runtime ownership is weak; unused builders cannot prolong the engine or its forest.
    runtime: Weak<RuntimeStorage>,
    /// Sole native declaration data, never a second copy of mounted topology.
    declaration: Rc<Declaration>,
    /// Immutable nested submenu positions; append-only construction keeps these positions stable.
    path: Vec<usize>,
}
impl Drop for Builder {
    /// Dropping the root abandons an unconsumed declaration and closes its native item queues.
    fn drop(&mut self) {
        if self.path.is_empty() {
            self.declaration.alive.set(false);
            let retired = self.declaration.menu.borrow_mut().take();
            drop(retired);
        }
    }
}
impl Builder {
    /// Checks logical lifetime and mutation policy before borrowing declaration storage.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        let runtime = self.runtime.upgrade().ok_or(UI_E_CLOSED)?;
        runtime.factory.host.check_mutation()?;
        if !self.declaration.alive.get() {
            return Err(UI_E_CLOSED);
        }
        Ok(runtime)
    }
}
impl IMenuBuilder for Builder {
    /// Appends one uniquely owned native record and publishes only its typed editing/event projection.
    fn item(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IMenuItemAbi>, NcStatus> {
        let runtime = self.owner()?;
        let label = text(input)?;
        let (handle, item) = crate::MenuItem::create(MenuItemParameters::new(label));
        let slot = {
            let mut data = self.declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
            let menu = menu_at(data.as_mut().ok_or(UI_E_CLOSED)?, &self.path)?;
            let slot = menu.entries.len();
            menu.entries.push(MenuEntry::Item(item));
            slot
        };
        Ok(Item::from_value(Item {
            runtime: Rc::downgrade(&runtime),
            declaration: Rc::downgrade(&self.declaration),
            path: self.path.clone(),
            slot,
            handle,
        })
        .query()
        .expect("menu item declares IMenuItem"))
    }
    /// Preserves separators as direct compact rows with no event source or widget allocation.
    fn separator(&self) -> Result<(), NcStatus> {
        self.owner()?;
        let mut data = self.declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        menu_at(data.as_mut().ok_or(UI_E_CLOSED)?, &self.path)?.entries.push(MenuEntry::Separator);
        Ok(())
    }
    /// Extends the existing declaration recursively and returns a scoped append-only child capability.
    fn submenu(&self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IMenuBuilderAbi>, NcStatus> {
        self.owner()?;
        let label = text(input)?;
        let mut path = self.path.clone();
        {
            let mut data = self.declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
            let menu = menu_at(data.as_mut().ok_or(UI_E_CLOSED)?, &path)?;
            path.push(menu.entries.len());
            menu.entries.push(MenuEntry::Submenu(Menu::new(label)));
        }
        Ok(Builder::from_value(Builder {
            runtime: self.runtime.clone(),
            declaration: self.declaration.clone(),
            path,
        })
        .query()
        .expect("child builder declares IMenuBuilder"))
    }
}

/// Resolves only construction-owned submenu positions; live topology remains exclusively in the manager.
fn menu_at<'a>(mut menu: &'a mut Menu, path: &[usize]) -> Result<&'a mut Menu, NcStatus> {
    for index in path {
        menu = match menu.entries.get_mut(*index) {
            Some(MenuEntry::Submenu(menu)) => menu,
            _ => return Err(UI_E_CLOSED),
        };
    }
    Ok(menu)
}
/// Resolves an item's immutable declaration location without exposing numeric identities across COM.
fn item_at<'a>(menu: &'a mut Menu, path: &[usize], slot: usize) -> Result<&'a mut MenuItemParameters, NcStatus> {
    match menu_at(menu, path)?.entries.get_mut(slot) {
        Some(MenuEntry::Item(item)) => Ok(&mut item.record.parameters),
        _ => Err(UI_E_CLOSED),
    }
}

#[com_object(interfaces(IMenuItem, IActivationEvents, IFontStyle))]
/// Menu rows expose direct live editing without synthesizing independent widget or container subtrees.
struct Item {
    /// Live manager resolution never retains the runtime through a row capability.
    runtime: Weak<RuntimeStorage>,
    /// Before mounting, edits address the uniquely owned declaration; after transfer this may expire.
    declaration: Weak<Declaration>,
    /// Immutable pre-mount submenu path, unused after the actual native record moves into the forest.
    path: Vec<usize>,
    /// Immutable direct-row position within that declaration subtree.
    slot: usize,
    /// The same typed native identity and weak event endpoint survive declaration consumption.
    handle: crate::MenuItemHandle,
}
/// Concrete edits replace the callback-shaped mutation helper formerly required by application code.
enum Edit {
    /// Whole validated display label.
    Label(String),
    /// Pointer and keyboard eligibility.
    Enabled(bool),
    /// Complete persistent marker role.
    Mark(crate::MenuItemMark),
    /// Optional display-only shortcut hint.
    Hint(Option<String>),
    /// Already validated semantic or named font selection.
    Font(crate::FontRef),
}
impl Edit {
    /// Applies one fully validated edit to whichever owner currently holds the sole native record.
    fn apply(self, parameters: &mut MenuItemParameters) {
        match self {
            Self::Label(value) => parameters.label = value,
            Self::Enabled(value) => parameters.enabled = value,
            Self::Mark(value) => parameters.mark = value,
            Self::Hint(value) => parameters.shortcut_hint = value,
            Self::Font(value) => parameters.font = value,
        }
    }
}
impl Item {
    /// Upgrades only for one operation and rejects retained capabilities after runtime closure.
    fn owner(&self) -> Result<Rc<RuntimeStorage>, NcStatus> {
        let runtime = self.runtime.upgrade().ok_or(UI_E_CLOSED)?;
        runtime.open()?;
        Ok(runtime)
    }
    /// Copies complete presentation state while holding no borrowed engine state across COM calls.
    fn read(&self) -> Result<MenuItemParameters, NcStatus> {
        let runtime = self.owner()?;
        if let Some(declaration) = self.declaration.upgrade() {
            let mut data = declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
            if let Some(menu) = data.as_mut() {
                return Ok(item_at(menu, &self.path, self.slot)?.clone());
            }
        }
        Ok(runtime.context()?.window_manager.menu_item(&self.handle).map_err(|_| UI_E_CLOSED)?.clone())
    }
    /// Silent edits target the declaration or the authoritative live row, with one shared invalidation boundary.
    fn edit(&self, edit: Edit) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        runtime.factory.host.check_mutation()?;
        if let Some(declaration) = self.declaration.upgrade() {
            let mut data = declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
            if let Some(menu) = data.as_mut() {
                edit.apply(item_at(menu, &self.path, self.slot)?);
                return Ok(());
            }
        }
        edit.apply(runtime.context()?.window_manager.menu_item_mut(&self.handle).map_err(|_| UI_E_CLOSED)?);
        runtime.pending.set(true);
        Ok(())
    }
}
impl IMenuItem for Item {
    /// Produces an independently owned immutable label snapshot.
    fn label(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.read()?.label))
    }
    /// Validates UTF-8 before changing the declaration or live surface.
    fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        self.edit(Edit::Label(text(label)?.to_owned()))
    }
    /// Reads actual retained eligibility, including edits applied after mounting.
    fn enabled(&self) -> Result<bool, NcStatus> {
        Ok(self.read()?.enabled)
    }
    /// Disabled rows remain in the same compact measurement and paint sequence.
    fn set_enabled(&self, enabled: bool) -> Result<(), NcStatus> {
        self.edit(Edit::Enabled(enabled))
    }
    /// Encodes the exact native persistent role without conflating unchecked and unmarked.
    fn mark(&self) -> Result<MenuMarkAbi, NcStatus> {
        Ok(match self.read()?.mark {
            crate::MenuItemMark::None => MenuMark::None,
            crate::MenuItemMark::Checked(checked) => MenuMark::Checked { checked },
            crate::MenuItemMark::Radio(selected) => MenuMark::Radio { selected },
        }
        .into())
    }
    /// Validates a complete concrete marker before applying its presentation change.
    fn set_mark(&self, mark: MenuMarkAbi) -> Result<(), NcStatus> {
        self.edit(Edit::Mark(match MenuMark::try_from(mark)? {
            MenuMark::None => crate::MenuItemMark::None,
            MenuMark::Checked { checked } => crate::MenuItemMark::Checked(checked),
            MenuMark::Radio { selected } => crate::MenuItemMark::Radio(selected),
        }))
    }
    /// Empty text represents the absence of a presentation-only hint.
    fn shortcut_hint(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(self.read()?.shortcut_hint.as_deref().unwrap_or("")))
    }
    /// Installs display text only; application shortcut behavior remains explicit.
    fn set_shortcut_hint(&self, hint: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        let hint = text(hint)?;
        self.edit(Edit::Hint((!hint.is_empty()).then(|| hint.to_owned())))
    }
}
impl IFontStyle for Item {
    /// Uses the same validated font-role mapping as ordinary COM text widgets.
    fn set_font_role(&mut self, role: TextFontRole) -> Result<(), NcStatus> {
        self.edit(Edit::Font(crate::FontRef::Role(super::super::widget::local_font_role(role)?)))
    }
    /// Resolves a named font once against the exact owning runtime resource bundle.
    fn set_named_font(&mut self, input: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        let runtime = self.owner()?;
        let name = text(input)?;
        let font = runtime.factory.resources.current().original().font_id(name).ok_or(nc_com::NC_E_NOT_FOUND)?;
        self.edit(Edit::Font(crate::FontRef::Named(font)))
    }
}
impl IActivationEvents for Item {
    /// Subscription uses the actual native record's source before or after mounting; no callback is retained by the row.
    fn subscribe(&self, sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
        self.owner()?.factory.host.check_mutation()?;
        super::super::events::subscribe(self.handle.submitted(), sink)
    }
}

impl RuntimeStorage {
    /// Authenticates a root builder by canonical COM identity and live weak registration before taking native data.
    fn menu_declaration(&self, menu: ComRef<'_, IMenuBuilderAbi>) -> Result<Rc<Declaration>, NcStatus> {
        let identity: ComPtr<IUnknown> = menu.add_ref().query()?;
        let key = identity.as_ptr() as usize;
        self.menus
            .borrow()
            .iter()
            .find_map(|(candidate, declaration)| {
                if *candidate != key {
                    return None;
                }
                declaration.upgrade().filter(|declaration| declaration.alive.get())
            })
            .ok_or(UI_E_OWNER)
    }
}
impl IMenus for Runtime {
    /// Creates one unique declaration root and registers only weak construction authority.
    fn create(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<ComPtr<IMenuBuilderAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let declaration = Rc::new(Declaration {
            menu: RefCell::new(Some(Menu::new(text(label)?))),
            alive: Cell::new(true),
        });
        let identity = Builder::from_value(Builder {
            runtime: Rc::downgrade(&self.storage),
            declaration: declaration.clone(),
            path: Vec::new(),
        });
        let mut registry = self.storage.menus.borrow_mut();
        registry.retain(|(_, state)| state.upgrade().is_some_and(|state| state.alive.get()));
        registry.push((identity.as_ptr() as usize, Rc::downgrade(&declaration)));
        drop(registry);
        identity.query()
    }
    /// Validates the complete bar shape and destination before consuming any declaration ownership.
    fn attach_bar(&self, window: ComRef<'_, IWindowAbi>, menu: ComRef<'_, IMenuBuilderAbi>) -> Result<(), NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parent = self.storage.parent(window)?;
        let declaration = self.storage.menu_declaration(menu)?;
        let mut context = self.storage.context()?;
        if context.window_manager.has_menu_bar(&parent).map_err(windows::status)? {
            return Err(UI_E_POLICY);
        }
        let mut data = declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?;
        let menu = data.as_ref().ok_or(UI_E_CLOSED)?;
        if menu.entries.iter().any(|entry| !matches!(entry, MenuEntry::Submenu(_))) {
            return Err(NC_E_INVALID_ARG);
        }
        let (_, entries) = data.take().expect("validated declaration remains owned").into_parts();
        drop(data);
        let menus = entries.into_iter().map(|entry| match entry {
            MenuEntry::Submenu(menu) => menu,
            _ => unreachable!("validated bar contains only submenus"),
        });
        context
            .window_manager
            .attach_menu_bar(&parent, crate::MenuBar::new(menus))
            .map_err(windows::status)?;
        self.storage.pending.set(true);
        Ok(())
    }
    /// Consumes one root declaration into the manager's existing standalone-popup and nested-menu registration.
    fn create_popup(&self, window: ComRef<'_, IWindowAbi>, menu: ComRef<'_, IMenuBuilderAbi>) -> Result<ComPtr<IPopupAbi>, NcStatus> {
        self.storage.factory.host.check_mutation()?;
        let parent = self.storage.parent(window)?;
        let declaration = self.storage.menu_declaration(menu)?;
        let mut context = self.storage.context()?;
        context.window_manager.window_visible(&parent).map_err(windows::status)?;
        let menu = declaration.menu.try_borrow_mut().map_err(|_| UI_E_BUSY)?.take().ok_or(UI_E_CLOSED)?;
        let popup = context
            .window_manager
            .create_menu_popup(&parent, menu)
            .unwrap_or_else(|_| unreachable!("authenticated parent cannot disappear before registration"));
        let identity = windows::LogicalPopup::from_value(windows::LogicalPopup {
            runtime: Rc::downgrade(&self.storage),
            popup: popup.clone(),
            closed: Cell::new(false),
        });
        self.storage.mounts.borrow_mut().push(Mount {
            surface: NativeSurface::Popup(popup),
            identity: identity.clone(),
        });
        self.storage.pending.set(true);
        identity.query()
    }
}
