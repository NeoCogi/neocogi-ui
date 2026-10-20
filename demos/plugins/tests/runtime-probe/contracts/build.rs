//! Compile the separately built registry probe's one shared contract source.
fn main() {
    neocogi_idl::cargo::Module::directory("idl", "nc/tests/registry_probe")
        .rust_path("neocogi_runtime_probe_contracts")
        .build()
        .expect("failed to compile the registry probe contract");
}
