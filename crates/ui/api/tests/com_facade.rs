//! Exercises the static facade against independent concrete COM implementations.
// This executable owns the test runtime; no production client dependency is added.
extern crate neocogi_com_host as _;

mod tests {
    use neocogi_ui::com::*;
    use neocogi_com::{ComRef, NcCopyFromBufferOffset, NcStatus};
    use neocogi_com::{ComPtr, ComBox, IUnknown, NcString, NC_E_INVALID_ARG};

    #[neocogi_com::com_object(neocogi_ui::nodes::Button, ownership = exclusive)]
    /// Concrete stand-in for a plugin button, using the same COM macro and ownership path.
    struct TestButton {
        /// Plugin-local UTF-8 storage; its Rust layout never crosses the boundary.
        label: String,
        /// Current requested flags owned by this independent COM fixture.
        requested_flags: WidgetOptions,
        /// Sticky invalidation consumed by runtime traversal.
        changed: bool,
    }

    impl Button for TestButton {
        /// Validates the complete replacement before mutating retained text.
        fn set_label(&mut self, label: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
            if label.offset() != 0 {
                return Err(NC_E_INVALID_ARG);
            }
            let text = std::str::from_utf8(label.as_slice()).map_err(|_| NC_E_INVALID_ARG)?;
            if self.label != text {
                self.label = text.to_owned();
                self.changed = true;
            }
            Ok(())
        }

        /// Copies a label snapshot into a standard COM string object.
        fn label(&self) -> Result<ComPtr<neocogi_com::INcStringAbi>, NcStatus> {
            Ok(NcString::from_utf8_str(&self.label))
        }

        /// Font editing is outside this facade fixture's observable state.
        fn set_font_role(&mut self, _role: FontRole) -> Result<(), NcStatus> {
            Ok(())
        }

        /// Named font editing is outside this facade fixture's observable state.
        fn set_named_font(&mut self, _name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
            Ok(())
        }

        /// Icon editing is outside this facade fixture's observable state.
        fn set_icon_role(&mut self, _icon: ContentIcon) -> Result<(), NcStatus> {
            Ok(())
        }

        /// Named icon editing is outside this facade fixture's observable state.
        fn set_named_icon(&mut self, _name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
            Ok(())
        }

        /// Returns one deterministic paint policy for proxy coverage.
        fn fill_states(&self) -> Result<FillStates, NcStatus> {
            Ok(FillStates::ALL)
        }

        /// Rejects unknown fill flags without retaining a second test field.
        fn set_fill_states(&mut self, states: FillStates) -> Result<(), NcStatus> {
            if states.is_valid() { Ok(()) } else { Err(NC_E_INVALID_ARG) }
        }

        /// Image content is intentionally absent from this minimal fixture.
        fn set_image(&mut self, _image: ComRef<'_, ITextureAbi>, _fit_width: bool) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_NOINTERFACE)
        }

        /// Clearing already-absent image content is idempotent.
        fn clear_image(&mut self) -> Result<(), NcStatus> {
            Ok(())
        }
    }

    impl ILeafLayout for TestButton {
        /// Uses the supplied shared metrics capability without retaining it.
        fn measure(&mut self, _bounds: LayoutLimits, metrics: ComRef<'_, ITextMetricsAbi>) -> Result<Size, NcStatus> {
            metrics.button_size(NcCopyFromBufferOffset::new(0, self.label.as_bytes()))
        }
    }

    impl IWidget for TestButton {
        /// This standalone facade fixture deliberately has no mountable runtime owner.
        fn host(&self) -> Result<ComPtr<IWidgetHostAbi>, NcStatus> {
            Err(neocogi_com::NC_E_NOINTERFACE)
        }
        /// Attachment is outside this editing-only fixture's supported behavior.
        fn mount(&mut self) -> Result<(), NcStatus> {
            Err(neocogi_com::NC_E_NOINTERFACE)
        }
        /// Fixture policy is independent of any native runtime or theme implementation.
        fn properties(&self) -> Result<WidgetProperties, NcStatus> {
            Ok(WidgetProperties {
                options: self.requested_flags,
                keyboard: 0,
                frame: WidgetFrame::Generic,
            })
        }
        /// Reads the requested configuration without including transient runtime policy.
        fn requested_flags(&self) -> Result<WidgetOptions, NcStatus> {
            Ok(self.requested_flags)
        }
        /// Replaces requested bits and preserves earlier label/geometry invalidation.
        fn replace_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
            flags.validate()?;
            self.changed |= self.requested_flags != flags;
            self.requested_flags = flags;
            Ok(())
        }
        /// Adds selected bits to the fixture's authoritative requested state.
        fn set_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
            flags.validate()?;
            self.replace_flags(self.requested_flags | flags)
        }
        /// Validates the clear mask before removing bits from requested state.
        fn clear_flags(&mut self, flags: WidgetOptions) -> Result<(), NcStatus> {
            flags.validate()?;
            self.replace_flags(WidgetOptions::from_raw(self.requested_flags.raw() & !flags.raw()))
        }
        /// Input validation remains typed even in a facade-only fake component.
        fn update(&mut self, _state: WidgetUpdate, event: WidgetEvent, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
            decode_widget_event(event, text)?;
            Ok(GeometryChange::NoChange)
        }

        /// Clears one accumulated programmatic geometry change.
        fn take_change(&mut self) -> Result<GeometryChange, NcStatus> {
            let changed = self.changed;
            self.changed = false;
            Ok(if changed { GeometryChange::Changed } else { GeometryChange::NoChange })
        }

        /// Delegates one illustrative fill to the scope-checked recorder.
        fn paint(&mut self, scope: PaintScope, state: WidgetPaint, mut painter: ComRef<'_, IPainterAbi>) -> Result<(), NcStatus> {
            // Retaining this interface is safe only because its implementation owns its storage;
            // the production recorder must reject this scope after the paint invocation ends.
            painter.fill(scope, state.rect, 0xff808080)
        }
    }

    /// The generic host proxy edits a concrete COM object and shares its invalidation state.
    #[test]
    fn generic_proxy_calls_com_contract() -> Result<(), NcStatus> {
        let unknown: ComBox<IUnknown> = TestButton::into_box(TestButton {
            label: String::new(),
            requested_flags: WidgetOptions::NONE,
            changed: false,
        });
        let mut button = ButtonWidget::from_interface(unknown.downgrade().query::<ButtonAbi>()?);
        button.set_label("Save")?;
        assert_eq!(button.label()?, "Save");
        let mut node = button.node_interface()?;
        assert_eq!(node.take_change()?, GeometryChange::Changed);
        assert_eq!(node.take_change()?, GeometryChange::NoChange);
        button.set_label("Save")?;
        assert_eq!(node.take_change()?, GeometryChange::NoChange);
        // The facade forwards all three operations to the independent object's current state.
        button.replace_flags(WidgetOptions::ALIGN_RIGHT)?;
        button.set_flags(WidgetOptions::NO_INTERACT)?;
        assert_eq!(node.properties()?.options, WidgetOptions::ALIGN_RIGHT | WidgetOptions::NO_INTERACT);
        button.clear_flags(WidgetOptions::NO_INTERACT)?;
        assert_eq!(node.properties()?.options, WidgetOptions::ALIGN_RIGHT);
        assert_eq!(node.take_change()?, GeometryChange::Changed);
        button.clear_flags(WidgetOptions::NO_INTERACT)?;
        assert_eq!(node.take_change()?, GeometryChange::NoChange);
        Ok(())
    }

    /// Construction uses one owner-sized facade; an explicitly requested field
    /// observes the same edits and expires when the transferred owner is dropped.
    #[test]
    fn owned_facade_has_one_handle_and_explicit_observers() -> Result<(), NcStatus> {
        use core::mem::size_of;
        assert_eq!(size_of::<OwnedWidget<ButtonAbi>>(), size_of::<ComBox<ButtonAbi>>());
        assert_eq!(size_of::<OwnedWidget<ButtonAbi>>(), 2 * size_of::<usize>());
        assert_eq!(size_of::<ButtonWidget>(), 2 * size_of::<usize>());
        let object = TestButton::into_box(TestButton {
            label: String::from("initial"),
            requested_flags: WidgetOptions::NONE,
            changed: false,
        })
        .query::<ButtonAbi>()
        .unwrap();
        let mut owner = OwnedWidget::from_interface(object);
        owner.set_label("before insertion")?;
        let mut field = owner.handle();
        assert_eq!(field.label()?, "before insertion");
        owner.set_flags(WidgetOptions::NO_INTERACT)?;
        assert_eq!(field.node_interface()?.properties()?.options, WidgetOptions::NO_INTERACT);
        field.set_label("through field")?;
        assert_eq!(owner.label()?, "through field");
        // Attachment moves only the original owner; the field has no retaining
        // conversion and cannot keep the payload alive when that owner expires.
        let mounted = owner.with_mount(Ok).unwrap();
        assert!(mounted.downgrade().same_object(field.as_interface()));
        assert_eq!(field.label()?, "through field");
        drop(mounted);
        assert_eq!(field.label(), Err(neocogi_com::NC_E_DROPPED));
        assert_eq!(field.set_label("too late"), Err(neocogi_com::NC_E_DROPPED));
        Ok(())
    }

    /// Wrong projections fail through QueryInterface and invalid inputs are transactional.
    #[test]
    fn concrete_proxy_rejects_wrong_capabilities_and_invalid_text() {
        let unknown = TestButton::into_box(TestButton {
            label: String::from("before"),
            requested_flags: WidgetOptions::NONE,
            changed: false,
        });
        assert_eq!(unknown.downgrade().query::<IChildrenAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
        let mut button = unknown.downgrade().query::<ButtonAbi>().unwrap();
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(1, b"after")), Err(NC_E_INVALID_ARG));
        assert_eq!(button.set_label(NcCopyFromBufferOffset::new(0, &[0xff])), Err(NC_E_INVALID_ARG));
        let proxy = ButtonWidget::from_interface(button);
        assert_eq!(proxy.label().unwrap(), "before");
        let mut node = proxy.node_interface().unwrap();
        assert_eq!(node.take_change().unwrap(), GeometryChange::NoChange);
        let mut clone = proxy.clone();
        clone.set_label("shared").unwrap();
        assert_eq!(proxy.label().unwrap(), "shared");
    }
}
