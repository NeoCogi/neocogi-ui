"""Exercise archive admission, exact release ownership and derived fixture freshness."""
import copy
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import check_packages as packages
import package_inputs


def metadata():
    """Minimal real metadata shape has one reusable owner and one private application."""
    public = {"id": "build", "name": packages.SCHEMA_OWNER, "version": packages.RELEASE,
              "manifest_path": str(packages.ROOT / "crates/build/Cargo.toml"), "publish": ["crates-io"], "dependencies": []}
    private = {"id": "app", "name": "application", "version": packages.RELEASE,
               "manifest_path": str(packages.ROOT / "examples/app/Cargo.toml"), "publish": [], "dependencies": []}
    return {"workspace_members": ["build", "app"], "packages": [public, private]}


class ReleaseSelection(unittest.TestCase):
    """Registry policy must reject incomplete, mixed or accidentally expanded releases."""
    def test_private_examples_and_exact_release_are_enforced(self):
        current = metadata()
        self.assertEqual(set(packages.release_packages(current)), {packages.SCHEMA_OWNER})
        for field, value in [("publish", ["crates-io"]), ("version", "0.0.0")]:
            changed = copy.deepcopy(current)
            changed["packages"][1][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): packages.release_packages(changed)

    def test_private_and_floating_internal_dependencies_are_rejected(self):
        for name, requirement in [("application", "=" + packages.RELEASE), (packages.SCHEMA_OWNER, packages.RELEASE)]:
            current = metadata()
            current["packages"][0]["dependencies"] = [{"name": name, "path": "/local", "kind": "dev", "req": requirement}]
            with self.subTest(name=name), self.assertRaises(ValueError): packages.release_packages(current)


class ArchiveAdmission(unittest.TestCase):
    """Malformed archives are rejected through valid tar storage before extraction."""
    def inspect(self, *, dependency="", extra=None, schemas=31, omit_license=False):
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
        with self.assertRaisesRegex(ValueError, "schema ownership"): self.inspect(schemas=30)


class ProviderArtifacts(unittest.TestCase):
    """Cargo all-target output includes test harnesses carrying cdylib target metadata."""
    def test_final_library_is_not_overwritten_by_a_test_executable(self):
        for extension in ["so", "dylib", "dll"]:
            library = {"reason": "compiler-artifact", "package_id": "path+file:///extracted/dylib#neocogi-gfx-dylib@" + packages.RELEASE,
                       "target": {"crate_types": ["cdylib"]}, "profile": {"test": False},
                       "filenames": ["/output/libnc_gfx." + extension, "/output/debug-sidecar"]}
            harness = copy.deepcopy(library)
            harness["profile"]["test"] = True
            harness["filenames"] = ["/output/build/neocogi-gfx-dylib/test-harness"]
            for records in [[library, harness], [harness, library]]:
                stream = "\n".join(json.dumps(record) for record in records)
                self.assertEqual(packages.provider_libraries(stream), {"neocogi-gfx-dylib": library["filenames"][0]})
        library["filenames"] = ["/output/executable"]
        with self.assertRaisesRegex(ValueError, "final provider library"):
            packages.provider_libraries(json.dumps(library))


class DerivedInputs(unittest.TestCase):
    """A changed canonical fixture cannot silently publish stale or unclaimed derived bytes."""
    def test_repair_freshness_and_pruning_are_scoped_to_owned_outputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            files = {"LICENSE": "project", "LICENSES/MIT": "notice", "Cargo.toml": "pins", "README.md": "application",
                     "assets/WHITE.png": "pixels", "themes/classic/theme.json": "theme",
                     "examples/ui-samples/Cargo.toml": '[package]\nname="sample"\npublish=false\n',
                     "examples/assets/src/recipe.rs": "recipe", "examples/assets/src/recipe_data.rs": "data",
                     "scripts/com_dependencies.rs": "helper", "crates/ui/api/docs/button_events.rs": "walkthrough",
                     "crates/ui/document/README.md": "document", "crates/ui/document/tests/fixtures/view.rs": "binding",
                     "crates/gfx/impl/Cargo.toml": '[package]\nname="fixture"\npublish=["crates-io"]\n'}
            for name, contents in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(contents)
            package_inputs.reconcile(check=False, root=root)
            self.assertEqual(package_inputs.reconcile(check=True, root=root), [])
            owned = root / "crates/gfx/impl/tests/package-inputs"
            stale = owned / "stale"
            stale.write_text("unclaimed")
            (root / "assets/WHITE.png").write_text("new pixels")
            untouched = root / "crates/gfx/impl/unrelated"
            untouched.write_text("keep")
            with self.assertRaisesRegex(ValueError, "stale derived"): package_inputs.reconcile(check=True, root=root)
            package_inputs.reconcile(check=False, root=root)
            self.assertFalse(stale.exists())
            self.assertEqual((owned / "assets/WHITE.png").read_text(), "new pixels")
            self.assertEqual(untouched.read_text(), "keep")


if __name__ == "__main__":
    unittest.main()
