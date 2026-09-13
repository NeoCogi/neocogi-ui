//! Rust conveniences layered over the generated widget-extension values.

use crate::WidgetCalls;

impl Default for WidgetCalls {
    /// Returns counters before any widget phase has run.
    fn default() -> Self {
        Self {
            measures: 0,
            placements: 0,
            updates: 0,
            paints: 0,
            closes: 0,
        }
    }
}
