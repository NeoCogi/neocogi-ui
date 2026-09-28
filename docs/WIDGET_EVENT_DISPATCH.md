# Widget-owned event dispatch

`IWidget::dispatch_next(&mut self) -> Result<bool>` is the common private-message
operation. A composite owns its model and receiving endpoint directly. Each call
attempts at most one message; `true` means one message was handled and `false`
means no work was available. A handler error consumes the attempted message and
leaves its successors queued. The COM mutable borrow gate protects the same
allocation used for widget editing and rendering.

Ordinary controls forward their constructor-mapped events to their behavioral
parent. They do not acquire private queues merely by implementing this method.
The built-in `HostedWidget` helper and application `LeafContent` helper provide
no-op defaults. Independent COM implementations return `Ok(false)` when they
have no inbox; IDL methods remain declarations, without implementation bodies.

Dispatch belongs outside input, layout and paint traversal. A handler can edit
other widgets through COM while using ordinary `&mut self` for its own state.
It must not call back into its own COM projection to obtain another mutable
borrow. A request to remove its own subtree must stop subsequent dispatch and
let its active call return before running terminal cleanup.

Closure remains the widget's existing descendant-first, idempotent operation.
Eventful composites additionally disconnect their private receiver, discard its
remaining messages, and release their constructor-selected output mapping.
Messages already posted to the application's inbox belong to the application.
