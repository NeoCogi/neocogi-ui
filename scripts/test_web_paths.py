"""Run unchanged browser scripts against subprocess canaries; no Cargo/browser build.

The matrix establishes path/argument selection and install policy. Real wasm and
Chromium qualification remains the responsibility of test-browser-input.sh.
"""
from __future__ import annotations
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
STUB = r'''
import json, os, signal, sys
from pathlib import Path
name = Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ['WEB_PATH_TRACE'], 'a') as output:
    output.write(json.dumps({'command': name, 'executable': str(Path(sys.argv[0]).resolve()), 'args': args}) + '\n')
root = Path.cwd()
target = Path(os.environ.get('CARGO_TARGET_DIR', str(root / 'target')))
if not target.is_absolute(): target = root / target
if name == 'rustup':
    print('wasm32-unknown-unknown')
elif name == 'cargo':
    if args[0] == 'install':
        tool = Path(args[args.index('--root') + 1]) / 'bin/wasm-bindgen'
        tool.parent.mkdir(parents=True, exist_ok=True)
        tool.write_bytes(Path(__file__).read_bytes())
        tool.chmod(0o755)
        tool.with_suffix('.version').write_text('0.2.127')
    elif args[0] == 'build':
        artifact = target / 'wasm32-unknown-unknown' / os.environ.get('NEOCOGI_WEB_PROFILE', 'release') / 'neocogi-demo-full.wasm'
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(b'fixture')
    elif args[0] == 'test':
        artifact = target / 'fixture.wasm'
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(b'fixture')
        print(json.dumps({'reason': 'compiler-artifact', 'executable': str(artifact)}))
    else: raise SystemExit('unexpected Cargo operation')
elif name == 'wasm-bindgen':
    if args == ['--version']:
        print('wasm-bindgen ' + Path(__file__).with_suffix('.version').read_text())
    else:
        destination = Path(args[args.index('--out-dir') + 1])
        destination.mkdir(parents=True, exist_ok=True)
        (destination / 'fixture.js').write_text('')
elif name == 'python3':
    if args[:2] == ['-m', 'http.server']: signal.pause()
    else: os.execv(sys.executable, [sys.executable, *args])
elif name == 'curl':
    print('ready')
elif name == 'probe-browser':
    print('<pre id="result">PASS</pre>')
else: raise SystemExit('unexpected canary executable')
'''


@unittest.skipIf(os.name == 'nt', 'POSIX shell script subprocess matrix')
class BrowserPathTests(unittest.TestCase):
    """Producer/consumer agreement matters even when both commands return success."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix='neocogi-web-paths-')
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.workspace = self.base / 'workspace with spaces'
        (self.workspace / 'scripts').mkdir(parents=True)
        for script in ['web-demo.sh', 'test-browser-input.sh']:
            shutil.copyfile(ROOT / 'scripts' / script, self.workspace / 'scripts' / script)
        for file in ['examples/full/web/index.html', 'examples/full/tests/browser-full.html', 'crates/render/plugin/tests/browser-input.html']:
            path = self.workspace / file
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('<html></html>')
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        self.trace = self.base / 'calls.jsonl'
        self.environment = os.environ.copy()
        for key in ['CARGO_TARGET_DIR', 'NEOCOGI_WASM_BINDGEN', 'NEOCOGI_WEB_PROFILE']:
            self.environment.pop(key, None)
        self.environment.update(PATH=str(self.bin) + os.pathsep + self.environment['PATH'], WEB_PATH_TRACE=str(self.trace), CHROME_BIN=str(self.bin / 'probe-browser'))
        self.stub = f'#!{sys.executable}\n' + STUB
        for name in ['rustup', 'cargo', 'python3', 'curl', 'probe-browser']:
            executable = self.bin / name
            executable.write_text(self.stub)
            executable.chmod(0o755)

    def tool(self, path: Path, version: str = '0.2.127') -> Path:
        """Each executable reports its own version so stale/default choices are observable."""
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(self.stub)
        path.chmod(0o755)
        path.with_suffix('.version').write_text(version)
        return path

    def run_script(self, name: str, argument: str) -> subprocess.CompletedProcess[str]:
        """Execute from outside the checkout, retaining normal quoting and shell traps."""
        return subprocess.run(['bash', str(self.workspace / 'scripts' / name), argument], cwd=self.base, env=self.environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=15)

    def calls(self) -> list[dict]:
        return [json.loads(line) for line in self.trace.read_text().splitlines()] if self.trace.exists() else []

    def assert_consumers(self, selected: Path) -> None:
        """Both suites must invoke the selected tool and use the expected Cargo package."""
        for suite in ['input', 'full']:
            result = self.run_script('test-browser-input.sh', suite)
            self.assertEqual(result.returncode, 0, result.stdout)
        bindgen = [call for call in self.calls() if call['command'] == 'wasm-bindgen']
        self.assertTrue(bindgen)
        self.assertTrue(all(Path(call['executable']) == selected.resolve() for call in bindgen))
        tests = [call['args'] for call in self.calls() if call['command'] == 'cargo' and call['args'][0] == 'test']
        self.assertEqual([args[args.index('-p') + 1] for args in tests[-2:]], ['neocogi-render-plugin', 'neocogi-demo-full'])

    def test_default_absolute_relative_and_space_roots_agree_without_install(self) -> None:
        for value in [None, 'relative cache', str(self.base / 'absolute cache')]:
            with self.subTest(target=value):
                self.trace.unlink(missing_ok=True)
                if value is None:
                    self.environment.pop('CARGO_TARGET_DIR', None)
                    target = self.workspace / 'target'
                else:
                    self.environment['CARGO_TARGET_DIR'] = value
                    target = Path(value) if Path(value).is_absolute() else self.workspace / value
                    self.tool(self.workspace / 'target/web-tools/bin/wasm-bindgen', '0.2.126')
                selected = self.tool(target / 'web-tools/bin/wasm-bindgen')
                result = self.run_script('web-demo.sh', 'build')
                self.assertEqual(result.returncode, 0, result.stdout)
                self.assert_consumers(selected)
                self.assertFalse(any(call['command'] == 'cargo' and call['args'][0] == 'install' for call in self.calls()))

    def test_absolute_and_relative_explicit_overrides_win(self) -> None:
        for relative in [False, True]:
            with self.subTest(relative=relative):
                self.trace.unlink(missing_ok=True)
                selected = self.tool(self.workspace / 'explicit tool/bin/wasm-bindgen')
                self.environment['NEOCOGI_WASM_BINDGEN'] = str(selected.relative_to(self.workspace) if relative else selected)
                self.environment['CARGO_TARGET_DIR'] = 'other cache'
                self.tool(self.workspace / 'target/web-tools/bin/wasm-bindgen', '0.2.126')
                result = self.run_script('web-demo.sh', 'build')
                self.assertEqual(result.returncode, 0, result.stdout)
                self.assert_consumers(selected)
                self.assertFalse(any(call['command'] == 'cargo' and call['args'][0] == 'install' for call in self.calls()))

    def test_invalid_explicit_override_never_installs_a_different_tool(self) -> None:
        selected = self.workspace / 'explicit tool/bin/wasm-bindgen'
        for version in [None, '0.2.126']:
            with self.subTest(version=version):
                self.trace.unlink(missing_ok=True)
                if version is not None: self.tool(selected, version)
                self.environment['NEOCOGI_WASM_BINDGEN'] = str(selected)
                result = self.run_script('web-demo.sh', 'build')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(str(selected), result.stdout)
                self.assertIn('0.2.127', result.stdout)
                self.assertFalse(any(call['command'] == 'cargo' for call in self.calls()))

    def test_missing_selected_tool_does_not_use_an_existing_default(self) -> None:
        self.environment['CARGO_TARGET_DIR'] = 'missing cache'
        self.tool(self.workspace / 'target/web-tools/bin/wasm-bindgen')
        result = self.run_script('test-browser-input.sh', 'input')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(self.workspace / 'missing cache/web-tools/bin/wasm-bindgen'), result.stdout)
        self.assertFalse(any(call['command'] == 'cargo' for call in self.calls()))

    def test_default_install_uses_the_selected_root_and_is_reused(self) -> None:
        self.environment['CARGO_TARGET_DIR'] = 'fresh cache'
        target = self.workspace / 'fresh cache'
        result = self.run_script('web-demo.sh', 'build')
        self.assertEqual(result.returncode, 0, result.stdout)
        selected = target / 'web-tools/bin/wasm-bindgen'
        self.assertTrue(selected.is_file())
        self.assert_consumers(selected)
        result = self.run_script('web-demo.sh', 'build')
        self.assertEqual(result.returncode, 0, result.stdout)
        installs = [call for call in self.calls() if call['command'] == 'cargo' and call['args'][0] == 'install']
        self.assertEqual(len(installs), 1)
        args = installs[0]['args']
        self.assertEqual(args[args.index('--root') + 1], str(target / 'web-tools'))


if __name__ == '__main__':
    unittest.main()
