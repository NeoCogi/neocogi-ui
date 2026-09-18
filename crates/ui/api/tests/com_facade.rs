//! Exercises the static facade against independent concrete COM implementations.
mod tests {
    use neocogi_ui::com::*;
    use neocogi_ui::com::nodes::ButtonInterfacesFor;
    use neocogi_com::{ComRef, NcCopyFromBufferOffset, NcStatus};
    use neocogi_com::{ComPtr, IUnknown, NcString, NC_E_INVALID_ARG, com_object};

    #[com_object(Button)]
    /// Concrete stand-in for a plugin button, using the same COM macro and ownership path.
    struct TestButton {
        /// Plugin-local UTF-8 storage; its Rust layout never crosses the boundary.
        label: String,
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

        /// This editing-only fixture has no event producer.
        fn subscribe_activated(&self, _sink: ComRef<'_, IActivationSinkAbi>) -> Result<ComPtr<IEventSubscriptionAbi>, NcStatus> {
            Err(neocogi_com::NC_E_NOINTERFACE)
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
        /// Its test-owned state needs no external teardown.
        fn close(&mut self) -> Result<(), NcStatus> {
            Ok(())
        }

        /// Fixture policy is independent of any native runtime or theme implementation.
        fn properties(&self) -> Result<WidgetProperties, NcStatus> {
            Ok(WidgetProperties {
                options: WidgetOptions::NONE,
                keyboard: 0,
                frame: WidgetFrame::Generic,
            })
        }
        /// This fixed fixture permits only its supported inert option policy.
        fn set_options(&mut self, options: WidgetOptions) -> Result<(), NcStatus> {
            if options == WidgetOptions::NONE { Ok(()) } else { Err(NC_E_INVALID_ARG) }
        }
        /// Input validation remains typed even in a facade-only fake component.
        fn update(&mut self, _state: WidgetUpdate, event: WidgetEventAbi, text: NcCopyFromBufferOffset<'_, u8>) -> Result<GeometryChange, NcStatus> {
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
        let unknown: ComPtr<IUnknown> = TestButton::from_value(TestButton { label: String::new(), changed: false });
        let mut button = ButtonWidget::from_interface(unknown.query::<ButtonAbi>()?);
        button.set_label("Save")?;
        assert_eq!(button.label()?, "Save");
        let mut node = button.node_interface()?;
        assert_eq!(node.take_change()?, GeometryChange::Changed);
        assert_eq!(node.take_change()?, GeometryChange::NoChange);
        button.set_label("Save")?;
        assert_eq!(node.take_change()?, GeometryChange::NoChange);
        Ok(())
    }

    /// Wrong projections fail through QueryInterface and invalid inputs are transactional.
    #[test]
    fn concrete_proxy_rejects_wrong_capabilities_and_invalid_text() {
        let unknown = TestButton::from_value(TestButton {
            label: String::from("before"),
            changed: false,
        });
        assert_eq!(unknown.query::<IChildrenAbi>().unwrap_err(), neocogi_com::NC_E_NOINTERFACE);
        let mut button: ComPtr<ButtonAbi> = unknown.query().unwrap();
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
