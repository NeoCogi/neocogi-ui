#!/usr/bin/env python3
"""Execute independent UI event consumers and generated-only COM SDK controls.

Each temporary Cargo workspace has one normal contract dependency and no COM or
dev dependency. Direct owner declarations are checked separately: a mandatory
transitive package can otherwise supply a missing macro feature. Python 3.11+
and the workspace-selected Rust toolchain are required. Commands run sequentially.
"""

from dataclasses import dataclass
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

from contract_check_support import child_environment


@dataclass(frozen=True)
class Consumer:
    """One actual package and its independent executable acceptance body."""

    package: str
    """Published Cargo package selected through its real checkout manifest."""
    path: str
    """Repository-relative directory; no package source is copied or replaced."""
    body: str
    """Rust main body that exercises the public contract without importing COM."""
    implements_objects: bool = True
    """Concrete event owners must declare macros; pure definitions must not need it."""


CONSUMERS = (
    Consumer("neocogi-ui-core-contracts", "crates/ui/core-contracts", '''
        let owner = contract::events::text_changed("éx", 2).unwrap();
        {
            let fact = owner.query_ref::<contract::ITextChangedEventAbi>().unwrap();
            assert_eq!(fact.cursor().unwrap(), 2);
        }
        drop(owner);
        assert!(contract::events::text_changed("éx", 1).is_err());
    '''),
    Consumer("neocogi-ui-nodes-contracts", "crates/ui/nodes-contracts", '''
        let owner = contract::events::numeric_changed(1.5).unwrap();
        {
            let fact = owner.query_ref::<contract::INumericChangedEventAbi>().unwrap();
            assert_eq!(fact.value().unwrap(), 1.5);
        }
        drop(owner);
        assert!(contract::events::numeric_changed(f32::NAN).is_err());
    '''),
    Consumer("neocogi-idl-example-contracts", "tools/idl/example-contracts", '''
        let mut delivery = contract::notifications::ExtentDelivery::Unobserved;
        delivery.changed(contract::Extent { width: 640, height: 480 }).unwrap();
        let _ = contract::ExtentBinding::Unobserved;
    ''', implements_objects=False),
    Consumer("neocogi-demo-contracts", "demos/contracts", '''
        let owner = contract::events::counter_changed(73).unwrap();
        {
            let fact = owner.query_ref::<contract::ICounterChangedEventAbi>().unwrap();
            assert_eq!(fact.value().unwrap(), 73);
        }
        drop(owner);
    '''),
    Consumer("neocogi-renderer-contracts", "crates/render/contracts", '''
        let _: Option<contract::IRendererAbi> = None;
    ''', implements_objects=False),
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
    (directory / "src/main.rs").write_text("fn main() {\n" + consumer.body + "\n}\n", encoding="utf-8")
    (directory / "Cargo.toml").write_text(
        '[package]\nname="contract-consumer-check"\nversion="0.0.0"\nedition="2024"\n'
        '[workspace]\n[dependencies]\ncontract={package=' + json.dumps(consumer.package) +
        ',path=' + json.dumps(str(repository / consumer.path)) + '}\n', encoding="utf-8",
    )


def check_consumer(consumer, repository, directory, environment):
    """Run actual event code, or inspect this invocation's pure facade features."""
    write_consumer(consumer, repository, directory)
    command = ["cargo", "run" if consumer.implements_objects else "check",
               "--manifest-path", str(directory / "Cargo.toml")]
    print("checking " + consumer.package, flush=True)
    if consumer.implements_objects:
        subprocess.run(command, cwd=directory, env=environment, check=True)
    else:
        output = subprocess.check_output(command + ["--message-format=json"], cwd=directory,
                                         env=environment, text=True)
        # Dependency proc macros used by the host IDL compiler are irrelevant.
        # The target facade artifact reports its actual selected feature set.
        facades = [record for line in output.splitlines() if (record := json.loads(line)).get("reason") == "compiler-artifact"
                   and record["target"]["name"] == "neocogi_com" and record["target"]["kind"] == ["lib"]]
        if not facades or any("macros" in record["features"] for record in facades):
            raise ValueError("generated-only consumer must compile the facade without COM macros")
        print(json.dumps({"package": consumer.package, "facade_features": [row["features"] for row in facades]}), flush=True)


def main():
    """Check declared owners, then five separate graphs with one owned scratch root."""
    repository = Path(__file__).resolve().parents[1]
    check_owner_requests(repository)
    environment = child_environment(repository)
    print("toolchain: " + environment["RUSTUP_TOOLCHAIN"], flush=True)
    with tempfile.TemporaryDirectory(prefix="neocogi-contract-consumers-") as temporary:
        root = Path(temporary)
        environment.setdefault("CARGO_TARGET_DIR", str(root / "target"))
        for index, consumer in enumerate(CONSUMERS):
            check_consumer(consumer, repository, root / str(index), environment)
    print("three independent UI event consumers and two generated-only controls passed", flush=True)


if __name__ == "__main__":
    main()
