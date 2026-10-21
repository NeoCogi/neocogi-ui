"""Exercise declaration ownership and child environment boundaries without builds."""

import importlib.util
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
                    manifest.parent.mkdir(parents=True)
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

    def test_fixture_has_only_its_normal_contract_dependency(self):
        """Permanent consumers cannot mask their package with an extra COM edge."""
        import tomllib
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, consumer in enumerate(consumers.CONSUMERS):
                directory = root / str(index)
                consumers.write_consumer(consumer, root, directory)
                data = tomllib.loads((directory / "Cargo.toml").read_text())
                self.assertEqual(set(data["dependencies"]), {"contract"})
                self.assertNotIn("dev-dependencies", data)
                self.assertEqual(data["dependencies"]["contract"]["package"], consumer.package)


class ChildEnvironment(unittest.TestCase):
    """Resolve in repository cwd and preserve explicit compiler/cache ownership."""

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


if __name__ == "__main__":
    unittest.main()
