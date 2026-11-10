#!/usr/bin/env python3
"""Execute independent UI event consumers and generated-only COM SDK controls.

Event consumers use ordinary library APIs. Generated-only controls compile
shared IDL in their own workspace without a target contract package or target COM
macros. Host build tools can use both. Direct owner declarations are checked separately: a mandatory
transitive package can otherwise supply a missing macro feature. Python 3.11+
and the workspace-selected Rust toolchain are required. Commands run sequentially.
"""

from dataclasses import dataclass
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

from contract_check_support import child_environment, registry_dependency


@dataclass(frozen=True)
class Consumer:
    """One actual package and its independent executable acceptance body."""

    package: str
    """Cargo package supplying the actual library or generated-only registry SDK."""
    path: str
    """Repository-relative owner directory, or empty for generated-only registry consumers."""
    body: str
    """Rust main body that exercises the public contract without importing COM."""
    namespace: str = ""
    """Public namespace exposed by the ordinary library dependency."""
    generated_module: str | None = None
    """Generate this shared IDL locally instead of importing library bindings."""
    implements_objects: bool = True
    """Concrete event owners must declare macros; pure definitions must not need it."""


CONSUMERS = (
    Consumer("neocogi-ui-contracts", "crates/ui-contracts", '''
        let owner = contract::events::text_changed("éx", 2).unwrap();
        {
            let fact = owner.query_ref::<contract::ITextChangedEventAbi>().unwrap();
            assert_eq!(fact.cursor().unwrap(), 2);
        }
        drop(owner);
        assert!(contract::events::text_changed("éx", 1).is_err());
    ''', namespace="abi::core"),
    Consumer("neocogi-ui-contracts", "crates/ui-contracts", '''
        let owner = contract::events::numeric_changed(1.5).unwrap();
        {
            let fact = owner.query_ref::<contract::INumericChangedEventAbi>().unwrap();
            assert_eq!(fact.value().unwrap(), 1.5);
        }
        drop(owner);
        assert!(contract::events::numeric_changed(f32::NAN).is_err());
    ''', namespace="abi::nodes"),
    Consumer("neocogi-ui-samples", "examples/ui-samples", '''
        let owner = contract::events::counter_changed(73).unwrap();
        {
            let fact = owner.query_ref::<contract::ICounterChangedEventAbi>().unwrap();
            assert_eq!(fact.value().unwrap(), 73);
        }
        drop(owner);
    ''', namespace="abi"),
    Consumer("neocogi-com", "", '''
        let _: Option<contract::IRendererAbi> = None;
    ''', generated_module="neocogi.renderer", implements_objects=False),
)


def check_owner_requests(repository):
    """Reject missing local requests even if a transitive owner supplies macros."""
    missing = []
    for consumer in CONSUMERS:
        if not consumer.implements_objects:
            continue
        manifest = repository / consumer.path / "Cargo.toml"
        with manifest.open("rb") as source:
            dependency = tomllib.load(source)["dependencies"]["neocogi-com"]
        if "macros" not in dependency.get("features", []):
            missing.append(str(manifest))
    if missing:
        raise ValueError("concrete event owners must directly request neocogi-com/macros: " + ", ".join(missing))


def write_consumer(consumer, repository, directory):
    """Create one independent normal-dependency consumer of the original package."""
    (directory / "src").mkdir(parents=True)
    manifest = ('[package]\nname="contract-consumer-check"\nversion="0.0.0"\nedition="2024"\n'
                '[workspace]\n[dependencies]\n')
    if consumer.generated_module:
        manifest += registry_dependency(repository, "neocogi-com", ["alloc"])
        manifest += '[build-dependencies]\n' + registry_dependency(repository, "neocogi-idl") + registry_dependency(repository, "neocogi-platform-contracts", ["build-support"])
        (directory / "build.rs").write_text(
            'fn main() { for module in neocogi_platform_contracts::build::sources(&[neocogi_platform_contracts::build::Module::Renderer]).unwrap()'
            '.build().unwrap() {'
            'println!("cargo::rustc-env=CONSUMER_BINDINGS={}", module.include.display()); } }\n')
        prefix = 'extern crate self as neocogi_platform_contracts;\nmod abi { include!(env!("CONSUMER_BINDINGS")); }\nuse abi as contract;\n'
    else:
        manifest += 'library={package=' + json.dumps(consumer.package) + ',path=' + json.dumps(str(repository / consumer.path)) + '}\n'
        prefix = 'use library::' + consumer.namespace + ' as contract;\n'
    (directory / "src/main.rs").write_text(prefix + "fn main() {\n" + consumer.body + "\n}\n", encoding="utf-8")
    (directory / "Cargo.toml").write_text(manifest, encoding="utf-8")


def check_consumer(consumer, repository, directory, environment):
    """Run actual event code, or inspect this invocation's pure facade features."""
    write_consumer(consumer, repository, directory)
    command = ["cargo", "run" if consumer.implements_objects else "check",
               "--manifest-path", str(directory / "Cargo.toml")]
    print("checking " + consumer.package, flush=True)
    if consumer.implements_objects:
        subprocess.run(command, cwd=directory, env=environment, check=True)
    else:
        # Separate host build artifacts even when the target is this same machine.
        # The schema package legitimately enables host macros; target definitions do not.
        compiler = subprocess.check_output(["rustc", "-vV"], env=environment, text=True)
        triple = next(line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: "))
        target_root = Path(environment.get("CARGO_TARGET_DIR", directory / "target")).resolve() / triple
        output = subprocess.check_output(command + ["--target", triple, "--message-format=json"], cwd=directory,
                                         env=environment, text=True)
        # Dependency proc macros used by the host IDL compiler are irrelevant.
        # The target facade artifact reports its actual selected feature set.
        facades = [record for line in output.splitlines() if (record := json.loads(line)).get("reason") == "compiler-artifact"
                   and record["target"]["name"] == "neocogi_com" and record["target"]["kind"] == ["lib"]
                   and any(Path(filename).is_relative_to(target_root) for filename in record["filenames"])]
        if not facades or any("macros" in record["features"] for record in facades):
            raise ValueError("generated-only consumer must compile the facade without COM macros")
        print(json.dumps({"package": consumer.package, "facade_features": [row["features"] for row in facades]}), flush=True)


def main():
    """Check declared owners, then four separate graphs with one owned scratch root."""
    repository = Path(__file__).resolve().parents[1]
    check_owner_requests(repository)
    environment = child_environment(repository)
    print("toolchain: " + environment["RUSTUP_TOOLCHAIN"], flush=True)
    with tempfile.TemporaryDirectory(prefix="neocogi-contract-consumers-") as temporary:
        root = Path(temporary)
        environment.setdefault("CARGO_TARGET_DIR", str(root / "target"))
        for index, consumer in enumerate(CONSUMERS):
            check_consumer(consumer, repository, root / str(index), environment)
    print("three independent UI event consumers and one generated-only renderer control passed", flush=True)


if __name__ == "__main__":
    main()
