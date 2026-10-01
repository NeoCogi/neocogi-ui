//! Explicit authored values; live widget handles and transient state never enter this schema.
use neocogi_com::{ComSerialize, ComDeserialize};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
pub use neocogi_ui_design::{CustomProperty, CustomValue};

/// Persistent schema revision; revision six removes named command inputs.
/// Getters and setters are widget methods. Earlier layouts are rejected, never migrated.
pub const VERSION: u32 = 6;
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
    /// Optional trace/display aliases for complete widget streams; not binding endpoints.
    pub outputs: Vec<Output>,
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
    /// Explicit requested flag replacement, including zero, or no override (None).
    /// Absence preserves the concrete provider's constructor configuration; the
    /// document schema never supplies a competing per-kind defaults table.
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
    /// A leaf control selected through a trusted registered COM factory.
    Custom {
        /// Stable kind resolved only against the application's installed registry.
        kind: String,
        /// Typed constructor values; their schema belongs to the installed factory.
        properties: Vec<CustomProperty>,
    },
}

/// Optional trace/display name for a widget's sole typed event stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ComSerialize, ComDeserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    /// Name shown by authoring tools; bindings still use the node key.
    pub name: String,
    /// Internal node key.
    pub node: String,
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
            Self::Custom { .. } => "Custom",
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
            }],
        }
    }
    /// Rejects invalid shape, names, arithmetic and event aliases before construction.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!("Unsupported UI document revision {}", self.version));
        }
        let mut nodes = BTreeMap::new();
        validate_node(&self.root, 0, &mut nodes)?;
        let mut names = std::collections::BTreeSet::new();
        let mut exported_nodes = std::collections::BTreeSet::new();
        for output in &self.outputs {
            if output.name.is_empty() || !names.insert(&output.name) {
                return Err(format!("Duplicate or empty output: {}", output.name));
            }
            let node = nodes
                .get(output.node.as_str())
                .ok_or_else(|| format!("Output {} refers to missing node {}", output.name, output.node))?;
            if !exported_nodes.insert(&output.node) {
                return Err(format!("Widget {} exports its event stream twice", output.node));
            }
            if node.widget.event_stream().is_none() {
                return Err(format!("{} has no event stream", output.node));
            }
            if nodes.contains_key(output.name.as_str()) && output.name != output.node {
                return Err(format!("Output {} conflicts with a widget event name", output.name));
            }
        }
        for node in nodes.values() {
            if node.widget.event_stream().is_some() && !exported_nodes.contains(&node.key) && names.contains(&node.key) {
                return Err(format!("Widget event name {} conflicts with an exported alias", node.key));
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
    /// Emits readable editable source under the JSON byte policy. Exact counting
    /// rejects oversized output without materializing it, including escaped text.
    /// Binary saveability is checked separately by validate_binary()/the loader.
    pub fn to_json(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        use neocogi_ui_design::bounded_json::{self, Format};
        bounded_json::encoded_len(self, MAX_BYTES, Format::Pretty)?;
        bounded_json::to_vec(self, MAX_BYTES, Format::Pretty)
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
    /// Generated metadata for a built-in event source, including its complete
    /// reachable value graph. Custom kinds obtain equivalent owned metadata through
    /// their COM catalog. Keep class selection here so stream and inspector lookups
    /// cannot drift apart. Widgets without an event stream return `None`.
    pub fn event_source_metadata(&self) -> Option<&'static neocogi_com::design::IdlInterface> {
        use neocogi_ui::*;
        match self {
            Self::Button { .. } => Some(&ButtonAbi::IDL_INTERFACE),
            Self::Checkbox { .. } => Some(&CheckboxAbi::IDL_INTERFACE),
            Self::Slider { .. } => Some(&SliderAbi::IDL_INTERFACE),
            Self::Number { .. } => Some(&NumberAbi::IDL_INTERFACE),
            Self::ListItem { .. } => Some(&ListItemAbi::IDL_INTERFACE),
            Self::ListBox { .. } => Some(&ListBoxAbi::IDL_INTERFACE),
            Self::Textbox { .. } => Some(&TextboxAbi::IDL_INTERFACE),
            Self::TextArea { .. } => Some(&TextAreaAbi::IDL_INTERFACE),
            Self::Scrollbar { .. } => Some(&ScrollbarAbi::IDL_INTERFACE),
            Self::Split { .. } => Some(&SplitViewAbi::IDL_INTERFACE),
            Self::Combo { .. } => Some(&ComboAbi::IDL_INTERFACE),
            Self::TextBlock { .. }
            | Self::ColorSwatch { .. }
            | Self::Linear { .. }
            | Self::Grid { .. }
            | Self::Scroll { .. }
            | Self::Form { .. }
            | Self::Disclosure { .. }
            | Self::Custom { .. } => None,
        }
    }
    /// The built-in class's complete resolved stream, taken from generated IDL metadata.
    /// Custom kinds obtain the equivalent owned description through their COM catalog.
    pub fn event_stream(&self) -> Option<neocogi_com::design::IdlEventQueue> {
        self.event_source_metadata()?.event_queue
    }
    /// Whether authored font selection has a concrete setter, including a disclosure's header.
    pub fn supports_font(&self) -> bool {
        match self {
            Self::Button { .. }
            | Self::Checkbox { .. }
            | Self::Textbox { .. }
            | Self::TextArea { .. }
            | Self::TextBlock { .. }
            | Self::Slider { .. }
            | Self::Number { .. }
            | Self::ColorSwatch { .. }
            | Self::ListItem { .. }
            | Self::ListBox { .. }
            | Self::Combo { .. }
            | Self::Disclosure { .. } => true,
            Self::Scrollbar { .. }
            | Self::Linear { .. }
            | Self::Grid { .. }
            | Self::Split { .. }
            | Self::Scroll { .. }
            | Self::Form { .. }
            | Self::Custom { .. } => false,
        }
    }
    /// Whether a named icon can be applied through the class's concrete visual API.
    pub fn supports_icon(&self) -> bool {
        matches!(self, Self::Button { .. } | Self::ListItem { .. })
    }
    /// Whether an authored image and fit policy have a concrete visual destination.
    pub fn supports_image(&self) -> bool {
        matches!(self, Self::Button { .. } | Self::ListBox { .. })
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
        Custom { kind, properties } => {
            // Bound the whole encoded payload before building a per-property
            // name index. Splitting text across fields cannot evade this budget.
            neocogi_ui_design::validate_properties_size(properties)?;
            let mut names = std::collections::BTreeSet::new();
            !kind.is_empty()
                && properties
                    .iter()
                    .all(|property| !property.name.is_empty() && names.insert(&property.name) && property.value.valid())
        }
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
    if node.style.font.is_some() && !node.widget.supports_font() {
        return Err(format!("{} has no font property", node.key));
    }
    if node.style.icon.is_some() && !node.widget.supports_icon() {
        return Err(format!("{} has no icon property", node.key));
    }
    if node.style.image.is_some() && !node.widget.supports_image() {
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
