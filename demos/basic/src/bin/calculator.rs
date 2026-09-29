//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
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
//! Calculator with concrete application state and typed COM button handlers.
use neocogi_ui::*;
use neocogi_demo_host::{AppState, Host, HostError, report_exit};
use neocogi_com::{ComPtr, ComRef, NcStatus, NcValueRef, WeakComPtr};

/// Maximum typed display length; calculated results retain the existing decimal formatting policy.
const DISPLAY_MAX_LEN: usize = 24;
/// Fraction of the client height reserved for the result field.
const DISPLAY_HEIGHT_FRACTION: f32 = 0.20;
/// Every keypad row receives an equal share of the remaining height.
const KEYPAD_ROW_HEIGHT_WEIGHT: f32 = 1.0;
/// Named recipe font selected once through the COM font capability.
const DISPLAY_FONT: &str = "calculator-display";

#[derive(Copy, Clone)]
/// Explicit pending arithmetic operation; no function pointer hides its behavior.
enum Operator {
    /// Add the right operand.
    Add,
    /// Subtract the right operand.
    Subtract,
    /// Multiply by the right operand.
    Multiply,
    /// Divide, rejecting a zero divisor.
    Divide,
}

#[derive(Copy, Clone)]
/// Concrete data carried by each typed button subscription.
enum Action {
    /// Append this decimal digit.
    Digit(char),
    /// Insert one decimal separator.
    Dot,
    /// Toggle the current entry sign.
    ToggleSign,
    /// Commit the current operand and select the next operation.
    Op(Operator),
    /// Evaluate the pending binary operation once.
    Equals,
    /// Reset both operands and the pending operation.
    ClearAll,
    /// Reset only the current editable entry.
    ClearEntry,
    /// Remove the final typed character.
    Backspace,
}

/// Arithmetic model owns the single authoritative entry string and pending operand state.
struct Calculator {
    /// Current entry or formatted result, copied to the textbox only after actions.
    display: String,
    /// Committed left operand while a binary operation is pending.
    accumulator: Option<f64>,
    /// Operation awaiting the current right operand.
    pending: Option<Operator>,
    /// The next digit begins a fresh entry after an operator or result.
    clear_on_input: bool,
    /// Arithmetic failure requires reset before another operation can run.
    error: bool,
}

impl Calculator {
    /// Initializes a zero entry without a pending operation.
    fn new() -> Self {
        Self {
            display: "0".to_string(),
            accumulator: None,
            pending: None,
            clear_on_input: false,
            error: false,
        }
    }

    /// Borrows the authoritative text for one silent textbox synchronization.
    fn display_text(&self) -> &str {
        &self.display
    }

    /// Dispatches one explicit action and preserves the calculator error/reset rules.
    fn apply(&mut self, action: Action) {
        match action {
            Action::ClearAll => {
                self.clear_all();
            }
            Action::ClearEntry => {
                self.clear_entry();
            }
            Action::Backspace => {
                self.recover_from_error();
                self.backspace();
            }
            Action::ToggleSign => {
                self.recover_from_error();
                self.toggle_sign();
            }
            Action::Dot => {
                self.recover_from_error();
                self.insert_dot();
            }
            Action::Digit(digit) => {
                self.recover_from_error();
                self.insert_digit(digit);
            }
            Action::Op(op) => {
                if !self.error {
                    self.set_operator(op);
                }
            }
            Action::Equals => {
                if !self.error {
                    self.evaluate();
                }
            }
        }
    }

    /// Resets the complete arithmetic transaction.
    fn clear_all(&mut self) {
        self.display = "0".to_string();
        self.accumulator = None;
        self.pending = None;
        self.clear_on_input = false;
        self.error = false;
    }

    /// Resets the right-hand entry while retaining a pending left operand.
    fn clear_entry(&mut self) {
        self.display = "0".to_string();
        self.clear_on_input = false;
        self.error = false;
    }

    /// Starts fresh when editing resumes after a failed calculation.
    fn recover_from_error(&mut self) {
        if self.error {
            self.clear_all();
        }
    }

    /// Starts a new entry when needed and enforces the typed character budget.
    fn insert_digit(&mut self, digit: char) {
        if !digit.is_ascii_digit() {
            return;
        }
        if self.clear_on_input {
            self.display = "0".to_string();
            self.clear_on_input = false;
        }

        if self.display == "0" {
            self.display.clear();
        } else if self.display == "-0" {
            self.display = "-".to_string();
        }

        if self.display.len() < DISPLAY_MAX_LEN {
            self.display.push(digit);
        }

        if self.display.is_empty() {
            self.display.push('0');
        }
    }

    /// Keeps at most one decimal separator in the active entry.
    fn insert_dot(&mut self) {
        if self.clear_on_input {
            self.display = "0".to_string();
            self.clear_on_input = false;
        }

        if !self.display.contains('.') {
            if self.display.is_empty() {
                self.display.push('0');
            }
            self.display.push('.');
        }
    }

    /// Changes sign without manufacturing a negative zero entry.
    fn toggle_sign(&mut self) {
        if self.display == "0" || self.display == "0." {
            return;
        }

        if self.display.starts_with('-') {
            self.display.remove(0);
        } else if self.display.len() < DISPLAY_MAX_LEN {
            self.display.insert(0, '-');
        }
    }

    /// Removes one typed character while preserving a valid zero fallback.
    fn backspace(&mut self) {
        if self.clear_on_input {
            self.display = "0".to_string();
            self.clear_on_input = false;
            return;
        }

        self.display.pop();

        if self.display.is_empty() || self.display == "-" {
            self.display = "0".to_string();
        }
    }

    /// Evaluates an existing pending operation before installing the next operator.
    fn set_operator(&mut self, op: Operator) {
        let rhs = self.current_value();

        if let Some(pending) = self.pending {
            if !self.clear_on_input {
                let lhs = self.accumulator.unwrap_or(rhs);
                match Self::compute(lhs, rhs, pending) {
                    Some(result) => {
                        self.accumulator = Some(result);
                        self.display = Self::format_value(result);
                    }
                    None => {
                        self.set_error();
                        return;
                    }
                }
            }
        } else {
            self.accumulator = Some(rhs);
        }

        self.pending = Some(op);
        self.clear_on_input = true;
    }

    /// Consumes the pending operation and makes its result the next left operand.
    fn evaluate(&mut self) {
        let Some(pending) = self.pending else {
            return;
        };

        let rhs = self.current_value();
        let lhs = self.accumulator.unwrap_or(rhs);

        match Self::compute(lhs, rhs, pending) {
            Some(result) => {
                self.display = Self::format_value(result);
                self.accumulator = Some(result);
                self.pending = None;
                self.clear_on_input = true;
            }
            None => {
                self.set_error();
            }
        }
    }

    /// Parses the entry maintained by the concrete editing rules.
    fn current_value(&self) -> f64 {
        self.display.parse::<f64>().unwrap_or(0.0)
    }

    /// Rejects zero division and nonfinite arithmetic before publishing a result.
    fn compute(lhs: f64, rhs: f64, op: Operator) -> Option<f64> {
        let result = match op {
            Operator::Add => lhs + rhs,
            Operator::Subtract => lhs - rhs,
            Operator::Multiply => lhs * rhs,
            Operator::Divide => {
                if rhs == 0.0 {
                    return None;
                }
                lhs / rhs
            }
        };

        if result.is_finite() { Some(result) } else { None }
    }

    /// Formats decimal output, removes redundant fractional zeros and normalizes negative zero.
    fn format_value(value: f64) -> String {
        let mut out = format!("{value:.10}");
        while out.contains('.') && out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
        if out == "-0" || out.is_empty() { "0".to_string() } else { out }
    }

    /// Terminates the failed operation and leaves an explicit recoverable display state.
    fn set_error(&mut self) {
        self.display = "Error".to_string();
        self.accumulator = None;
        self.pending = None;
        self.clear_on_input = true;
        self.error = true;
    }
}

#[neocogi_com::com_object(neocogi_ui::nodes::ButtonEventQueue)]
/// One concrete receiver for the entire keypad; action selection belongs to State.
struct KeypadQueue {
    /// Only weak origins are needed for the button's sole activation variant.
    send: std::sync::mpsc::Sender<WeakComPtr<ButtonAbi>>,
}
impl neocogi_ui::nodes::ButtonEventQueue for KeypadQueue {
    /// Validate foreign payloads before queueing; presentation labels do not identify keys.
    fn post(&mut self, origin: ComRef<'_, ButtonAbi>, event: NcValueRef<'_, neocogi_ui::button::Event>) -> Result<(), NcStatus> {
        neocogi_ui::button::validate(&event.to_owned()?)?;
        let _ = self.send.send(origin.downgrade()?);
        Ok(())
    }
}

/// Local calculator model and editing capability; topology remains in the UI plugin.
struct State {
    /// The fixed calculator panel follows the platform drawable rectangle.
    window: ComPtr<IWindowAbi>,
    /// The result field is edited only after a calculator action changes the display.
    display: TextboxWidget,
    /// Arithmetic and formatting are ordinary concrete Rust state, independent of any plugin.
    calculator: Calculator,
    /// Application-owned keypad associations; no widget retains this model.
    keys: Vec<(WeakComPtr<ButtonAbi>, Action)>,
}
impl AppState for State {
    /// All keypad controls post to one concrete queue and are resolved by identity.
    type Message = WeakComPtr<ButtonAbi>;
    /// Consumes one command using exclusive access to the ordinary arithmetic model.
    fn update(&mut self, origin: WeakComPtr<ButtonAbi>) -> Result<(), NcStatus> {
        if origin.upgrade()?.is_none() {
            return Ok(());
        }
        let Some((_, action)) = self.keys.iter().find(|(key, _)| key.same_object(&origin)) else {
            return Ok(());
        };
        self.calculator.apply(*action);
        self.display.set_text(self.calculator.display_text())
    }

    /// Resizing updates the actual logical window; unchanged geometry is an inexpensive no-op.
    fn frame(&mut self, _host: &Host, size: Size, _seconds: f64) -> Result<bool, NcStatus> {
        self.window.set_rect(Rect {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        })?;
        Ok(true)
    }
}
/// Builds one COM grid of buttons; no separate CalcButton, event port or shadow node list is needed.
fn run() -> Result<(), HostError> {
    neocogi_demo_host::initialize()?;
    let host = Host::new("Calculator", Size { width: 320, height: 420 }, neocogi_demo_assets::load_bundle()?)?;
    let mut display = host.ui.widgets().textbox_unobserved("0")?;
    display.set_named_font(DISPLAY_FONT)?;
    display.replace_flags(WidgetOptions::FRAME | WidgetOptions::ALIGN_RIGHT | WidgetOptions::NO_INTERACT)?;
    let mut grid = host.ui.widgets().grid(
        &[LayoutTrack::Flex { weight: 1.0 }; 4],
        &[LayoutTrack::Flex { weight: KEYPAD_ROW_HEIGHT_WEIGHT }; 5],
    )?;
    let mut tree = host.ui.widgets().linear(FlowDirection::TopToBottom, CrossSize::Stretch)?;
    tree.insert(0, &display, LinearEdge::new(LayoutTrack::Flex { weight: DISPLAY_HEIGHT_FRACTION }))?
        .finish()?;
    tree.insert(1, &grid, LinearEdge::new(LayoutTrack::Flex { weight: 1.0 - DISPLAY_HEIGHT_FRACTION }))?
        .finish()?;
    let window = host.ui.window("Calculator", Rect { x: 0, y: 0, width: 320, height: 420 }, &tree)?;
    window.set_options(WindowOptions::FRAME | WindowOptions::NO_RESIZE | WindowOptions::NO_TITLE)?;
    let mut state = State {
        window,
        display,
        calculator: Calculator::new(),
        keys: Vec::new(),
    };
    let (events_tx, events) = std::sync::mpsc::channel();
    let queue = KeypadQueue::from_value(KeypadQueue { send: events_tx }).query::<ButtonEventQueueAbi>()?;
    for (index, (label, action)) in [
        ("AC", Action::ClearAll),
        ("CE", Action::ClearEntry),
        ("BS", Action::Backspace),
        ("/", Action::Op(Operator::Divide)),
        ("7", Action::Digit('7')),
        ("8", Action::Digit('8')),
        ("9", Action::Digit('9')),
        ("*", Action::Op(Operator::Multiply)),
        ("4", Action::Digit('4')),
        ("5", Action::Digit('5')),
        ("6", Action::Digit('6')),
        ("-", Action::Op(Operator::Subtract)),
        ("1", Action::Digit('1')),
        ("2", Action::Digit('2')),
        ("3", Action::Digit('3')),
        ("+", Action::Op(Operator::Add)),
        ("+/-", Action::ToggleSign),
        ("0", Action::Digit('0')),
        (".", Action::Dot),
        ("=", Action::Equals),
    ]
    .into_iter()
    .enumerate()
    {
        let mut button = host.ui.widgets().button_with_queue(label, Some(queue.as_com_ref()))?;
        state.keys.push((button.as_interface().downgrade()?, action));
        button.replace_flags(WidgetOptions::FRAME | WidgetOptions::ALIGN_CENTER)?;
        grid.insert(index as u32, &button, GridExtent { columns: 1, rows: 1 })?.finish()?;
    }
    host.run(state, events)
}

/// Reports startup and frame failures through formatting retained by the min-size profile.
fn main() -> std::process::ExitCode {
    report_exit(run())
}
