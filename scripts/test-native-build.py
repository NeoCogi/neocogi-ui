#!/usr/bin/env python3
"""Check actual Cargo target profiles without compiling or inventing a build graph.

The runtime previously appeared as a build-tool artifact and silently received
unoptimized/unwinding settings. Inspect Cargo's own unit graph for the complete
Vulkan application; real loader/ABI execution lives in demos/plugins tests.
"""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
PROVIDERS = {"nc_com", "nc_ui", "nc_ui_nodes", "nc_render", "nc_gfx", "nc_skin"}


class NativeBuildProfiles(unittest.TestCase):
    """Deployed DLLs must receive their application's target compilation profile."""

    def test_release_artifacts_use_the_target_profile(self):
        """Artifact-only providers share release settings, never build overrides."""
        result = subprocess.run(
            ["cargo", "build", "-p", "neocogi-demo-full", "--release", "--offline",
             "--no-default-features", "--features", "dynamic,vulkan",
             "-Zunstable-options", "--unit-graph"],
            cwd=ROOT, capture_output=True, text=True, check=True,
        )
        units = json.loads(result.stdout)["units"]
        application = next(u for u in units if u["target"]["name"] == "neocogi-demo-full")
        self.assertEqual(application["target"]["kind"], ["bin"])
        expected = application["profile"]
        self.assertEqual(expected["opt_level"], "z")
        self.assertEqual(expected["panic"], "abort")
        artifacts = [u for u in units if u["target"]["name"] in PROVIDERS]
        self.assertEqual({u["target"]["name"] for u in artifacts}, PROVIDERS)
        self.assertEqual(len(artifacts), len(PROVIDERS), "one runtime/provider artifact per target graph")
        for unit in artifacts:
            with self.subTest(provider=unit["target"]["name"]):
                self.assertEqual(unit["target"]["kind"], ["cdylib"], "implementation rlibs must not enter the host")
                for key in ("opt_level", "panic", "lto", "codegen_units", "strip"):
                    self.assertEqual(unit["profile"][key], expected[key], key)


if __name__ == "__main__":
    unittest.main()
