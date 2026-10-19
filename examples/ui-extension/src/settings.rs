//! Provider-owned NCB settings and editor. No application layout reflection is involved.
use neocogi_com::{ComBox, ComRef, ComSerialize, ComDeserialize, NcOutputWriter, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::*;
use neocogi_ui_document_contracts::{IDesignSettingsEditor, IDesignSettingsEditorAbi};

/// Private persistent schema, interpreted only by this plugin.
#[derive(Clone, ComSerialize, ComDeserialize)]
pub(super) struct Settings {
    /// Positional NCB schemas require an explicit revision when their contract changes.
    revision: u32,
    /// Positive increment applied to each user activation.
    step: u32,
    /// Complete nested policy rather than independent, potentially inconsistent scalar properties.
    mode: Mode,
}
/// Tagged configuration whose bounded case owns its two related limits.
#[derive(Clone, ComSerialize, ComDeserialize)]
enum Mode {
    /// Count from zero using saturating arithmetic.
    Unlimited,
    /// Start at minimum and clamp user increments at maximum.
    Bounded {
        /// First permitted count, also the initial value.
        minimum: u32,
        /// Last permitted count, inclusive.
        maximum: u32,
    },
}
impl Default for Settings {
    /// Typed and catalog construction share this exact initial policy.
    fn default() -> Self {
        Self {
            revision: 1,
            step: 1,
            mode: Mode::Unlimited,
        }
    }
}
impl Settings {
    /// Version and semantic checks precede construction or draft publication.
    fn validate(&self) -> Result<(), NcStatus> {
        match (&self.mode, self.revision, self.step) {
            (Mode::Unlimited, 1, 1..) => Ok(()),
            (Mode::Bounded { minimum, maximum }, 1, 1..) if minimum <= maximum => Ok(()),
            _ => Err(NC_E_INVALID_ARG),
        }
    }
    /// Both editor and constructor decode the same complete schema and budgets.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, NcStatus> {
        let settings: Self = neocogi_ui_design::decode_settings(bytes).map_err(|_| NC_E_INVALID_ARG)?;
        settings.validate()?;
        Ok(settings)
    }
    /// Encodes only an accepted typed value under the shared NCB transport ceiling.
    pub(super) fn encode(&self) -> Result<Vec<u8>, NcStatus> {
        self.validate()?;
        neocogi_ui_design::encode_settings(self).map_err(|_| NC_E_INVALID_ARG)
    }
    /// The bounded mode starts within its own legal interval.
    pub(super) fn initial(&self) -> u32 {
        match self.mode {
            Mode::Unlimited => 0,
            Mode::Bounded { minimum, .. } => minimum,
        }
    }
    /// User increments and application setters use the same accepted interval.
    pub(super) fn accepts(&self, count: u32) -> bool {
        match self.mode {
            Mode::Unlimited => true,
            Mode::Bounded { minimum, maximum } => (minimum..=maximum).contains(&count),
        }
    }
    /// Saturation is deterministic at integer and configured boundaries.
    pub(super) fn next(&self, count: u32) -> u32 {
        let next = count.saturating_add(self.step);
        match self.mode {
            Mode::Unlimited => next,
            Mode::Bounded { maximum, .. } => next.min(maximum),
        }
    }
}

#[neocogi_com::com_object(interfaces(neocogi_ui_document_contracts::IDesignSettingsEditor), ownership = exclusive)]
/// Draft controller: the inspector owns the view after transfer; these controls are weak.
struct Editor {
    /// Unmounted controls held only until take_view transfers their sole ownership.
    view: Option<ComBox<IWidgetAbi>>,
    /// Draft increment text; parsing is deferred until the explicit Apply operation.
    step: TextboxWidget,
    /// Selects the nested enum case without exposing a reflective property language.
    bounded: CheckboxWidget,
    /// Lower bound used only for the bounded case.
    minimum: TextboxWidget,
    /// Upper bound used only for the bounded case.
    maximum: TextboxWidget,
}
impl IDesignSettingsEditor for Editor {
    /// A view has exactly one owner; a second transfer cannot succeed.
    fn take_view(&mut self) -> Result<ComBox<IWidgetAbi>, NcStatus> {
        self.view.take().ok_or(neocogi_com::NC_E_NOT_FOUND)
    }
    /// Snapshot reads never modify the accepted product. Invalid drafts remain editable.
    fn settings(&self, mut output: NcOutputWriter<'_, u8>) -> Result<u32, NcStatus> {
        if output.offset() != 0 {
            return Err(NC_E_INVALID_ARG);
        }
        let settings = Settings {
            revision: 1,
            step: self.step.text()?.parse().map_err(|_| NC_E_INVALID_ARG)?,
            mode: match self.bounded.checked()? {
                false => Mode::Unlimited,
                true => Mode::Bounded {
                    minimum: self.minimum.text()?.parse().map_err(|_| NC_E_INVALID_ARG)?,
                    maximum: self.maximum.text()?.parse().map_err(|_| NC_E_INVALID_ARG)?,
                },
            },
        };
        let bytes = settings.encode()?;
        if output.capacity() >= bytes.len() {
            output.extend_from_slice(&bytes)?;
        }
        Ok(bytes.len() as u32)
    }
}

/// Adds an ordinary control to the unique view owner, dropping rejected fresh controls after return.
fn append<I: neocogi_com::ComInterface>(root: &mut OwnedWidget<LinearAbi>, child: OwnedWidget<I>) -> Result<(), NcStatus> {
    root.insert(root.children()?.child_count()?, child, LinearEdge::new(LayoutTrack::Content))
        .map_err(|(status, _child)| status)
}
/// Label and input remain ordinary host-provided controls; no backend implementation is linked.
fn text(widgets: &WidgetFactory, root: &mut OwnedWidget<LinearAbi>, label: &str, value: u32) -> Result<TextboxWidget, NcStatus> {
    append(root, widgets.text_block(label, TextWrapping::None)?)?;
    let control = widgets.textbox_unobserved(&value.to_string())?;
    let handle = control.handle();
    append(root, control)?;
    Ok(handle)
}
/// Fully constructs a draft before publishing its controller. No queue or RefCell is needed:
/// native controls own editing state and settings() reads them after input dispatch returns.
pub(super) fn create(widgets: ComRef<'_, UiFactoryAbi>, bytes: &[u8]) -> Result<ComBox<IDesignSettingsEditorAbi>, NcStatus> {
    let settings = Settings::decode(bytes)?;
    let widgets = WidgetFactory::from_interface(widgets.try_add_ref()?);
    let mut root = widgets.linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    let step = text(&widgets, &mut root, "Increment", settings.step)?;
    let (bounded, minimum, maximum) = match settings.mode {
        Mode::Unlimited => (false, 0, 100),
        Mode::Bounded { minimum, maximum } => (true, minimum, maximum),
    };
    let control = widgets.checkbox_unobserved("Bounded counter", bounded)?;
    let bounded = control.handle();
    append(&mut root, control)?;
    let minimum = text(&widgets, &mut root, "Minimum", minimum)?;
    let maximum = text(&widgets, &mut root, "Maximum", maximum)?;
    let view = root.into_interface().query::<IWidgetAbi>().map_err(|(status, _root)| status)?;
    Editor::into_box(Editor {
        view: Some(view),
        step,
        bounded,
        minimum,
        maximum,
    })
    .query()
    .map_err(|(status, _editor)| status)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Both construction policies are persisted as their real record/tagged payload schema.
    #[test]
    fn nested_settings_round_trip_and_enforce_revision_and_semantics() {
        for mode in [Mode::Unlimited, Mode::Bounded { minimum: 10, maximum: 20 }] {
            let settings = Settings { revision: 1, step: 3, mode };
            let decoded = Settings::decode(&settings.encode().unwrap()).unwrap();
            assert_eq!(decoded.step, 3);
            assert_eq!(decoded.initial(), settings.initial());
            assert_eq!(decoded.next(19), settings.next(19));
            assert_eq!(decoded.accepts(21), settings.accepts(21));
        }
        for settings in [
            Settings { revision: 2, ..Settings::default() },
            Settings { step: 0, ..Settings::default() },
            Settings {
                mode: Mode::Bounded { minimum: 20, maximum: 10 },
                ..Settings::default()
            },
        ] {
            // Bypass semantic encode only to construct a well-formed but invalid provider input.
            let bytes = neocogi_ui_design::encode_settings(&settings).unwrap();
            assert!(Settings::decode(&bytes).is_err());
            assert!(settings.encode().is_err());
        }
        let bytes = Settings::default().encode().unwrap();
        for end in 0..bytes.len() {
            assert!(Settings::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(Settings::decode(&trailing).is_err());
        assert!(Settings::decode(&vec![0; neocogi_ui_design::MAX_SETTINGS_BYTES + 1]).is_err());
    }
}
