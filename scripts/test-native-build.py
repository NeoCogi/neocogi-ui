#!/usr/bin/env python3
"""Check actual Cargo target profiles without compiling or inventing a build graph.

The runtime previously appeared as a build-tool artifact and silently received
unoptimized/unwinding settings. Inspect Cargo's own unit graph for the complete
Vulkan application; real loader/ABI execution lives in examples/plugins tests.
"""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
PROVIDERS = {"nc_ui", "nc_ui_nodes", "nc_render", "nc_gfx", "nc_skin"}


class NativeBuildProfiles(unittest.TestCase):
    """Deployed DLLs must receive their application's target compilation profile."""

    def check_profile(self, profile, panic):
        """Compare every deployed artifact and rebuilt std with the target root."""
        result = subprocess.run(
            ["cargo", "build", "-p", "neocogi-demo-full", "--profile", profile,
             "--no-default-features", "--features", "dynamic,vulkan",
             "-Zunstable-options", "--unit-graph"],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        units = json.loads(result.stdout)["units"]
        application = next(u for u in units if u["target"]["name"] == "neocogi-demo-full")
        self.assertEqual(application["target"]["kind"], ["bin"])
        expected = application["profile"]
        self.assertEqual(expected["opt_level"], "z")
        self.assertEqual(expected["panic"], panic)
        self.assertEqual(expected["lto"], "fat")
        self.assertEqual(expected["codegen_units"], 1)
        self.assertEqual(expected["strip"], {"resolved": {"Named": "symbols"}})
        self.assertEqual(expected["trim_paths"], "all")
        artifacts = [u for u in units if u["target"]["name"] in PROVIDERS]
        self.assertEqual({u["target"]["name"] for u in artifacts}, PROVIDERS)
        self.assertEqual(len(artifacts), len(PROVIDERS), "one provider artifact per target graph")
        for unit in artifacts:
            with self.subTest(provider=unit["target"]["name"]):
                self.assertEqual(unit["target"]["kind"], ["cdylib"], "implementation rlibs must not enter the host")
                for key in ("opt_level", "panic", "lto", "codegen_units", "strip", "trim_paths"):
                    self.assertEqual(unit["profile"][key], expected[key], key)
                self.assertEqual(unit["profile"].get("rustflags", []), expected.get("rustflags", []))
        standard = [u for u in units if u["target"]["name"] == "std"]
        self.assertEqual(len(standard), 1, "target std must be rebuilt with the selected profile")
        for key in ("opt_level", "panic"):
            self.assertEqual(standard[0]["profile"][key], expected[key], key)
        self.assertEqual(standard[0]["profile"].get("rustflags", []), expected.get("rustflags", []))
        self.assertIn("optimize_for_size", standard[0]["features"])
        if profile == "min-size":
            self.assertIn("-Cforce-unwind-tables=no", expected["rustflags"])
            self.assertIn("-Zlocation-detail=none", expected["rustflags"])
            self.assertIn("-Zfmt-debug=none", expected["rustflags"])
            # Naga's emitted shader syntax and stage-derived symbol names are program data.
            # Stripping Debug in either package produces invalid or colliding GLSL.
            compilers = [u for u in units if u["target"]["name"] in {"naga", "naga_types"}]
            self.assertEqual({u["target"]["name"] for u in compilers}, {"naga", "naga_types"})
            compiler_packages = {unit["pkg_id"] for unit in compilers}
            for unit in compilers:
                with self.subTest(shader_compiler=unit["target"]["name"]):
                    flags = unit["profile"].get("rustflags", [])
                    self.assertIn("-Zfmt-debug=full", flags)
                    self.assertNotIn("-Zfmt-debug=none", flags)
            for unit in units:
                if unit["target"]["kind"] in (["proc-macro"], ["custom-build"]) and unit["mode"] == "build":
                    self.assertEqual(unit["profile"]["panic"], "unwind", unit["target"]["name"])
                    flags = ["-Zfmt-debug=full"] if unit["pkg_id"] in compiler_packages else []
                    self.assertEqual(unit["profile"].get("rustflags", []), flags, unit["target"]["name"])

    def test_release_artifacts_use_the_target_profile(self):
        """Artifact-only providers share release settings, never build overrides."""
        self.check_profile("release", "abort")

    def test_plain_min_size_selects_the_complete_target_preset(self):
        """Plain --profile min-size includes std, panic and diagnostic policies."""
        self.check_profile("min-size", "immediate-abort")


if __name__ == "__main__":
    unittest.main()
