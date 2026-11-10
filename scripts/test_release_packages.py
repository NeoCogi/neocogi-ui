"""Exercise archive admission, exact release ownership and derived fixture freshness."""
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import check_packages as packages
import package_inputs


def metadata():
    """Minimal real metadata shape has one reusable owner and one private application."""
    public = [{"id": name, "name": name, "version": packages.RELEASE,
               "manifest_path": str(packages.ROOT / "crates" / directory / "Cargo.toml"),
               "publish": ["crates-io"], "dependencies": []}
              for name, directory in [(packages.SCHEMA_OWNER, "ui-contracts"), ("neocogi-ui", "ui"), ("neocogi-ui-dylib", "ui-dylib")]]
    private = {"id": "app", "name": "application", "version": packages.RELEASE,
               "manifest_path": str(packages.ROOT / "examples/app/Cargo.toml"), "publish": [], "dependencies": []}
    return {"workspace_members": [p["id"] for p in public] + ["app"], "packages": public + [private]}


class ReleaseSelection(unittest.TestCase):
    """Registry policy must reject incomplete, mixed or accidentally expanded releases."""
    def test_private_examples_and_exact_release_are_enforced(self):
        current = metadata()
        self.assertEqual(set(packages.release_packages(current)), packages.PUBLIC_PACKAGES)
        for field, value in [("publish", ["crates-io"]), ("version", "0.0.0")]:
            changed = copy.deepcopy(current)
            changed["packages"][-1][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): packages.release_packages(changed)

    def test_private_and_floating_internal_dependencies_are_rejected(self):
        for name, requirement in [("application", "=" + packages.RELEASE), (packages.SCHEMA_OWNER, packages.RELEASE)]:
            current = metadata()
            current["packages"][0]["dependencies"] = [{"name": name, "path": "/local", "kind": "dev", "req": requirement}]
            with self.subTest(name=name), self.assertRaises(ValueError): packages.release_packages(current)

    def test_missing_or_additional_public_package_is_rejected(self):
        current = metadata()
        current["packages"].pop(1)
        with self.assertRaisesRegex(ValueError, "exactly"):
            packages.release_packages(current)
        current = metadata()
        extra = copy.deepcopy(current["packages"][0])
        extra.update(id="extra", name="extra")
        current["packages"].append(extra)
        current["workspace_members"].append("extra")
        with self.assertRaisesRegex(ValueError, "exactly"):
            packages.release_packages(current)

    def test_internal_dev_dependencies_are_exact_and_self_contained(self):
        current = metadata()
        current["packages"][0]["dependencies"] = [{"name": "neocogi-ui", "path": "/local", "kind": "dev", "req": "=" + packages.RELEASE}]
        self.assertEqual(set(packages.release_packages(current)), packages.PUBLIC_PACKAGES)
        current["packages"][0]["dependencies"][0]["req"] = "*"
        with self.assertRaisesRegex(ValueError, "exact"):
            packages.release_packages(current)

    def test_publication_preflight_is_explicitly_nonuploading_and_selective(self):
        for preflight in [False, True]:
            command = packages.archive_command(["cargo", "+nightly"], {"peer", "owner"}, [] if preflight else ["--offline"],
                                               Path("/temporary resolution/config.toml"),
                                               allow_dirty=False, publish_dry_run=preflight)
            self.assertEqual(command[:4] if preflight else command[:3],
                             ["cargo", "+nightly", "publish", "--dry-run"] if preflight else ["cargo", "+nightly", "package"])
            self.assertEqual(command[-4:], ["-p", "owner", "-p", "peer"])
            self.assertNotIn("--allow-dirty", command)
            self.assertIn("/temporary resolution/config.toml", command)
            self.assertIn("--no-verify", command)


class CommittedQualification(unittest.TestCase):
    """Uncommitted tooling outside public package directories still invalidates release evidence."""
    def test_dirty_tooling_fails_before_preparing_or_building_packages(self):
        with patch.object(packages, "run", return_value=" M scripts/check_packages.py\n") as run, patch.object(packages, "reconcile") as prepare:
            with self.assertRaisesRegex(ValueError, "clean committed tree"):
                packages.qualify()
            run.assert_called_once_with(["git", "status", "--porcelain"], capture=True)
            prepare.assert_not_called()

    def test_explicit_development_override_reaches_preparation(self):
        with patch.object(packages, "run") as run, patch.object(packages, "reconcile", side_effect=RuntimeError("preparation boundary")) as prepare:
            with self.assertRaisesRegex(RuntimeError, "preparation boundary"):
                packages.qualify(allow_dirty=True)
            run.assert_not_called()
            prepare.assert_called_once_with(check=True)

    def test_offline_publication_preflight_fails_before_registry_or_source_work(self):
        with patch.object(packages, "run") as run, patch.object(packages, "reconcile") as prepare:
            with self.assertRaisesRegex(ValueError, "needs registry access"):
                packages.qualify(offline=True, publish_dry_run=True)
            run.assert_not_called()
            prepare.assert_not_called()


class ValidationIsolation(unittest.TestCase):
    """Generated Cargo children must share archive sources without inheriting checkout policy."""

    def test_private_target_and_archive_patches_preserve_the_callers_configuration(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            original = base / "original-home"
            original.mkdir()
            for name in ("registry", "git"):
                (original / name).mkdir()
            config = original / "config.toml"
            config.write_text('[build]\nrustflags=["--cfg", "caller_setting"]\n')
            outside = base / "validation"
            outside.mkdir()
            locations = {"owner": outside / "crates/owner"}
            with patch.dict(os.environ, {"CARGO_HOME": str(original)}):
                environment = packages.validation_environment(outside, {"owner"}, locations, "fixture-nightly", offline=True)
            home = Path(environment["CARGO_HOME"])
            self.assertEqual(Path(environment["CARGO_TARGET_DIR"]), outside / "target")
            self.assertEqual(environment["RUSTUP_TOOLCHAIN"], "fixture-nightly")
            self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
            for name in ("registry", "git"):
                self.assertEqual((home / name).resolve(), original / name)
            self.assertEqual(tomllib.loads((home / "config.toml").read_text()), {
                "patch": {"crates-io": {"owner": {"path": str(locations["owner"])}}}})
            self.assertEqual(config.read_text(), '[build]\nrustflags=["--cfg", "caller_setting"]\n')

    def test_validation_inside_checkout_is_rejected_before_creating_configuration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(packages, "ROOT", root):
                for outside in (root, root / "target/generated"):
                    with self.subTest(outside=outside), self.assertRaisesRegex(ValueError, "outside the checkout"):
                        packages.validation_environment(outside, {}, {}, "fixture-nightly", offline=True)
                    self.assertFalse((outside / "cargo-home").exists())

    def test_real_child_cargo_configuration_is_isolated_from_checkout_ancestors(self):
        toolchain = tomllib.loads((packages.ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            checkout = base / "checkout"
            (checkout / ".cargo").mkdir(parents=True)
            (checkout / ".cargo/config.toml").write_text('[build]\nrustflags=["--cfg", "checkout_settings"]\n')
            outside = base / "validation"
            outside.mkdir()
            with patch.object(packages, "ROOT", checkout):
                environment = packages.validation_environment(outside, {}, {}, toolchain, offline=True)
            # Let the actual ancestor configuration decide the canary, independent of caller flags.
            for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS"):
                environment.pop(name, None)
            for directory, isolated in ((checkout / "target/tests/trybuild/canary", False),
                                        (Path(environment["CARGO_TARGET_DIR"]) / "tests/trybuild/canary", True)):
                with self.subTest(isolated=isolated):
                    (directory / "src").mkdir(parents=True)
                    (directory / "Cargo.toml").write_text('[package]\nname="archive-config-canary"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
                    source = '#![allow(unexpected_cfgs)]\n#[cfg(checkout_settings)]\ncompile_error!("checkout configuration leaked");\nfn main() {}\n'
                    (directory / "src/main.rs").write_text(source)
                    result = subprocess.run(["cargo", "+" + toolchain, "check", "--offline", "--quiet"],
                                            cwd=directory, env=environment, text=True,
                                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30)
                    if isolated:
                        self.assertEqual(result.returncode, 0, result.stdout)
                    else:
                        self.assertNotEqual(result.returncode, 0, result.stdout)
                        self.assertIn("checkout configuration leaked", result.stdout)


class ArchiveAdmission(unittest.TestCase):
    """Malformed archives are rejected through valid tar storage before extraction."""
    def inspect(self, *, dependency="", extra=None, schemas=32, omit_license=False):
        """Build bounded tar inputs with actual normalized manifest text and arbitrary member kinds."""
        package = metadata()["packages"][0]
        prefix = package["name"] + "-" + packages.RELEASE
        manifest = '[package]\nname="' + package["name"] + '"\nversion="' + packages.RELEASE + '"\n' + dependency
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / "release.crate"
            with tarfile.open(archive, "w:gz") as bundle:
                def add(name, data):
                    member = tarfile.TarInfo(prefix + "/" + name)
                    member.size = len(data)
                    bundle.addfile(member, io.BytesIO(data))
                add("Cargo.toml", manifest.encode())
                add("README.md", b"readme")
                if not omit_license: add("LICENSE", b"notices")
                for index in range(schemas): add(f"schemas/neocogi/test/{index}.ncidl", b"declaration")
                if extra is not None: bundle.addfile(extra)
            return packages.archive_manifest(archive, package)

    def test_complete_owner_archive_is_accepted(self):
        self.assertEqual(self.inspect(), packages.SCHEMA_OWNER + "-" + packages.RELEASE)

    def test_path_escape_link_and_required_input_fail_before_extraction(self):
        for name, kind in [("/outside", tarfile.REGTYPE), ("../outside", tarfile.REGTYPE),
                           (packages.SCHEMA_OWNER + "-" + packages.RELEASE + "/link", tarfile.SYMTYPE)]:
            member = tarfile.TarInfo(name)
            member.type = kind
            member.linkname = "../../outside"
            with self.subTest(name=name), self.assertRaises(ValueError): self.inspect(extra=member)
        with self.assertRaisesRegex(ValueError, "missing archive input"): self.inspect(omit_license=True)

    def test_nonregistry_dependencies_and_incomplete_schema_inventory_fail(self):
        for source in ['path="../sibling",version="1"', 'git="https://example.invalid/repo",version="1"', 'workspace=true', 'features=[]']:
            with self.subTest(source=source), self.assertRaises(ValueError): self.inspect(dependency='[dependencies]\npeer={' + source + '}\n')
        with self.assertRaisesRegex(ValueError, "schema ownership"): self.inspect(schemas=31)


class ProviderArtifacts(unittest.TestCase):
    """Cargo all-target output includes test harnesses carrying cdylib target metadata."""
    def test_final_library_is_not_overwritten_by_a_test_executable(self):
        for extension in ["so", "dylib", "dll"]:
            library = {"reason": "compiler-artifact", "package_id": "path+file:///extracted/dylib#neocogi-ui-dylib@" + packages.RELEASE,
                       "target": {"crate_types": ["cdylib"]}, "profile": {"test": False},
                       "filenames": ["/output/libnc_ui." + extension, "/output/debug-sidecar"]}
            harness = copy.deepcopy(library)
            harness["profile"]["test"] = True
            harness["filenames"] = ["/output/build/neocogi-ui-dylib/test-harness"]
            for records in [[library, harness], [harness, library]]:
                stream = "\n".join(json.dumps(record) for record in records)
                self.assertEqual(packages.provider_libraries(stream), {"neocogi-ui-dylib": library["filenames"][0]})
        library["filenames"] = ["/output/executable"]
        with self.assertRaisesRegex(ValueError, "final provider library"):
            packages.provider_libraries(json.dumps(library))


class DerivedInputs(unittest.TestCase):
    """A changed canonical fixture cannot silently publish stale or unclaimed derived bytes."""
    def test_repair_freshness_and_pruning_are_scoped_to_owned_outputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            files = {"LICENSE": "project", "LICENSES/MIT": "notice", "Cargo.toml": '[workspace.package]\nversion="' + packages.RELEASE + '"\n', "README.md": "application",
                     "assets/WHITE.png": "pixels", "themes/classic/theme.json": "theme",
                     "examples/ui-samples/Cargo.toml": '[package]\nname="sample"\npublish=false\n',
                     "examples/assets/src/recipe.rs": "recipe", "examples/assets/src/recipe_data.rs": "data",
                     "scripts/com_dependencies.rs": "helper", "crates/ui-contracts/docs/button_events.rs": "walkthrough",
                     "crates/ui/docs/document.md": "document", "examples/ui-samples/schemas/neocogi/demo.ncidl": "schema", "crates/ui/tests/document/fixtures/view.rs": "binding",
                     "crates/ui/Cargo.toml": '[package]\nname="fixture"\npublish=["crates-io"]\n'}
            for name, contents in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(contents)
            package_inputs.reconcile(check=False, root=root)
            self.assertEqual(package_inputs.reconcile(check=True, root=root), [])
            owned = root / "crates/ui/tests/package-inputs"
            stale = owned / "stale"
            stale.write_text("unclaimed")
            (root / "assets/WHITE.png").write_text("new pixels")
            untouched = root / "crates/ui/unrelated"
            untouched.write_text("keep")
            with self.assertRaisesRegex(ValueError, "stale derived"): package_inputs.reconcile(check=True, root=root)
            package_inputs.reconcile(check=False, root=root)
            self.assertFalse(stale.exists())
            self.assertEqual((owned / "assets/WHITE.png").read_text(), "new pixels")
            self.assertEqual(untouched.read_text(), "keep")


if __name__ == "__main__":
    unittest.main()
