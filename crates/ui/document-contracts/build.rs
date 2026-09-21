//! Builds the opaque loaded-component boundary and concrete input messages.
fn main() {
    neocogi_idl::cargo::Module::directory("idl", "neocogi/ui/document")
        .build()
        .expect("failed to build UI document contracts");
}
