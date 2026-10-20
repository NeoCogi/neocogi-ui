//! Compile the private runtime contract using the public core dependency graph.
fn main() {
    neocogi_idl::cargo::Module::directory("idl", "neocogi/ui/runtime/internal")
        .rust_path("nc_ui::choice_menu::bindings")
        .build()
        .expect("failed to compile the runtime-private choice menu contract");
}
