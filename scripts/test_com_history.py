"""Exercise historical COM source admission independently of real repositories."""

from pathlib import Path
import subprocess
import tempfile
import unittest

from check_com_history import (
    COM_REPOSITORY, COM_URL, declared_revision, dependency_tables, manifest_links,
    message_link, read_ledger, sdk_package, verify,
)


UI_REVISION = "1" * 40
COM_REVISION = "2" * 40


class Ledger(unittest.TestCase):
    """A missing, duplicated or abbreviated row cannot look like complete coverage."""

    def test_full_identifiers_and_explicit_pre_com_state(self):
        links = read_ledger(f"ui_commit\tcom_commit\n{UI_REVISION}\t-\n{'3' * 40}\t{COM_REVISION}\n")
        self.assertIsNone(links[0].com)
        self.assertEqual(links[1].com, COM_REVISION)

    def test_invalid_records_are_rejected(self):
        rows = ("", "wrong\theader\n", "ui_commit\tcom_commit\n",
                f"ui_commit\tcom_commit\n{UI_REVISION[:8]}\t{COM_REVISION}\n",
                f"ui_commit\tcom_commit\n{UI_REVISION}\t{COM_REVISION}\textra\n",
                f"ui_commit\tcom_commit\n{UI_REVISION}\t-\n{UI_REVISION}\t-\n")
        for text in rows:
            with self.subTest(text=text), self.assertRaises(ValueError):
                read_ledger(text)


class Trailers(unittest.TestCase):
    """Both repository and exact revision belong to the historical commit."""

    def test_absent_or_complete_pair(self):
        self.assertIsNone(message_link("Before COM\n"))
        message = f"A UI change\n\nNeoCogi-Com-Repository: {COM_REPOSITORY}\nNeoCogi-Com-Commit: {COM_REVISION}\n"
        self.assertEqual(message_link(message), COM_REVISION)

    def test_partial_duplicate_and_abbreviated_pairs_are_rejected(self):
        repository = f"NeoCogi-Com-Repository: {COM_REPOSITORY}\n"
        revision = f"NeoCogi-Com-Commit: {COM_REVISION}\n"
        for message in (repository, revision, repository + revision + revision,
                        repository + revision + repository,
                        repository + "NeoCogi-Com-Commit: 12345678\n",
                        "NeoCogi-Com-Repository: wrong\n" + revision):
            with self.subTest(message=message), self.assertRaises(ValueError):
                message_link("Malformed COM link\n\n" + message)

    def test_body_examples_are_not_revision_trailers(self):
        message = ("Explain the format\n\n```text\nNeoCogi-Com-Repository: example\n"
                   "NeoCogi-Com-Commit: <commit ID>\n```\n\nThe actual pin lives in Cargo.\n")
        self.assertIsNone(message_link(message))


class ManifestSources(unittest.TestCase):
    """Aliases, conditional edges and artifacts must use the same companion source."""

    def source(self, **changes):
        """Return one normal pinned SDK declaration with deliberate overrides."""
        return {"git": COM_URL, "rev": COM_REVISION, **changes}

    def test_historical_names_and_ui_tooling_have_distinct_ownership(self):
        for name in ("nc-com", "nc-com-runtime-sys", "neocogi-com", "neocogi-idl-build", "neocogi-native-runtime-link"):
            self.assertTrue(sdk_package(name))
        for name in ("neocogi-native-layout", "neocogi-ui-runtime", "neocogi-render-core", "neocogi-computer"):
            self.assertFalse(sdk_package(name))

    def test_workspace_inheritance_and_local_ui_dependencies_are_preserved(self):
        manifest = {"workspace": {"dependencies": {"neocogi-com": self.source()}},
                    "dependencies": {"neocogi-com": {"workspace": True}, "neocogi-ui": {"path": "crates/ui/api"}}}
        self.assertEqual(manifest_links(manifest, COM_REVISION), {"neocogi-com"})

    def test_alias_and_artifact_policy_does_not_change_source_ownership(self):
        declaration = self.source(package="neocogi-com-runtime", artifact="cdylib", target="target", optional=True)
        manifest = {"target": {"cfg(unix)": {"build-dependencies": {"com-library": declaration}}}}
        self.assertEqual(manifest_links(manifest, COM_REVISION), {"neocogi-com-runtime"})

    def test_every_dependency_table_is_inspected(self):
        manifest = {"dependencies": {}, "dev-dependencies": {}, "build-dependencies": {},
                    "workspace": {"dependencies": {}}, "target": {"cfg(unix)": {"dependencies": {}}},
                    "patch": {"some-source": {"sdk": self.source(package="neocogi-com")}}}
        self.assertEqual(len(list(dependency_tables(manifest))), 8)
        self.assertEqual(manifest_links(manifest, COM_REVISION), {"neocogi-com"})

    def test_different_floating_or_local_sources_are_rejected(self):
        declarations = ("0.0.0", {"path": "../com"}, self.source(rev="3" * 40),
                        self.source(git="https://example.invalid/sdk"), self.source(branch="master"),
                        self.source(tag="release"), self.source(path="../com"))
        for dependency in declarations:
            with self.subTest(dependency=dependency), self.assertRaises(ValueError):
                manifest_links({"dependencies": {"neocogi-com": dependency}}, COM_REVISION)

    def test_pre_com_commit_cannot_silently_acquire_an_sdk_dependency(self):
        with self.assertRaises(ValueError):
            manifest_links({"dependencies": {"neocogi-com": self.source()}}, None)

    def test_later_root_pins_reject_mixed_or_unpinned_sources(self):
        self.assertEqual(declared_revision({"workspace": {"dependencies": {"neocogi-com": self.source()}}}), COM_REVISION)
        for dependencies in ({"neocogi-com": {"git": COM_URL}},
                             {"neocogi-com": self.source(rev=123)},
                             {"neocogi-com": self.source(), "neocogi-idl": self.source(rev="3" * 40)}):
            with self.subTest(dependencies=dependencies), self.assertRaises(ValueError):
                declared_revision({"dependencies": dependencies})


class RepositoryAudit(unittest.TestCase):
    """Exercise committed trees and real Git object/ancestry boundaries."""

    def setUp(self):
        """Create small independent UI and COM repositories with two UI epochs."""
        self.temporary = tempfile.TemporaryDirectory(prefix="neocogi-history-test-")
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.ui, self.com = root / "ui", root / "com"
        for repository in (self.ui, self.com):
            repository.mkdir()
            self.git(repository, "init", "-q", "-b", "master")
        (self.com / "Cargo.toml").write_text('[package]\nname="neocogi-com"\nversion="0.0.0"\n')
        self.com_revision = self.commit(self.com, "COM baseline")
        (self.ui / "Cargo.toml").write_text('[package]\nname="ui"\nversion="0.0.0"\n')
        self.before = self.commit(self.ui, "Before COM")
        self.manifest = ('[workspace.dependencies]\nneocogi-com={git="' + COM_URL + '",rev="' + self.com_revision + '"}\n')
        (self.ui / "Cargo.toml").write_text(self.manifest)
        self.linked = self.commit(self.ui, self.message(self.com_revision))
        self.ledger = root / "history.tsv"
        self.ledger.write_text(f"ui_commit\tcom_commit\n{self.before}\t-\n{self.linked}\t{self.com_revision}\n")

    def git(self, repository, *arguments):
        """Run an isolated fixture command without relying on caller identity."""
        return subprocess.check_output(
            ["git", "-C", str(repository), "-c", "user.name=History Tests",
             "-c", "user.email=history@example.invalid", "-c", "commit.gpgsign=false", *arguments],
            stderr=subprocess.PIPE,
        ).decode().strip()

    def commit(self, repository, message):
        """Commit the fixture's exact files and return their real commit ID."""
        self.git(repository, "add", "-A")
        self.git(repository, "commit", "-q", "-m", message)
        return self.git(repository, "rev-parse", "HEAD")

    def message(self, revision):
        """Construct one explicit historical companion link."""
        return f"Use COM\n\nNeoCogi-Com-Repository: {COM_REPOSITORY}\nNeoCogi-Com-Commit: {revision}\n"

    def test_complete_history_and_later_ordinary_commit(self):
        self.assertEqual(verify(self.ui, self.com, self.ledger), (2, 1, 1, 2))
        (self.ui / "README.md").write_text("UI documentation\n")
        self.commit(self.ui, "Update UI documentation")
        self.assertEqual(verify(self.ui, self.com, self.ledger), (3, 2, 1, 2))

    def test_missing_initial_history_is_rejected(self):
        self.ledger.write_text(f"ui_commit\tcom_commit\n{self.linked}\t{self.com_revision}\n")
        with self.assertRaisesRegex(ValueError, "complete initial UI ancestry"):
            verify(self.ui, self.com, self.ledger)

    def test_dependency_package_must_exist_at_the_selected_revision(self):
        (self.ui / "Cargo.toml").write_text(self.manifest.replace("neocogi-com=", "neocogi-idl="))
        self.commit(self.ui, "Request unavailable compiler")
        with self.assertRaisesRegex(ValueError, "package absent"):
            verify(self.ui, self.com, self.ledger)

    def test_later_trailer_cannot_override_the_actual_dependency_pin(self):
        (self.com / "README.md").write_text("COM update\n")
        other = self.commit(self.com, "Update COM documentation")
        (self.ui / "README.md").write_text("UI update\n")
        self.commit(self.ui, self.message(other))
        with self.assertRaisesRegex(ValueError, "root dependency pin"):
            verify(self.ui, self.com, self.ledger)

    def test_later_intentional_pin_rollback_is_authoritative(self):
        (self.com / "README.md").write_text("COM update\n")
        other = self.commit(self.com, "Update COM documentation")
        (self.ui / "Cargo.toml").write_text(self.manifest.replace(self.com_revision, other))
        self.commit(self.ui, "Advance the SDK")
        (self.ui / "Cargo.toml").write_text(self.manifest)
        self.commit(self.ui, "Restore the prior SDK pin")
        self.assertEqual(verify(self.ui, self.com, self.ledger), (4, 3, 2, 3))


if __name__ == "__main__":
    unittest.main()
