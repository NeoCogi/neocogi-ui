//
// Copyright 2023-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//

//! Retained numeric-entry widget.

use crate::{ControlRole};

use crate::*;

use super::numeric_edit::*;

/// One-shot construction input for a [`Number`].
pub struct NumberParameters {
    /// Validated finite initial number value.
    value: Real,
    /// Validated finite non-negative step applied by dragging and arrow keys.
    step: Real,
    /// Bounded number of digits after the decimal point when rendering.
    precision: DecimalPrecision,
    /// Font used for the numeric label and editor.
    font: FontRef,
    /// Base widget options.
    pub(crate) opt: WidgetOption,
}

impl crate::LeafWidget for Number {
    fn measure(&self, style: &Skin, atlas: &AtlasHandle, constraints: Constraints) -> Dimensioni {
        self.preferred_size_widget(style, atlas, constraints)
    }
}

impl NumberParameters {
    /// Creates number parameters with default widget options.
    ///
    /// # Errors
    ///
    /// Returns `NumericParameterError` when `value` is not finite or `step` is not finite and
    /// non-negative. Precision is already valid by construction through [`DecimalPrecision`].
    pub fn new(value: Real, step: Real, precision: DecimalPrecision) -> Result<Self, NumericParameterError> {
        Self::with_opt(value, step, precision, WidgetOption::FRAME)
    }

    /// Creates number parameters with explicit widget options.
    ///
    /// # Errors
    ///
    /// Returns `NumericParameterError` when `value` is not finite or `step` is not finite and
    /// non-negative. Failed construction publishes no partially normalized parameter value.
    pub fn with_opt(value: Real, step: Real, precision: DecimalPrecision, opt: WidgetOption) -> Result<Self, NumericParameterError> {
        // The numeric fields stay private, so this one gate establishes the invariant consumed by
        // Number construction and every later update/paint path.
        validate_numeric_value_and_step(value, step)?;
        Ok(Self {
            value,
            step,
            precision,
            font: FontRef::Role(FontRole::Body),
            opt,
        })
    }
}

#[neocogi_com::com_class(neocogi_ui_nodes_contracts::Number, ownership = exclusive)]
/// Concrete retained number input and dynamically constructible COM object.
pub struct Number {
    /// Non-semantic COM lifecycle and host-backed phase resources.
    runtime: crate::widget_object::WidgetRuntime,
    /// Initialization-only validated drag and arrow-key step.
    step: Real,
    /// Initialization-only bounded display precision.
    precision: DecimalPrecision,
    /// Initialization-only font.
    font: FontRef,
    /// Base widget options.
    /// Current number value.
    value: Real,
    /// Text editing state for shift-click numeric entry.
    edit: NumberEditState,
    /// Single lifetime event slot; the constructor fixes the parent receiver before publishing the widget.
    events: crate::widget_object::WidgetEvents<neocogi_ui_nodes_contracts::NumberEventQueueAbi>,
}

impl Number {
    /// Combines validated semantic parameters with one already bound common runtime.
    pub(crate) fn from_runtime(runtime: crate::widget_object::WidgetRuntime, parameters: NumberParameters) -> Self {
        // Fallible parameter constructors guarantee finite value and nonnegative finite step.
        Self {
            runtime,
            step: parameters.step,
            precision: parameters.precision,
            font: parameters.font,
            value: parameters.value,
            edit: NumberEditState::default(),
            events: Default::default(),
        }
    }

    /// Returns the current number value.
    pub fn value(&self) -> Real {
        self.value
    }

    /// Updates the current number value when `value` is finite.
    ///
    /// NaN and infinities are ignored so invalid external input cannot erase the last valid
    /// retained value. User interaction uses the same finite-state invariant.
    pub fn set_value(&mut self, value: Real) {
        if value.is_finite() {
            self.value = value;
        }
    }

    /// Returns whether the inline numeric editor is active.
    pub fn is_editing(&self) -> bool {
        self.edit.editing
    }

    /// Test construction consumes the bare algorithm before fixing its real weak origin.
    #[cfg(test)]
    pub(crate) fn into_observed(mut self, queue: ComPtr<neocogi_ui_nodes_contracts::NumberEventQueueAbi>) -> neocogi_com::LocalBox<Self> {
        Self::into_local_box_cyclic::<NumberAbi>(move |origin| {
            self.events = crate::widget_object::WidgetEvents::with_queue(Some(queue), origin);
            self
        })
        .unwrap()
    }

    /// Measures the formatted number label.
    fn preferred_size_widget(&self, style: &Skin, atlas: &AtlasHandle, _constraints: Constraints) -> Dimensioni {
        number_preferred_size(style, atlas, &self.font, self.value, self.precision, 0, 0)
    }

    /// Updates number value from shift-click text entry or horizontal drag.
    fn update_widget(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        let last = self.value;
        if !self.edit.editing {
            // Construction guarantees one non-negative direction policy: Up increases and Down
            // decreases by the same configured amount used by horizontal dragging.
            let amount = f64::from(self.step);
            match ctx.action(input, self.keyboard_behavior()) {
                Some(KeyboardAction::Decrease) => self.value = add_number_delta(self.value, -amount),
                Some(KeyboardAction::Increase) => self.value = add_number_delta(self.value, amount),
                Some(KeyboardAction::Activate | KeyboardAction::Expand | KeyboardAction::Collapse) | None => {}
            }
        }
        if !number_textbox_update(ctx, input, &mut self.edit, self.precision, font, &mut self.value, self.runtime.state.host())
            && ctx.focused()
            && ctx.mouse_buttons().intersects(MouseButton::LEFT)
            && let Some(UiInputEvent::MouseDrag { delta, .. }) = input
        {
            // Widen both multiplication and addition. An i32 drag multiplied by a finite f32 step
            // always fits f64, including cancellation cases whose correct result fits Real even
            // though the intermediate f32 product would be infinite.
            let drag_delta = f64::from(delta.x) * f64::from(self.step);
            self.value = add_number_delta(self.value, drag_delta);
        }
        let changed = (self.value != last).then_some(crate::NumericChanged { value: self.value });
        if let Some(event) = changed {
            ctx.posted(self.events.post(&neocogi_ui_nodes_contracts::NumberEvent::Changed(event)));
        }
        // Only committed value/text changes affect this widget's preferred dimensions.
        crate::Change::from(self.value != last)
    }

    /// Paints either the inline numeric editor or the formatted value.
    fn paint_widget(&self, ctx: &mut WidgetPaintCtx<'_>) {
        let font = ctx.skin().resolve_font(ctx.atlas(), &self.font);
        if self.edit.editing {
            number_textbox_paint(ctx, &self.edit, font);
            return;
        }

        let base = ctx.local_rect();
        if !ctx.options().intersects(WidgetOption::FRAME) {
            // Unframed numeric editors retain the input role's center without structural borders.
            ctx.draw_control_center(ControlRole::TextInput, base);
        }
        let label = number_label(self.value, self.precision);
        ctx.draw_control_text_with_font(font, label.as_str(), base, ControlRole::TextInput, ctx.options());
    }
}

/// Adds one interaction delta in a wider domain and narrows with directional saturation.
fn add_number_delta(value: Real, delta: f64) -> Real {
    // Parameters and retained state are finite f32 values, while pointer deltas are i32. Their
    // complete product and sum fit f64, so clamping there prevents valid cancellation from being
    // lost to an infinite f32 intermediate and keeps overflow at the corresponding finite limit.
    let result = f64::from(value) + delta;
    result.clamp(f64::from(Real::MIN), f64::from(Real::MAX)) as Real
}

impl Widget for Number {
    fn frame_appearance_role(&self) -> FrameRole {
        // Numeric editing shares the single-line text-input background and border contract.
        FrameRole::Control(ControlRole::TextInput)
    }

    fn update(&mut self, ctx: &mut WidgetUpdateCtx<'_>, input: Option<&UiInputEvent>) -> crate::Change {
        self.update_widget(ctx, input)
    }

    fn paint(&self, ctx: &mut WidgetPaintCtx<'_>) {
        self.paint_widget(ctx);
    }

    fn keyboard_behavior(&self) -> KeyboardBehavior {
        // Native text input is useful only while the numeric editor is active.
        let text = if self.edit.editing {
            KeyboardBehavior::TEXT_INPUT
        } else {
            KeyboardBehavior::NONE
        };
        KeyboardBehavior::TAB_STOP | KeyboardBehavior::ADJUST_VERTICAL | text
    }
}

#[cfg(test)]
impl From<NumberParameters> for crate::test_support::algorithm::Algorithm<Number> {
    /// Transfers one-shot parameters into the concrete Number runtime.
    fn from(parameters: NumberParameters) -> Self {
        Self::new(parameters.opt, |runtime| Number::from_runtime(runtime, parameters))
    }
}

impl crate::widget_object::WidgetAlgorithm for Number {

}

impl crate::widget_object::HostedWidget for Number {

    /// Borrows common lifecycle and resource state without a second number representation.
    fn runtime(&self) -> &crate::widget_object::WidgetRuntime {
        &self.runtime
    }

    /// Exclusively borrows common lifecycle and resource state for a COM mutation or phase.
    fn runtime_mut(&mut self) -> &mut crate::widget_object::WidgetRuntime {
        &mut self.runtime
    }
}

impl crate::widget_object::FontState for Number {
    /// Borrows the semantic font reference already used by layout and painting.
    fn font(&self) -> &crate::FontRef {
        &self.font
    }
    /// Replaces the same field after the COM resource owner validates the selection.
    fn set_font(&mut self, font: crate::FontRef) {
        self.font = font;
    }
}

use crate::widget_object::{HostedWidget, direct_leaf_layout_impl, direct_widget_phase_impl, set_direct_font_role, set_direct_named_font};
use neocogi_ui_core_contracts::{GeometryChange, IWidgetHostAbi, FontRole};
use neocogi_ui_nodes_contracts::{NumberAbi, NumberOptions};
use neocogi_com::*;

direct_widget_phase_impl!(Number);
direct_leaf_layout_impl!(Number);

impl Number {
    /// Constructs the class for the host-bound factory through the registry's typed contract.
    pub(crate) fn create_com(
        host: ComPtr<IWidgetHostAbi>,
        resources: crate::widget_object::SharedAssets,
        options: NumberOptions,
        queue: Option<ComPtr<neocogi_ui_nodes_contracts::NumberEventQueueAbi>>,
    ) -> Result<neocogi_com::ComBox<NumberAbi>, NcStatus> {
        let mut value = Self::build(host, options, Some(&resources))?;
        // Fix the sole destination before publishing the completed widget.
        Self::into_box_cyclic::<NumberAbi>(move |origin| {
            value.events = crate::widget_object::WidgetEvents::with_queue(queue, origin);
            value
        })?
        .query().map_err(|(status, _unpublished)| status)
    }

    /// Builds one number editor with either directly imported or runtime-shared resources.
    fn build(host: ComPtr<IWidgetHostAbi>, options: NumberOptions, resources: Option<&crate::widget_object::SharedAssets>) -> Result<Self, NcStatus> {
        let parameters = crate::NumberParameters::new(
            options.value,
            options.step,
            crate::DecimalPrecision::try_from(usize::from(options.precision)).map_err(|_| NC_E_INVALID_ARG)?,
        )
        .map_err(|_| NC_E_INVALID_ARG)?;
        let runtime = crate::widget_object::WidgetRuntime::for_resources(host, parameters.opt, resources)?;
        Ok(Self::from_runtime(runtime, parameters))
    }
}

impl neocogi_ui_nodes_contracts::Number for Number {
    /// Reads the finite value owned by the number editor's concrete semantic algorithm.
    fn value(&self) -> Result<f32, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.value())
    }
    /// Rejects nonfinite input and dirties geometry only when the committed value changes.
    fn set_value(&mut self, value: f32) -> Result<(), NcStatus> {
        self.runtime().state.writable()?;
        if !value.is_finite() {
            return Err(NC_E_INVALID_ARG);
        }
        let previous = self.value();
        self.set_value(value);
        if self.value() != previous {
            self.runtime_mut().state.record_change(GeometryChange::Changed);
        }
        Ok(())
    }
    /// Exposes the same edit mode that controls routed text input.
    fn is_editing(&self) -> Result<bool, NcStatus> {
        self.runtime().state.readable()?;
        Ok(self.is_editing())
    }
    /// Validates a semantic font role before changing the number editor's measurement input.
    fn set_font_role(&mut self, role: FontRole) -> Result<(), NcStatus> {
        set_direct_font_role(self, role)
    }
    /// Resolves one exact named font before replacing the number editor's selection.
    fn set_named_font(&mut self, name: NcCopyFromBufferOffset<'_, u8>) -> Result<(), NcStatus> {
        set_direct_named_font(self, name)
    }
}
