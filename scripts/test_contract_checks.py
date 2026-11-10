"""Exercise declaration ownership and child environment boundaries without builds."""

import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from contract_check_support import child_environment


def load_script(name):
    """Load the maintained CLI as a module without executing its main function."""
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


consumers = load_script("check-contract-consumers")


class ContractDependencies(unittest.TestCase):
    """Transitive compilation cannot substitute for a package-local request."""

    def test_each_direct_owner_omission_is_rejected(self):
        """Removing any one request fails, including nodes/demo's masked requests."""
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            for consumer in consumers.CONSUMERS:
                if consumer.implements_objects:
                    manifest = repository / consumer.path / "Cargo.toml"
                    manifest.parent.mkdir(parents=True, exist_ok=True)
                    manifest.write_text('[dependencies]\nneocogi-com={workspace=true,features=["alloc","macros"]}\n')
            consumers.check_owner_requests(repository)
            for consumer in consumers.CONSUMERS:
                if not consumer.implements_objects:
                    continue
                manifest = repository / consumer.path / "Cargo.toml"
                original = manifest.read_text()
                manifest.write_text(original.replace(',"macros"', ''))
                with self.subTest(owner=consumer.package), self.assertRaisesRegex(ValueError, consumer.path):
                    consumers.check_owner_requests(repository)
                manifest.write_text(original)

    def test_fixture_dependencies_match_library_or_local_generation(self):
        """UI consumers have one normal library edge; local generation uses the SDK."""
        import tomllib
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            # Independent graphs inherit the production registry version,
            # while their facade deliberately selects allocation without macros.
            (root / "Cargo.toml").write_text(
                '[workspace.dependencies]\n'
                'neocogi-com={version="=0.1.0",default-features=false}\n'
                'neocogi-idl={version="=0.1.0"}\n'
                'neocogi-platform-contracts={version="=0.2.0"}\n'
            )
            for index, consumer in enumerate(consumers.CONSUMERS):
                directory = root / str(index)
                consumers.write_consumer(consumer, root, directory)
                data = tomllib.loads((directory / "Cargo.toml").read_text())
                self.assertNotIn("dev-dependencies", data)
                if consumer.generated_module:
                    self.assertEqual(set(data["dependencies"]), {"neocogi-com"})
                    self.assertEqual(data["dependencies"]["neocogi-com"]["features"], ["alloc"])
                    self.assertFalse(data["dependencies"]["neocogi-com"]["default-features"])
                    self.assertEqual(set(data["build-dependencies"]), {"neocogi-idl", "neocogi-platform-contracts"})
                    self.assertEqual(data["build-dependencies"]["neocogi-platform-contracts"]["version"], "=0.2.0")
                    self.assertEqual(data["build-dependencies"]["neocogi-platform-contracts"]["features"], ["build-support"])
                    for dependency in (data["dependencies"]["neocogi-com"], data["build-dependencies"]["neocogi-idl"]):
                        self.assertEqual(dependency["version"], "=0.1.0")
                        self.assertNotIn("git", dependency)
                        self.assertNotIn("rev", dependency)
                        self.assertNotIn("path", dependency)
                else:
                    self.assertEqual(set(data["dependencies"]), {"library"})
                    self.assertEqual(data["dependencies"]["library"]["package"], consumer.package)
                    self.assertNotIn("build-dependencies", data)


class ChildEnvironment(unittest.TestCase):
    """Resolve in repository cwd and preserve caller compiler/cache/network policy."""

    def test_selection_and_relative_cache_survive_external_cwd(self):
        """The selected compiler reaches children; relative paths keep caller meaning."""
        with patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": "caller-choice", "CARGO_TARGET_DIR": "cache with spaces"}, clear=True):
            with patch("contract_check_support.subprocess.check_output", return_value="caller-choice (environment override)\n") as active:
                environment = child_environment(Path("/repository"))
            self.assertEqual(active.call_args.kwargs["cwd"], Path("/repository"))
            self.assertEqual(active.call_args.kwargs["env"]["RUSTUP_TOOLCHAIN"], "caller-choice")
            self.assertEqual(environment["RUSTUP_TOOLCHAIN"], "caller-choice")
            self.assertEqual(environment["CARGO_TARGET_DIR"], str(Path("cache with spaces").resolve()))
            self.assertEqual(os.environ["CARGO_TARGET_DIR"], "cache with spaces")

    def test_no_cache_is_created_or_claimed_by_environment_selection(self):
        """The caller owns its scratch target policy; selection adds no runtime state."""
        with patch.dict(os.environ, {}, clear=True), patch("contract_check_support.subprocess.check_output", return_value="workspace-nightly (override)\n"):
            environment = child_environment(Path("/repository"))
        self.assertEqual(environment["RUSTUP_TOOLCHAIN"], "workspace-nightly")
        self.assertNotIn("CARGO_TARGET_DIR", environment)
        self.assertNotIn("CARGO_NET_GIT_FETCH_WITH_CLI", environment)

    def test_explicit_git_client_policy_is_preserved(self):
        """Registry consumers still preserve an explicit caller's Git configuration."""
        with patch.dict(os.environ, {"CARGO_NET_GIT_FETCH_WITH_CLI": "false"}, clear=True), patch(
            "contract_check_support.subprocess.check_output", return_value="workspace-nightly\n"
        ):
            environment = child_environment(Path("/repository"))
        self.assertEqual(environment["CARGO_NET_GIT_FETCH_WITH_CLI"], "false")

    def test_cargo_network_policy_reaches_both_consumer_commands(self):
        """Fresh runners may fetch; explicit true/false reach event and facade checks."""
        for offline in (None, "true", "false"):
            caller = {} if offline is None else {"CARGO_NET_OFFLINE": offline}
            with self.subTest(offline=offline), patch.dict(os.environ, caller, clear=True):
                with patch("contract_check_support.subprocess.check_output", return_value="workspace-nightly\n"):
                    environment = child_environment(Path("/repository"))
                self.assertEqual(environment.get("CARGO_NET_OFFLINE"), offline)
                self.assertEqual(os.environ, caller)
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    (root / "Cargo.toml").write_text((Path(__file__).resolve().parents[1] / "Cargo.toml").read_text())
                    for index, consumer in enumerate((consumers.CONSUMERS[0], consumers.CONSUMERS[-1])):
                        target = root / str(index) / "target"
                        facade = "\n".join(json.dumps({"reason": "compiler-artifact", "target": {
                            "name": "neocogi_com", "kind": ["lib"]}, "features": features,
                            "filenames": [str(target / relative)]}) for features, relative in [
                                (["macros"], "debug/deps/host.rmeta"),
                                (["alloc"], "x86_64-unknown-linux-gnu/debug/deps/target.rmeta")])
                        with patch.object(consumers.subprocess, "run") as run, patch.object(
                            consumers.subprocess, "check_output", side_effect=["host: x86_64-unknown-linux-gnu\n", facade]
                        ) as output:
                            consumers.check_consumer(consumer, root, root / str(index), environment)
                        launched = run if consumer.implements_objects else output
                        self.assertEqual(launched.call_count, 1 if consumer.implements_objects else 2)
                        self.assertNotIn("--offline", launched.call_args.args[0])
                        self.assertEqual(launched.call_args.kwargs["env"].get("CARGO_NET_OFFLINE"), offline)


if __name__ == "__main__":
    unittest.main()
