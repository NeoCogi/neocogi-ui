"""Qualify the unchanged browser runner's owned HTTP startup and cleanup.

Cargo and wasm-bindgen are canaries; the server is real Python HTTP and the
browser canary fetches its actual response. These tests require loopback access,
but neither compile WebAssembly nor replace the real Chromium suites.
"""
from __future__ import annotations

import functools
import http.server
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
CANARY = r'''
import json, os, sys, urllib.request
from pathlib import Path
name = Path(sys.argv[0]).name
if name == 'cargo':
    print(json.dumps({'reason': 'compiler-artifact', 'executable': 'fixture.wasm'}))
elif name == 'wasm-bindgen':
    Path(sys.argv[sys.argv.index('--out-dir') + 1]).mkdir(parents=True, exist_ok=True)
elif name == 'probe-browser':
    Path(os.environ['BROWSER_CALL']).write_text(sys.argv[-1])
    print(urllib.request.urlopen(sys.argv[-1], timeout=5).read().decode())
elif name == 'python3':
    if len(sys.argv) == 5 and sys.argv[1] == '-':
        Path(os.environ['SERVER_CALL']).write_text(json.dumps({
            'pid': os.getpid(), 'ready': sys.argv[4], 'site': sys.argv[2]}))
    os.execv(sys.executable, [sys.executable, *sys.argv[1:]])
else:
    raise SystemExit('unexpected canary executable')
'''


@unittest.skipIf(os.name == 'nt', 'POSIX browser runner requires Bash')
class BrowserServerTests(unittest.TestCase):
    """Readiness must belong to the child, not any responder at the same port."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix='neocogi-browser-server-')
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.workspace = self.base / 'workspace with spaces'
        (self.workspace / 'scripts').mkdir(parents=True)
        shutil.copyfile(ROOT / 'scripts/test-browser-input.sh', self.workspace / 'scripts/test-browser-input.sh')
        for suite, file in [('input', 'crates/render/plugin/tests/browser-input.html'),
                            ('full', 'examples/full/tests/browser-full.html')]:
            fixture = self.workspace / file
            fixture.parent.mkdir(parents=True, exist_ok=True)
            fixture.write_text(f'<p>SELECTED {suite}</p><pre id="result">PASS</pre>')
        self.bin = self.base / 'canary bin'
        self.bin.mkdir()
        for name in ['cargo', 'wasm-bindgen', 'probe-browser', 'python3']:
            executable = self.bin / name
            executable.write_text(f'#!{sys.executable}\n' + CANARY)
            executable.chmod(0o755)
        self.browser_call = self.base / 'browser-call'
        self.server_call = self.base / 'server-call.json'
        self.environment = os.environ.copy()
        self.environment.update(
            PATH=str(self.bin) + os.pathsep + self.environment['PATH'],
            CARGO_TARGET_DIR='target with spaces',
            NEOCOGI_WASM_BINDGEN=str(self.bin / 'wasm-bindgen'),
            CHROME_BIN=str(self.bin / 'probe-browser'),
            BROWSER_CALL=str(self.browser_call), SERVER_CALL=str(self.server_call),
        )

    def run_suite(self, suite: str, port: int) -> subprocess.CompletedProcess[str]:
        """Run the production script from outside its checkout, with a hard limit."""
        self.environment['NEOCOGI_BROWSER_TEST_PORT'] = str(port)
        return subprocess.run(
            ['bash', str(self.workspace / 'scripts/test-browser-input.sh'), suite],
            cwd=self.base, env=self.environment, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=20,
        )

    def assert_cleaned(self) -> None:
        """The runner must reap its own child and delete its private signal directory."""
        call = json.loads(self.server_call.read_text())
        self.assertFalse(Path(call['ready']).parent.exists())
        with self.assertRaises(ProcessLookupError):
            os.kill(call['pid'], 0)

    @staticmethod
    def free_port() -> int:
        """Reserve a candidate briefly; the actual runner must still own its bind."""
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', 0))
            return probe.getsockname()[1]

    def test_selected_input_and_full_sites_are_served_and_children_reaped(self) -> None:
        signals = []
        for suite in ['input', 'full']:
            with self.subTest(suite=suite):
                result = self.run_suite(suite, self.free_port())
                self.assertEqual(result.returncode, 0, result.stdout)
                site = self.workspace / f'target with spaces/browser-{suite}-tests'
                self.assertIn(f'SELECTED {suite}', (site / 'result.html').read_text())
                self.assertTrue(self.browser_call.exists())
                signals.append(json.loads(self.server_call.read_text())['ready'])
                self.assert_cleaned()
        self.assertEqual(len(set(signals)), 2, 'each invocation needs a private signal')

    def test_existing_pass_server_never_launches_browser_or_qualifies(self) -> None:
        old_site = self.base / 'unrelated site'
        old_site.mkdir()
        old_html = '<p>UNRELATED SITE</p><pre id="result">PASS</pre>'
        (old_site / 'index.html').write_text(old_html)
        handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(old_site))
        with http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                for suite in ['input', 'full']:
                    with self.subTest(suite=suite):
                        result = self.run_suite(suite, server.server_port)
                        self.assertNotEqual(result.returncode, 0, result.stdout)
                        self.assertIn('did not become ready', result.stdout)
                        self.assertIn('Address already in use', result.stdout)
                        self.assertFalse(self.browser_call.exists())
                        self.assert_cleaned()
                # Failed startup must leave the unrelated listener alive.
                with urllib.request.urlopen(f'http://127.0.0.1:{server.server_port}', timeout=5) as response:
                    self.assertEqual(response.read().decode(), old_html)
            finally:
                server.shutdown()
                thread.join(timeout=5)


if __name__ == '__main__':
    unittest.main()
