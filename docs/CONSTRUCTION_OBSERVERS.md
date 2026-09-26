# Superseded construction-observer design

The constructor-observer protocol has been replaced by ordinary generic COM
interfaces and concrete class event streams. See [the current design](CLASS_EVENT_STREAMS.md)
for IDL, implementation, generated forms, COM discovery, and lifetime rules.

The previous design and its binary-size measurements remain in Git history.
They do not describe the current attachment protocol or its performance.
