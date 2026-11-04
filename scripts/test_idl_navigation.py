"""Check navigation boundaries alongside the actual server smoke in CI."""

import io
import json
import os
from pathlib import Path
import queue
import tempfile
import unittest
from unittest.mock import patch

from test_contract_checks import load_script


navigation = load_script("check-idl-navigation")
REPOSITORY = Path(__file__).resolve().parents[1]


class NavigationProbe(unittest.TestCase):
    """One concrete source supplies every compile input and editor cursor."""

    def test_all_markers_address_their_actual_identifier(self):
        """Repeated enum uses and its field cannot silently select another token."""
        for probe in navigation.PROBES:
            cursor = navigation.probe_position(navigation.PROBE_SOURCE, probe)
            line, column = navigation.scalar_position(navigation.PROBE_SOURCE, cursor)
            actual = navigation.PROBE_SOURCE.splitlines()[line - 1][column - 1:]
            with self.subTest(marker=probe.marker):
                self.assertTrue(actual.startswith(probe.symbol))
        with self.assertRaisesRegex(ValueError, "one actual marked use"):
            navigation.probe_position(navigation.PROBE_SOURCE * 2, navigation.PROBES[0])

    def test_utf16_and_scalar_positions_are_distinct(self):
        """An astral character spends two UTF-16 units but one source scalar."""
        probe = navigation.PROBES[0]
        source = "/* 😀é */ /*abi*/TextboxAbi"
        cursor = navigation.probe_position(source, probe)
        expected = source.index(probe.symbol) + 1
        self.assertEqual(navigation.scalar_position(source, cursor), (1, expected))
        with self.assertRaises(UnicodeDecodeError):
            navigation.scalar_position("😀", {"line": 0, "character": 1})
        with self.assertRaises(ValueError):
            navigation.scalar_position("x", {"line": 0, "character": 3})

    def test_consumer_has_only_the_ui_dependency(self):
        """A compensating direct COM dependency cannot hide COM24 in this gate."""
        import tomllib
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source = navigation.prepare_probe(directory, REPOSITORY)
            data = tomllib.loads((directory / "Cargo.toml").read_text())
            self.assertEqual(set(data["dependencies"]), {"neocogi-ui"})
            self.assertEqual(source.read_text(), navigation.PROBE_SOURCE)

    def test_compile_command_preserves_caller_cargo_network_policy(self):
        """The navigation entrypoint allows a cold cache and honors explicit offline."""
        for offline in (None, "true", "false"):
            caller = {} if offline is None else {"CARGO_NET_OFFLINE": offline}
            with self.subTest(offline=offline), patch.dict(os.environ, caller, clear=True), patch(
                "sys.argv", ["check-idl-navigation.py", "--compile-only"]
            ), patch("contract_check_support.subprocess.check_output", return_value="workspace-nightly\n"), patch.object(
                navigation.subprocess, "run"
            ) as cargo:
                navigation.main()
                cargo.assert_called_once()
                self.assertNotIn("--offline", cargo.call_args.args[0])
                self.assertEqual(cargo.call_args.kwargs["env"].get("CARGO_NET_OFFLINE"), offline)
                self.assertEqual(os.environ, caller)

    def test_expected_member_is_scoped_to_its_owner(self):
        """LayoutTrack's extent cannot be confused with the other native enums."""
        field = next(probe for probe in navigation.PROBES if probe.marker == "field")
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            file = repository / "crates/build/schemas/neocogi/ui/nodes/layout.ncidl"
            file.parent.mkdir(parents=True)
            file.write_text("enum Earlier: u8 {\n    Fixed {\n        extent: i32,\n    },\n}\n"
                            "enum LayoutTrack: u8 {\n    Fixed {\n        extent: i32,\n    },\n}\n")
            path, span = navigation.expected_origin(repository, field)
            self.assertEqual(path, file.resolve())
            self.assertEqual(span, {"start": {"line": 8, "column": 9}, "end": {"line": 8, "column": 15}})


class DefinitionResults(unittest.TestCase):
    """Response shape and map specificity preserve exact authored identity."""

    def test_location_and_link_select_the_identifier(self):
        """A LocationLink's broad targetRange must not replace targetSelectionRange."""
        identifier = {"start": {"line": 8, "character": 4}, "end": {"line": 8, "character": 10}}
        location = {"uri": "file:///generated.rs", "range": identifier}
        link = {"targetUri": location["uri"], "targetSelectionRange": identifier,
                "targetRange": {"start": {"line": 0, "character": 0}, "end": {"line": 40, "character": 0}}}
        for result in (location, [location], [link]):
            self.assertEqual(navigation.definition_location(result), (location["uri"], identifier))
        for result in (None, [], [location, location]):
            with self.assertRaises(ValueError):
                navigation.definition_location(result)

    def test_smallest_range_wins_and_end_is_excluded(self):
        """The final map's field takes precedence over an enclosing enum fragment."""
        broad = {"generated": {"start": {"line": 1, "column": 1}, "end": {"line": 4, "column": 1}}, "name": "enum"}
        field = {"generated": {"start": {"line": 2, "column": 10}, "end": {"line": 2, "column": 16}}, "name": "field"}
        for entries in ([broad, field], [field, broad]):
            self.assertEqual(navigation.lookup({"mappings": entries}, (2, 11))["name"], "field")
            self.assertEqual(navigation.lookup({"mappings": entries}, (2, 16))["name"], "enum")
        later = dict(field, name="later exact range")
        self.assertEqual(navigation.lookup({"mappings": [field, later]}, (2, 11))["name"], later["name"])
        with self.assertRaises(ValueError):
            navigation.lookup({"mappings": [field]}, (2, 16))

    def test_wrong_existing_file_and_wrong_member_are_rejected(self):
        """Existing IDL alone is insufficient; both the owner and member must match."""
        probe = next(probe for probe in navigation.PROBES if probe.marker == "field")
        expected_file, expected_range = navigation.expected_origin(REPOSITORY, probe)
        with tempfile.TemporaryDirectory() as temporary:
            generated = Path(temporary) / "module_0123456789abcdef0123456789abcdef.rs"
            generated.write_text("pub enum Track {\n Fixed { extent: i32 },\n}\n")
            canonical = expected_file
            expected_file = generated.parent / "neocogi-ui-sources/neocogi/ui/nodes" / probe.file
            expected_file.parent.mkdir(parents=True)
            expected_file.write_bytes(canonical.read_bytes())
            mapping = {
                "schema": 2, "module": "neocogi.ui.nodes", "generated_file": generated.name,
                "sources": [{"module": "neocogi.ui.nodes", "location": str(expected_file)}],
                "mappings": [{"generated": {"start": {"line": 2, "column": 10}, "end": {"line": 2, "column": 16}},
                              "original": {"source": 0, "range": expected_range}}],
            }
            result = {"uri": generated.as_uri(), "range": {"start": {"line": 1, "character": 9}}}
            sidecar = generated.with_suffix(".rs.map")
            sidecar.write_text(json.dumps(mapping))
            navigation.validate_definition(REPOSITORY, probe, result)
            expected_file.write_bytes(b"stale declaration")
            with self.assertRaisesRegex(ValueError, "wrong authored origin"):
                navigation.validate_definition(REPOSITORY, probe, result)
            expected_file.write_bytes(canonical.read_bytes())
            mapping["sources"][0]["location"] = str(canonical)
            sidecar.write_text(json.dumps(mapping))
            with self.assertRaisesRegex(ValueError, "wrong authored origin"):
                navigation.validate_definition(REPOSITORY, probe, result)
            mapping["sources"][0]["location"] = str(expected_file)
            mapping["module"] = "neocogi.ui.core"
            sidecar.write_text(json.dumps(mapping))
            with self.assertRaisesRegex(ValueError, "unexpected generated map identity"):
                navigation.validate_definition(REPOSITORY, probe, result)
            mapping["module"] = "neocogi.ui.nodes"
            mapping["sources"][0]["location"] = str(expected_file.with_name("widgets.ncidl"))
            sidecar.write_text(json.dumps(mapping))
            with self.assertRaisesRegex(ValueError, "wrong authored origin"):
                navigation.validate_definition(REPOSITORY, probe, result)
            mapping["sources"][0]["location"] = str(expected_file)
            variant = next(probe for probe in navigation.PROBES if probe.marker == "variant")
            mapping["mappings"][0]["original"]["range"] = navigation.expected_origin(REPOSITORY, variant)[1]
            sidecar.write_text(json.dumps(mapping))
            with self.assertRaisesRegex(ValueError, "wrong authored origin"):
                navigation.validate_definition(REPOSITORY, probe, result)

    def test_server_eof_wakes_the_response_waiter(self):
        """An exited server reports its real failure instead of waiting 180 seconds."""
        messages = queue.Queue()
        navigation.read_messages(io.BytesIO(b""), messages)
        with self.assertRaisesRegex(RuntimeError, "closed stdout"):
            navigation.receive_until(messages, lambda message: None, lambda message: True, timeout=.1)


if __name__ == "__main__":
    unittest.main()
