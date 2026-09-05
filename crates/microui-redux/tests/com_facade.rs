//! Exercises the static facade against independent concrete COM implementations.
mod tests {
    use microui_redux::com::*;
    use nc_com::{ComRef, NcCopyFromBufferOffset, NcStatus};
    use nc_com::{ComPtr, IUnknown, NcString, NC_E_INVALID_ARG, com_class};
    use std::cell::{Cell, RefCell};

    #[com_class(interfaces(IWidget, IButton))]
    /// Concrete stand-in for a plugin button, using the same COM macro and ownership path.
    struct TestButton {
        /// Plugin-local UTF-8 storage; its Rust layout never crosses the boundary.
        label: RefCell<String>,
        /// Sticky invalidation consumed by runtime traversal.
        changed: Cell<bool>,
    }

    impl IButton for TestButton {
        /// Validates the complete replacement before mutating retained text.
        fn set_label(&self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
            if label.offset() != 0 {
                return Err(NC_E_INVALID_ARG);
            }
            let text = std::str::from_utf8(label.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
            let mut stored = self.label.try_borrow_mut().map_err(|_| NC_E_INVALID_ARG)?;
            if *stored != text {
                *stored = text.to_owned();
                self.changed.set(true);
            }
            Ok(())
        }

        /// Copies a label snapshot into a standard COM string object.
        fn label(&self) -> Result<ComPtr<nc_com::INcStringAbi>, NcStatus> {
            let stored = self.label.try_borrow().map_err(|_| NC_E_INVALID_ARG)?;
            Ok(NcString::from_utf8_str(&stored))
        }
    }

    impl IWidget for TestButton {
        /// This standalone facade fixture deliberately has no mountable runtime owner.
        fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
            Err(nc_com::NC_E_NOINTERFACE)
        }
        /// Attachment is outside this editing-only fixture's supported behavior.
        fn mount(&self) -> Result<(), NcStatus> {
            Err(nc_com::NC_E_NOINTERFACE)
        }
        /// Its test-owned state needs no external teardown.
        fn close(&self) -> Result<(), NcStatus> {
            Ok(())
        }

        /// Fixture policy is independent of any native runtime or theme implementation.
        fn properties(&self) -> Result<WidgetProperties, NcStatus> {
            Ok(WidgetProperties {
                options: WidgetOptions::NONE,
                keyboard: 0,
                frame: WidgetFrame::GENERIC,
            })
        }
        /// This fixed fixture permits only its supported inert option policy.
        fn set_options(&self, options: WidgetOptions) -> Result<(), NcStatus> {
            if options == WidgetOptions::NONE { Ok(()) } else { Err(NC_E_INVALID_ARG) }
        }
        /// Input validation remains typed even in a facade-only fake component.
        fn update(&self, _state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
            decode_widget_event(event, text)?;
            Ok(GeometryChange::NO_CHANGE)
        }

        /// Clears one accumulated programmatic geometry change.
        fn take_change(&self) -> Result<GeometryChange, NcStatus> {
            Ok(if self.changed.replace(false) {
                GeometryChange::CHANGED
            } else {
                GeometryChange::NO_CHANGE
            })
        }

        /// Delegates one illustrative fill to the scope-checked recorder.
        fn paint(&self, scope: PaintScope, rect: Rect, painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
            // Retaining this interface is safe only because its implementation owns its storage;
            // the production recorder must reject this scope after the paint invocation ends.
            painter.fill(scope, rect, 0xff808080)
        }
    }

    /// The generic host proxy edits a concrete COM object and shares its invalidation state.
    #[test]
    fn generic_proxy_calls_com_contract() -> Result<(), NcStatus> {
        let unknown: ComPtr<IUnknown> = TestButton::from_value(TestButton {
            label: RefCell::new(String::new()),
            changed: Cell::new(false),
        });
        let button = Button::from_interface(unknown.query::<IButtonAbi>()?);
        button.set_label("Save")?;
        assert_eq!(button.label()?, "Save");
        let node = button.node_interface()?;
        assert_eq!(node.take_change()?, GeometryChange::CHANGED);
        assert_eq!(node.take_change()?, GeometryChange::NO_CHANGE);
        button.set_label("Save")?;
        assert_eq!(node.take_change()?, GeometryChange::NO_CHANGE);
        Ok(())
    }

    /// Wrong projections fail through QueryInterface and invalid inputs are transactional.
    #[test]
    fn concrete_proxy_rejects_wrong_capabilities_and_invalid_text() {
        let unknown = TestButton::from_value(TestButton {
            label: RefCell::new(String::from("before")),
            changed: Cell::new(false),
        });
        assert_eq!(unknown.query::<IChildrenAbi>().unwrap_err(), nc_com::NC_E_NOINTERFACE);
        let button: ComPtr<IButtonAbi> = unknown.query().unwrap();
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(1, b"after")), Err(NC_E_INVALID_ARG));
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
        let proxy = Button::from_interface(button);
        assert_eq!(proxy.label().unwrap(), "before");
        assert_eq!(proxy.node_interface().unwrap().take_change().unwrap(), GeometryChange::NO_CHANGE);
        let clone = proxy.clone();
        clone.set_label("shared").unwrap();
        assert_eq!(proxy.label().unwrap(), "shared");
    }
}
