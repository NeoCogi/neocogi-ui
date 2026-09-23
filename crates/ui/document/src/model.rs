//! Explicit authored values; live widget handles and transient state never enter this schema.
use neocogi_com::{ComSerialize, ComDeserialize};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

/// Persistent schema revision; changes to field order or enum ordinals require a new revision.
pub const VERSION: u32 = 2;
/// Maximum authored nodes in one component.
pub const MAX_NODES: usize = 4096;
/// Maximum authored ownership depth.
pub const MAX_DEPTH: usize = 32;
/// Maximum encoded project/document size.
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// One reusable UI component, independent of native window and filesystem ownership.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Explicit schema revision, currently VERSION.
    pub version: u32,
    /// Human-readable component title.
    pub name: String,
    /// The single owned widget tree.
    pub root: Node,
    /// Public events mapped to internal concrete ports.
    pub outputs: Vec<Output>,
    /// Public commands mapped to typed internal setters.
    pub inputs: Vec<Input>,
    /// Executable private output-to-input connections, dispatched by the mounted component.
    pub connections: Vec<Connection>,
}

/// One authored node; key is a document-local name preserved when reparenting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Unique nonempty internal name, independent of visible text and sibling order.
    pub key: String,
    /// Complete concrete initialization.
    pub widget: WidgetSpec,
    /// Common presentation and logical resource references.
    pub style: Style,
    /// Owned children with their parent-owned layout settings.
    pub children: Vec<Child>,
}

/// A child and the relationship owned by its parent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Child {
    /// Layout settings interpreted by the owning container.
    pub edge: Edge,
    /// Child subtree, owned exactly once.
    pub node: Node,
}

/// Persistent presentation; no GPU handles, atlas indices, or preview focus state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    /// Explicit widget option bits, or the normal constructor defaults.
    pub options: Option<u16>,
    /// Named font in the supplied asset bundle, or the widget default.
    pub font: Option<String>,
    /// Named icon for a button/list item, or no override.
    pub icon: Option<String>,
    /// Logical image key resolved by the application resource provider.
    pub image: Option<String>,
    /// Scale image content to the assigned width.
    pub fit_image: bool,
}

/// Persistent main-axis or grid track policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum Track {
    /// Intrinsic preferred size.
    Content,
    /// An exact nonnegative extent.
    Fixed {
        /// Pixel extent.
        pixels: i32,
    },
    /// A positive finite share of remaining space.
    Flex {
        /// Relative weight.
        weight: f32,
    },
}

/// Persistent linear cross-axis policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum Cross {
    /// Use content preferred extent.
    Content,
    /// Use allocated extent.
    Stretch,
    /// An exact nonnegative extent.
    Fixed {
        /// Pixel extent.
        pixels: i32,
    },
}

/// Ordered flow without ambiguous independent direction bits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum Direction {
    /// Left to right.
    Right,
    /// Right to left.
    Left,
    /// Top to bottom.
    Down,
    /// Bottom to top.
    Up,
}

/// Text wrapping policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum Wrapping {
    /// Do not wrap.
    None,
    /// Wrap at spaces.
    Word,
    /// Wrap at spaces or Unicode scalar boundaries.
    Character,
}

/// Only the owning container interprets this layout relationship.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum Edge {
    /// Linear or disclosure-body sizing.
    Linear {
        /// Main-axis size.
        main: Track,
        /// Exact nonnegative cross size, or stretch.
        cross: Option<i32>,
    },
    /// Sequential grid placement with positive spans.
    Grid {
        /// Column span.
        columns: u32,
        /// Row span.
        rows: u32,
    },
    /// Split pane or scroll content; no independent sizing metadata.
    Pane,
    /// Exact child rectangle within a Form, in content pixels.
    Position {
        /// Distance from the left content edge.
        x: i32,
        /// Distance from the top content edge.
        y: i32,
        /// Positive outer control width.
        width: i32,
        /// Positive outer control height.
        height: i32,
    },
}

/// Closed built-in constructor schemas; each variant has an explicit factory mapping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum WidgetSpec {
    /// Action button.
    Button {
        /// Initial visible label or text.
        text: String,
    },
    /// Boolean control.
    Checkbox {
        /// Initial visible label or text.
        text: String,
        /// Initial checked value.
        checked: bool,
    },
    /// Single-line editable text.
    Textbox {
        /// Initial visible label or text.
        text: String,
    },
    /// Multiline display text.
    TextBlock {
        /// Initial visible label or text.
        text: String,
        /// Wrapping policy.
        wrapping: Wrapping,
    },
    /// Multiline editor with optional scrolling.
    TextArea {
        /// Initial visible label or text.
        text: String,
        /// Wrapping policy.
        wrapping: Wrapping,
        /// Enable scrolling.
        scrolling: bool,
    },
    /// Bounded numeric control.
    Slider {
        /// Initial value.
        value: f32,
        /// Lower bound.
        low: f32,
        /// Upper bound.
        high: f32,
        /// Nonnegative increment; zero is continuous.
        step: f32,
        /// Decimal precision from zero through six.
        precision: u8,
    },
    /// Unbounded numeric entry.
    Number {
        /// Initial finite value.
        value: f32,
        /// Nonnegative increment.
        step: f32,
        /// Decimal precision from zero through six.
        precision: u8,
    },
    /// An RGBA swatch with an optional label.
    ColorSwatch {
        /// Initial visible label or text.
        text: String,
        /// RGBA value in the normal renderer byte order.
        rgba: u32,
    },
    /// Activatable list row.
    ListItem {
        /// Initial visible label or text.
        text: String,
    },
    /// Button-like list entry.
    ListBox {
        /// Initial visible label or text.
        text: String,
    },
    /// Standalone normalized scroll model.
    Scrollbar {
        /// Vertical rather than horizontal axis.
        vertical: bool,
        /// Nonnegative viewport length.
        viewport: i32,
        /// Nonnegative content length.
        content: i32,
        /// Initial nonnegative offset.
        offset: i32,
    },
    /// Window-owned flat choice menu.
    Combo {
        /// Choice labels in order.
        labels: Vec<String>,
        /// Initial selection, or none.
        selected: Option<u32>,
        /// Text used with no selection.
        placeholder: String,
    },
    /// Directional child container.
    Linear {
        /// Main-axis direction.
        direction: Direction,
        /// Shared cross-axis sizing.
        cross: Cross,
    },
    /// Grid with sequential cells and per-child spans.
    Grid {
        /// Nonempty column policies.
        columns: Vec<Track>,
        /// Nonempty row policies.
        rows: Vec<Track>,
    },
    /// At most two resizable panes.
    Split {
        /// Stack panes vertically when true.
        vertical: bool,
        /// Requested first-pane extent.
        position: i32,
        /// Nonnegative first-pane minimum.
        minimum_first: i32,
        /// Nonnegative second-pane minimum.
        minimum_second: i32,
        /// Positive divider thickness.
        handle: i32,
    },
    /// One scrolling content subtree.
    Scroll {
        /// Enable scrolling.
        enabled: bool,
    },
    /// Expandable header and linear body.
    Disclosure {
        /// Initial visible label or text.
        text: String,
        /// Compact tree header rather than framed header.
        tree: bool,
        /// Initial expansion state.
        expanded: bool,
    },
    /// Fixed-coordinate form with a preferred content size.
    Form {
        /// Positive preferred content width.
        width: i32,
        /// Positive preferred content height.
        height: i32,
    },
}

/// One stable public output mapped to a private concrete event port.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    /// Public application-facing name.
    pub name: String,
    /// Internal node key.
    pub node: String,
    /// Concrete declared event, such as activated or changed.
    pub port: String,
}

/// One stable public input mapped to a typed setter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Public application-facing name.
    pub name: String,
    /// Internal node key.
    pub node: String,
    /// Concrete accepted message contract.
    pub kind: InputKind,
}

/// Supported typed application-to-view operations.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum InputKind {
    /// SetText changes text or label.
    Text,
    /// SetChecked changes checkbox state.
    Checked,
    /// SetValue changes numeric state.
    Value,
    /// SetEnabled preserves other option bits.
    Enabled,
    /// SetExpanded changes disclosure expansion.
    Expanded,
    /// SetChoices replaces a combo model.
    Choices,
    /// SetSelection selects an existing combo item or clears selection.
    Selection,
    /// SetOffset moves a scrollbar within its current range.
    Offset,
    /// SetPosition changes only the requested split divider position.
    Position,
}
impl InputKind {
    /// Complete command catalog shared by validation and authoring tools.
    pub const ALL: [Self; 9] = [
        Self::Text,
        Self::Checked,
        Self::Value,
        Self::Enabled,
        Self::Expanded,
        Self::Choices,
        Self::Selection,
        Self::Offset,
        Self::Position,
    ];

    /// Human-readable operation label; field identity always uses the enum itself.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text => "Set text",
            Self::Checked => "Set checked",
            Self::Value => "Set value",
            Self::Enabled => "Set enabled",
            Self::Expanded => "Set expanded",
            Self::Choices => "Set choices",
            Self::Selection => "Set selection",
            Self::Offset => "Set offset",
            Self::Position => "Set position",
        }
    }
}

impl Default for Style {
    /// Retains normal constructor presentation without resource overrides.
    fn default() -> Self {
        Self {
            options: None,
            font: None,
            icon: None,
            image: None,
            fit_image: false,
        }
    }
}
impl Node {
    /// Creates a leaf or empty container using normal widget defaults.
    pub fn new(key: impl Into<String>, widget: WidgetSpec) -> Self {
        Self {
            key: key.into(),
            widget,
            style: Style::default(),
            children: Vec::new(),
        }
    }
    /// Finds a named node without exposing runtime handles or depending on sibling positions.
    pub fn find(&self, key: &str) -> Option<&Node> {
        if self.key == key {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.node.find(key))
    }
    /// Finds an authored node for one exclusive editor operation.
    pub fn find_mut(&mut self, key: &str) -> Option<&mut Node> {
        if self.key == key {
            return Some(self);
        }
        self.children.iter_mut().find_map(|child| child.node.find_mut(key))
    }
}
impl WidgetSpec {
    /// Explicit UI label survives release builds that strip generated Debug implementations.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Button { .. } => "Button",
            Self::Checkbox { .. } => "Checkbox",
            Self::Textbox { .. } => "Textbox",
            Self::TextBlock { .. } => "TextBlock",
            Self::TextArea { .. } => "TextArea",
            Self::Slider { .. } => "Slider",
            Self::Number { .. } => "Number",
            Self::ColorSwatch { .. } => "ColorSwatch",
            Self::ListItem { .. } => "ListItem",
            Self::ListBox { .. } => "ListBox",
            Self::Scrollbar { .. } => "Scrollbar",
            Self::Combo { .. } => "Combo",
            Self::Linear { .. } => "Linear",
            Self::Grid { .. } => "Grid",
            Self::Split { .. } => "Split",
            Self::Scroll { .. } => "Scroll",
            Self::Disclosure { .. } => "Disclosure",
            Self::Form { .. } => "Form",
        }
    }
    /// Whether this authored kind owns user-editable children.
    pub fn container(&self) -> bool {
        matches!(
            self,
            Self::Linear { .. } | Self::Grid { .. } | Self::Split { .. } | Self::Scroll { .. } | Self::Disclosure { .. } | Self::Form { .. }
        )
    }
}
impl Document {
    /// A small useful initial form, also the embedded runtime example.
    pub fn new(name: impl Into<String>) -> Self {
        let mut root = Node::new(
            "root",
            WidgetSpec::Linear {
                direction: Direction::Down,
                cross: Cross::Stretch,
            },
        );
        root.children.push(Child {
            edge: Edge::Linear { main: Track::Content, cross: None },
            node: Node::new(
                "title",
                WidgetSpec::TextBlock {
                    text: "Hello UI".into(),
                    wrapping: Wrapping::Word,
                },
            ),
        });
        root.children.push(Child {
            edge: Edge::Linear { main: Track::Content, cross: None },
            node: Node::new("save", WidgetSpec::Button { text: "Save".into() }),
        });
        Self {
            version: VERSION,
            name: name.into(),
            root,
            outputs: vec![Output {
                name: "save_requested".into(),
                node: "save".into(),
                port: "activated".into(),
            }],
            connections: vec![],
            inputs: vec![Input {
                name: "title".into(),
                node: "title".into(),
                kind: InputKind::Text,
            }],
        }
    }
    /// Rejects invalid shape, names, arithmetic, references and endpoint contracts before construction.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!("Unsupported UI document revision {}", self.version));
        }
        let mut nodes = BTreeMap::new();
        validate_node(&self.root, 0, &mut nodes)?;
        let mut names = std::collections::BTreeSet::new();
        let mut ports = std::collections::BTreeSet::new();
        for output in &self.outputs {
            if output.name.is_empty() || !names.insert(&output.name) {
                return Err(format!("Duplicate or empty output: {}", output.name));
            }
            let node = nodes
                .get(output.node.as_str())
                .ok_or_else(|| format!("Output {} refers to missing node {}", output.name, output.node))?;
            if !ports.insert((&output.node, &output.port)) {
                return Err(format!("Port {}.{} is exported twice", output.node, output.port));
            }
            if !node.widget.outputs().contains(&output.port.as_str()) {
                return Err(format!("{} has no {} output", output.node, output.port));
            }
        }
        for connection in &self.connections {
            if !ports.insert((&connection.source, &connection.port)) {
                return Err(format!("Port {}.{} already has a destination", connection.source, connection.port));
            }
            let source = nodes.get(connection.source.as_str()).ok_or("Connection source is missing")?;
            let target = nodes.get(connection.target.as_str()).ok_or("Connection target is missing")?;
            let payload = source.widget.output_type(&connection.port).ok_or("Connection source has no such output")?;
            connection.action.validate(payload, &target.widget)?;
        }
        names.clear();
        let mut inputs = std::collections::BTreeSet::new();
        for input in &self.inputs {
            if input.name.is_empty() || !names.insert(&input.name) {
                return Err(format!("Duplicate or empty input: {}", input.name));
            }
            let node = nodes
                .get(input.node.as_str())
                .ok_or_else(|| format!("Input {} refers to missing node {}", input.name, input.node))?;
            // The inspector and runtime expose exactly one public name per concrete command.
            // Reject aliases at the source boundary instead of silently collapsing them on edit.
            if !inputs.insert((&input.node, &input.kind)) {
                return Err(format!("Input {} exports a command on {} more than once", input.name, input.node));
            }
            if !node.widget.accepts(&input.kind) {
                return Err(format!("{} cannot accept input {}", input.node, input.name));
            }
        }
        Ok(())
    }
    /// Reads a bounded editable JSON document using this same typed schema.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("UI document exceeds the byte limit".into());
        }
        let result: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        result.validate()?;
        Ok(result)
    }
    /// Emits readable editable source; validation prevents saving a broken authored tree.
    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("UI document exceeds the byte limit".into());
        }
        Ok(bytes)
    }
}

impl Track {
    /// Converts the persistent policy to the existing checked layout contract.
    pub fn layout(&self) -> neocogi_ui::LayoutTrack {
        match *self {
            Self::Content => neocogi_ui::LayoutTrack::Content,
            Self::Fixed { pixels } => neocogi_ui::LayoutTrack::Fixed { extent: pixels },
            Self::Flex { weight } => neocogi_ui::LayoutTrack::Flex { weight },
        }
    }
    /// Counts and arithmetic are checked before any widget is created.
    fn valid(&self) -> bool {
        match *self {
            Self::Content => true,
            Self::Fixed { pixels } => pixels >= 0,
            Self::Flex { weight } => weight.is_finite() && weight > 0.0,
        }
    }
}
impl WidgetSpec {
    /// Declared concrete outputs; this also populates the designer's output picker.
    pub fn outputs(&self) -> &'static [&'static str] {
        match self {
            Self::Button { .. } | Self::ListItem { .. } | Self::ListBox { .. } => &["activated"],
            Self::Checkbox { .. } | Self::Slider { .. } | Self::Number { .. } | Self::Scrollbar { .. } | Self::Split { .. } | Self::Combo { .. } => {
                &["changed"]
            }
            Self::Textbox { .. } | Self::TextArea { .. } => &["changed", "submitted"],
            _ => &[],
        }
    }
    /// Checks the concrete setter's payload contract without allocating a receiver.
    pub fn accepts(&self, input: &InputKind) -> bool {
        match input {
            InputKind::Enabled => true,
            InputKind::Text => matches!(
                self,
                Self::Button { .. }
                    | Self::Checkbox { .. }
                    | Self::Textbox { .. }
                    | Self::TextArea { .. }
                    | Self::TextBlock { .. }
                    | Self::ColorSwatch { .. }
                    | Self::ListItem { .. }
                    | Self::ListBox { .. }
                    | Self::Disclosure { .. }
            ),
            InputKind::Checked => matches!(self, Self::Checkbox { .. }),
            InputKind::Value => matches!(self, Self::Slider { .. } | Self::Number { .. }),
            InputKind::Expanded => matches!(self, Self::Disclosure { .. }),
            InputKind::Choices | InputKind::Selection => matches!(self, Self::Combo { .. }),
            InputKind::Offset => matches!(self, Self::Scrollbar { .. }),
            InputKind::Position => matches!(self, Self::Split { .. }),
        }
    }
}

/// Validates the complete tree with bounded recursion and a single name index.
fn validate_node<'a>(node: &'a Node, depth: usize, nodes: &mut BTreeMap<&'a str, &'a Node>) -> Result<(), String> {
    use WidgetSpec::*;
    if depth >= MAX_DEPTH || nodes.len() >= MAX_NODES {
        return Err("UI ownership tree exceeds its limits".into());
    }
    if node.key.is_empty() || nodes.insert(&node.key, node).is_some() {
        return Err(format!("Duplicate or empty node key: {}", node.key));
    }
    let valid = match &node.widget {
        Slider { value, low, high, step, precision } => {
            value.is_finite()
                && low.is_finite()
                && high.is_finite()
                && high >= low
                && (high - low).is_finite()
                && step.is_finite()
                && *step >= 0.0
                && *precision <= 6
        }
        Number { value, step, precision } => value.is_finite() && step.is_finite() && *step >= 0.0 && *precision <= 6,
        Scrollbar { viewport, content, offset, .. } => *viewport >= 0 && *content >= 0 && *offset >= 0,
        Combo { labels, selected, .. } => labels.len() <= 65536 && selected.is_none_or(|index| (index as usize) < labels.len()),
        Grid { columns, rows } => !columns.is_empty() && !rows.is_empty() && columns.iter().chain(rows).all(Track::valid),
        Split {
            position,
            minimum_first,
            minimum_second,
            handle,
            ..
        } => *position >= 0 && *minimum_first >= 0 && *minimum_second >= 0 && *handle > 0 && node.children.len() <= 2,
        Form { width, height } => *width > 0 && *height > 0,
        Scroll { .. } => node.children.len() == 1,
        Linear { cross: Cross::Fixed { pixels }, .. } => *pixels >= 0,
        _ => true,
    };
    if !valid {
        return Err(format!("Invalid {} settings on {}", node.widget.name(), node.key));
    }
    if !node.widget.container() && !node.children.is_empty() {
        return Err(format!("{} cannot own children", node.key));
    }
    if let Some(options) = node.style.options {
        neocogi_ui::WidgetOptions::from_raw(options).validate().map_err(|error| error.to_string())?;
    }
    if node.style.font.is_some()
        && !matches!(
            node.widget,
            Button { .. }
                | Checkbox { .. }
                | Textbox { .. }
                | TextArea { .. }
                | TextBlock { .. }
                | Slider { .. }
                | Number { .. }
                | ColorSwatch { .. }
                | ListItem { .. }
                | ListBox { .. }
                | Combo { .. }
                | Disclosure { .. }
        )
    {
        return Err(format!("{} has no font property", node.key));
    }
    if node.style.icon.is_some() && !matches!(node.widget, Button { .. } | ListItem { .. }) {
        return Err(format!("{} has no icon property", node.key));
    }
    if node.style.image.is_some() && !matches!(node.widget, Button { .. } | ListBox { .. }) {
        return Err(format!("{} has no image property", node.key));
    }
    for child in &node.children {
        let valid = match (&node.widget, &child.edge) {
            (Linear { .. } | Disclosure { .. }, Edge::Linear { main, cross }) => main.valid() && cross.is_none_or(|pixels| pixels >= 0),
            (Grid { .. }, Edge::Grid { columns, rows }) => *columns > 0 && *rows > 0,
            (Split { .. } | Scroll { .. }, Edge::Pane) => true,
            (Form { .. }, Edge::Position { x, y, width, height }) => {
                *x >= 0 && *y >= 0 && *width > 0 && *height > 0 && x.checked_add(*width).is_some() && y.checked_add(*height).is_some()
            }
            _ => false,
        };
        if !valid {
            return Err(format!("Invalid child layout on {} in {}", child.node.key, node.key));
        }
        validate_node(&child.node, depth + 1, nodes)?;
    }
    Ok(())
}

impl WidgetSpec {
    /// Authored constructor option defaults for this document revision.
    /// Dynamic routing bits (scroll eligibility or an empty split) are deliberately
    /// excluded: reading effective runtime properties back as authoring would
    /// freeze transient behavior into a later setter or saved document.
    pub fn default_options(&self) -> neocogi_ui::WidgetOptions {
        use neocogi_ui::WidgetOptions as Options;
        match self {
            Self::Button { .. }
            | Self::Textbox { .. }
            | Self::TextArea { .. }
            | Self::Slider { .. }
            | Self::Number { .. }
            | Self::Combo { .. }
            | Self::Scroll { .. } => Options::FRAME,
            Self::ColorSwatch { .. } => Options::FRAME | Options::ALIGN_CENTER | Options::NO_INTERACT,
            Self::Form { .. } | Self::TextBlock { .. } | Self::Linear { .. } | Self::Grid { .. } | Self::Disclosure { .. } => Options::NO_INTERACT,
            Self::Checkbox { .. } | Self::ListItem { .. } | Self::ListBox { .. } | Self::Scrollbar { .. } | Self::Split { .. } => Options::NONE,
        }
    }
}

/// Source acceptance must agree with the single-endpoint inspector representation.
#[cfg(test)]
mod binding_tests {
    use super::*;

    /// Different public names cannot conceal duplicate command endpoints during editing.
    #[test]
    fn duplicate_input_endpoints_are_rejected_before_editing() {
        let mut document = Document::new("bindings");
        document.inputs.push(Input {
            name: "title_alias".into(),
            node: "title".into(),
            kind: InputKind::Text,
        });
        assert!(document.validate().unwrap_err().contains("more than once"));
        // The same command on another node is an independent endpoint, not an alias.
        document.inputs[1].node = "save".into();
        document.validate().unwrap();
    }
}

/// A private executable edge; each source port has one public or private destination.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// Stable source node key.
    pub source: String,
    /// Concrete output port on the source widget.
    pub port: String,
    /// Stable destination node key; no public input alias is required.
    pub target: String,
    /// Typed transformation applied at the component's safe event boundary.
    pub action: Action,
}

/// Concrete output contracts shared by validation, runtime binding, and authoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    /// Payload-free activation.
    Activation,
    /// Checkbox boolean snapshot.
    Checked,
    /// Finite numeric snapshot.
    Number,
    /// Edited text plus cursor snapshot.
    TextChanged,
    /// Submitted text snapshot.
    TextSubmitted,
    /// Pixel scroll offset.
    Offset,
    /// Pixel divider position.
    Position,
    /// Optional combo selection identity.
    Selection,
}
impl WidgetSpec {
    /// Resolves a real port to its concrete message type; unknown ports remain invalid.
    pub fn output_type(&self, port: &str) -> Option<OutputType> {
        match (self, port) {
            (Self::Button { .. } | Self::ListItem { .. } | Self::ListBox { .. }, "activated") => Some(OutputType::Activation),
            (Self::Checkbox { .. }, "changed") => Some(OutputType::Checked),
            (Self::Slider { .. } | Self::Number { .. }, "changed") => Some(OutputType::Number),
            (Self::Textbox { .. } | Self::TextArea { .. }, "changed") => Some(OutputType::TextChanged),
            (Self::Textbox { .. } | Self::TextArea { .. }, "submitted") => Some(OutputType::TextSubmitted),
            (Self::Scrollbar { .. }, "changed") => Some(OutputType::Offset),
            (Self::Split { .. }, "changed") => Some(OutputType::Position),
            (Self::Combo { .. }, "changed") => Some(OutputType::Selection),
            _ => None,
        }
    }
}

/// Explicit payload forwarding or a concrete constant command; no executable code is serialized.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub enum Action {
    /// Forward a compatible captured output field to this concrete command.
    Forward(InputKind),
    /// Replace destination text on every source event.
    Text(String),
    /// Set destination checked state on every source event.
    Checked(bool),
    /// Set a finite numeric value on every source event.
    Value(f32),
    /// Enable or disable the destination on every source event.
    Enabled(bool),
    /// Expand or collapse the destination on every source event.
    Expanded(bool),
    /// Select a combo ordinal, or clear selection with -1.
    Selection(i32),
    /// Request a pixel scroll offset, clamped by the destination.
    Offset(i32),
    /// Request a nonnegative split divider position.
    Position(i32),
}
impl Action {
    /// Destination command identity, independent of constant-versus-forwarded payload policy.
    pub fn input(&self) -> InputKind {
        match self {
            Self::Forward(kind) => kind.clone(),
            Self::Text(_) => InputKind::Text,
            Self::Checked(_) => InputKind::Checked,
            Self::Value(_) => InputKind::Value,
            Self::Enabled(_) => InputKind::Enabled,
            Self::Expanded(_) => InputKind::Expanded,
            Self::Selection(_) => InputKind::Selection,
            Self::Offset(_) => InputKind::Offset,
            Self::Position(_) => InputKind::Position,
        }
    }

    /// Checks payload compatibility and constant domain before any widget is constructed.
    pub fn validate(&self, source: OutputType, target: &WidgetSpec) -> Result<(), String> {
        if !target.accepts(&self.input()) {
            return Err("Connection target does not accept this command".into());
        }
        let valid = match self {
            Self::Forward(kind) => matches!(
                (source, kind),
                (OutputType::Checked, InputKind::Checked | InputKind::Enabled | InputKind::Expanded)
                    | (OutputType::Number, InputKind::Value)
                    | (OutputType::TextChanged | OutputType::TextSubmitted, InputKind::Text)
                    | (OutputType::Offset | OutputType::Position, InputKind::Offset | InputKind::Position)
                    | (OutputType::Selection, InputKind::Selection)
            ),
            Self::Value(value) => value.is_finite(),
            Self::Position(value) => *value >= 0,
            Self::Selection(value) => match target {
                WidgetSpec::Combo { labels, .. } => *value >= -1 && (*value < 0 || (*value as usize) < labels.len()),
                _ => false,
            },
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err("Connection payload is incompatible or invalid".into())
        }
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;

    /// Bad graph endpoints, payloads, constants, and competing destinations fail before construction.
    #[test]
    fn invalid_connections_fail_validation() {
        let mut document = Document::new("connections");
        let connection = Connection {
            source: "save".into(),
            port: "activated".into(),
            target: "title".into(),
            action: Action::Text("changed".into()),
        };
        document.connections.push(connection.clone());
        assert!(document.validate().is_err()); // The port already has a public export.
        document.outputs.clear();
        document.validate().unwrap();
        document.connections.push(connection);
        assert!(document.validate().is_err());
        document.connections.pop();
        for action in [Action::Forward(InputKind::Text), Action::Checked(true), Action::Value(f32::NAN)] {
            document.connections[0].action = action;
            assert!(document.validate().is_err());
        }
        document.connections[0].action = Action::Text("ok".into());
        document.connections[0].target = "missing".into();
        assert!(document.validate().is_err());
        document.connections[0].target = "title".into();
        document.connections[0].port = "missing".into();
        assert!(document.validate().is_err());
    }
}
