"""Qualify the browser runner's owned HTTP startup, async completion and cleanup.

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
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
CANARY = r'''
import json, os, signal, sys, threading, time, urllib.request
from pathlib import Path
name = Path(sys.argv[0]).name
if name == 'cargo':
    print(json.dumps({'reason': 'compiler-artifact', 'executable': 'fixture.wasm'}))
elif name == 'wasm-bindgen':
    Path(sys.argv[sys.argv.index('--out-dir') + 1]).mkdir(parents=True, exist_ok=True)
elif name == 'probe-browser':
    Path(os.environ['BROWSER_CALL']).write_text(sys.argv[-1])
    base = sys.argv[-1]
    page = urllib.request.urlopen(base, timeout=5).read().decode()
    assert 'src="/__neocogi_wait"' in page, 'missing async load barrier'
    mode = os.environ.get('BROWSER_COMPLETION', 'delayed')
    def complete():
        request = urllib.request.Request(base + '/__neocogi_done', method='POST')
        assert urllib.request.urlopen(request, timeout=5).status == 204
    if mode == 'early':
        complete()
        assert urllib.request.urlopen(base + '/__neocogi_wait', timeout=5).status == 204
    elif mode == 'timeout':
        assert urllib.request.urlopen(base + '/__neocogi_wait', timeout=35).status == 204
    else:
        results = []
        def wait_for_completion():
            results.append(urllib.request.urlopen(base + '/__neocogi_wait', timeout=5).status)
        thread = threading.Thread(target=wait_for_completion)
        thread.start()
        time.sleep(0.1)
        assert not results, 'load barrier opened before fixture completion'
        complete()
        thread.join(timeout=5)
        assert results == [204], 'completion did not release the waiting browser'
    print(page)
elif name == 'python3':
    if len(sys.argv) == 5 and sys.argv[1] == '-':
        Path(os.environ['SERVER_CALL']).write_text(json.dumps({
            'pid': os.getpid(), 'ready': sys.argv[4], 'site': sys.argv[2]}))
        mode = os.environ.get('BROWSER_SERVER_FAULT')
        if mode == 'exit':
            raise SystemExit('injected startup failure')
        if mode == 'stall':
            print('injected startup stall', file=sys.stderr, flush=True)
            signal.pause()
        if mode == 'publication':
            # Bind remains real; publishing to a directory fails after bind.
            Path(sys.argv[4]).mkdir()
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
        for suite, file in [('input', 'tests/platform/browser-input.html'),
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
        self.environment.pop('BROWSER_SERVER_FAULT', None)
        self.environment.pop('BROWSER_COMPLETION', None)

    def run_suite(self, suite: str, port: int) -> subprocess.CompletedProcess[str]:
        """Run the production script from outside its checkout, with a hard limit."""
        self.environment['NEOCOGI_BROWSER_TEST_PORT'] = str(port)
        return subprocess.run(
            ['bash', str(self.workspace / 'scripts/test-browser-input.sh'), suite],
            cwd=self.base, env=self.environment, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=40,
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

    def test_early_exit_and_failed_publication_report_diagnostics(self) -> None:
        for mode, diagnostic in [('exit', 'injected startup failure'),
                                 ('publication', 'IsADirectoryError')]:
            with self.subTest(mode=mode):
                self.environment['BROWSER_SERVER_FAULT'] = mode
                result = self.run_suite('input', self.free_port())
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn('did not become ready', result.stdout)
                self.assertIn(diagnostic, result.stdout)
                self.assertFalse(self.browser_call.exists())
                self.assert_cleaned()

    def test_timeout_ignores_stale_site_signal_and_reaps_stalled_child(self) -> None:
        # A shared site artifact cannot substitute for a new private publication.
        site = self.workspace / 'target with spaces/browser-input-tests'
        site.mkdir(parents=True)
        (site / 'server-ready').write_text('ready\n')
        self.environment['BROWSER_SERVER_FAULT'] = 'stall'
        start = time.monotonic()
        result = self.run_suite('input', self.free_port())
        elapsed = time.monotonic() - start
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertGreaterEqual(elapsed, 9)
        self.assertLess(elapsed, 18)
        self.assertIn('did not become ready', result.stdout)
        self.assertIn('injected startup stall', result.stdout)
        self.assertFalse(self.browser_call.exists())
        self.assert_cleaned()

    def test_termination_during_startup_reaps_child_and_signal_directory(self) -> None:
        self.environment['BROWSER_SERVER_FAULT'] = 'stall'
        self.environment['NEOCOGI_BROWSER_TEST_PORT'] = str(self.free_port())
        process = subprocess.Popen(
            ['bash', str(self.workspace / 'scripts/test-browser-input.sh'), 'input'],
            cwd=self.base, env=self.environment, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        try:
            deadline = time.monotonic() + 5
            while not self.server_call.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(self.server_call.exists(), 'server child did not launch')
            process.send_signal(signal.SIGTERM)
            output, _ = process.communicate(timeout=5)
            self.assertEqual(process.returncode, 143, output)
            self.assertFalse(self.browser_call.exists())
            self.assert_cleaned()
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=5)

    def test_failed_browser_result_still_cleans_up_real_server(self) -> None:
        fixture = self.workspace / 'tests/platform/browser-input.html'
        fixture.write_text('<pre id="result">FAIL</pre>')
        result = self.run_suite('input', self.free_port())
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn('<pre id="result">FAIL</pre>', result.stdout)
        self.assertTrue(self.browser_call.exists())
        self.assert_cleaned()

    def test_completion_before_image_request_opens_load_barrier(self) -> None:
        self.environment['BROWSER_COMPLETION'] = 'early'
        result = self.run_suite('full', self.free_port())
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assert_cleaned()

    def test_unfinished_fixture_releases_load_after_bounded_timeout_and_fails(self) -> None:
        self.environment['BROWSER_COMPLETION'] = 'timeout'
        fixture = self.workspace / 'examples/full/tests/browser-full.html'
        fixture.write_text('<pre id="result">RUNNING</pre>')
        start = time.monotonic()
        result = self.run_suite('full', self.free_port())
        elapsed = time.monotonic() - start
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertGreaterEqual(elapsed, 29)
        self.assertLess(elapsed, 38)
        self.assertIn('<pre id="result">RUNNING</pre>', result.stdout)
        self.assert_cleaned()


if __name__ == '__main__':
    unittest.main()
