#!/usr/bin/env python3
"""Verify real rust-analyzer definition targets and their adjacent IDL source maps.

Requires an installed rust-analyzer and cached Cargo dependencies. No editor
settings change: an isolated client loads a temporary consumer of the actual
workspace contracts. CARGO_TARGET_DIR may select an existing validation cache.
"""

import argparse
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
from urllib.parse import unquote, urlparse


def check(server, directory, repository):
    """Load the real generated contracts and require navigable Rust/IDL origins."""
    symbols = ["TextboxAbi", "TextboxClient", "TextboxVTable", "SliderOptions",
               "LayoutTrackAbi", "LayoutTrackTag", "LayoutTrackPayload", "ChildCrossSize"]
    declaration = "pub fn probe(" + ", ".join(f"_: Option<{name}>" for name in symbols) + ") {}"
    content = "use neocogi_ui_nodes_contracts::*;\n\n" + declaration + "\n"
    source = directory / "src/lib.rs"
    source.parent.mkdir()
    source.write_text(content)
    dependency = json.dumps(str(repository / "crates/ui/nodes-contracts"))
    (directory / "Cargo.toml").write_text(
        '[package]\nname = "neocogi-navigation-probe"\nversion = "0.0.0"\nedition = "2024"\n'
        '[dependencies]\nneocogi-ui-nodes-contracts = { path = ' + dependency + ' }\n'
    )
    environment = dict(os.environ, CARGO_NET_OFFLINE="true")
    environment.setdefault("CARGO_TARGET_DIR", str(repository / "target"))
    messages = queue.Queue()
    with (directory / "server.log").open("w+") as log:
        process = subprocess.Popen([server], cwd=directory, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=log, env=environment)

        def read_messages():
            """Read framed responses without blocking service of server requests."""
            while True:
                headers = {}
                while (line := process.stdout.readline()) not in (b"\r\n", b"\n", b""):
                    key, value = line.decode().split(":", 1)
                    headers[key.lower()] = value.strip()
                if not headers:
                    return
                messages.put(json.loads(process.stdout.read(int(headers["content-length"]))))

        def send(message):
            """Write a complete LSP frame; test results use a separate stdout."""
            data = json.dumps(message).encode()
            process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
            process.stdin.flush()

        def receive_until(predicate):
            """Answer configuration requests while waiting for loading or navigation."""
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                message = messages.get(timeout=max(.01, deadline - time.monotonic()))
                if "method" in message and "id" in message:
                    result = ([None for _ in message["params"]["items"]]
                              if message["method"] == "workspace/configuration" else None)
                    send({"jsonrpc": "2.0", "id": message["id"], "result": result})
                if predicate(message):
                    return message
            raise TimeoutError("rust-analyzer did not finish the request")

        threading.Thread(target=read_messages, daemon=True).start()
        try:
            send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "processId": os.getpid(), "rootUri": directory.as_uri(),
                "capabilities": {"experimental": {"serverStatusNotification": True}},
                "initializationOptions": {"checkOnSave": False, "cargo": {"buildScripts": {"enable": True}},
                                          "procMacro": {"enable": True}},
                "workspaceFolders": [{"uri": directory.as_uri(), "name": "navigation-probe"}]}})
            receive_until(lambda message: message.get("id") == 1)
            send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
            receive_until(lambda message: message.get("method") == "experimental/serverStatus"
                          and message["params"].get("quiescent"))
            send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": {"uri": source.as_uri(), "languageId": "rust", "version": 1, "text": content}}})
            for identifier, symbol in enumerate(symbols, 2):
                send({"jsonrpc": "2.0", "id": identifier, "method": "textDocument/definition", "params": {
                    "textDocument": {"uri": source.as_uri()},
                    "position": {"line": 2, "character": declaration.index(symbol) + 2}}})
                response = receive_until(lambda message: message.get("id") == identifier)
                targets = response.get("result")
                assert targets and len(targets) == 1, (symbol, response)
                target = targets[0]
                generated = Path(unquote(urlparse(target["uri"]).path))
                assert generated.name == "neocogi_ui_nodes.rs", (symbol, target)
                # These generated identifiers are ASCII, so LSP's zero-based
                # UTF-16 columns convert directly to the map's scalar columns.
                cursor = target["range"]["start"]
                position = (cursor["line"] + 1, cursor["character"] + 1)
                mapping = json.loads(generated.with_suffix(".rs.map").read_text())
                matches = [entry for entry in mapping["mappings"] if
                           tuple(entry["generated"]["start"][key] for key in ("line", "column")) <= position <
                           tuple(entry["generated"]["end"][key] for key in ("line", "column"))]
                assert matches, (symbol, position)
                origin = matches[-1]["original"]
                assert mapping["schema"] == 2, mapping["schema"]
                authored = generated.parent / mapping["sources"][origin["source"]]["location"]
                assert authored.is_file(), authored
                print(json.dumps({"symbol": symbol, "rust": str(generated), "position": position,
                                  "idl": str(authored.resolve()), "origin": origin["range"]}), flush=True)
        except BaseException:
            log.seek(0)
            print(log.read())
            raise
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    """Select an existing language server and discard only this test's fixture."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust-analyzer", default="rust-analyzer", help="Path to an installed language server")
    options = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix="neocogi-navigation-") as directory:
        check(options.rust_analyzer, Path(directory), repository)


if __name__ == "__main__":
    main()
