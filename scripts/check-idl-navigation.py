#!/usr/bin/env python3
"""Compile one current contract probe, then verify actual Rust and IDL origins.

The ordinary mode requires an installed rust-analyzer; --compile-only exercises
that same source without an editor. Cached Cargo dependencies and the selected
workspace compiler are used in a temporary independent UI facade consumer.
Caller-selected CARGO_TARGET_DIR caches are retained; default scratch targets
and the language-server process are owned by this invocation.
"""

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time
from urllib.parse import unquote, urlparse

from contract_check_support import child_environment


# The compiled source is also the sole source of LSP request positions. Markers
# distinguish separate uses of the same identifier, including a constructor field.
# Client views are unsized projections borrowed from COM handles; the probe must
# borrow the view too rather than trying to store it directly inside Option.
PROBE_SOURCE = '''use neocogi_ui::abi::nodes::*;
pub fn probe(
    _: Option</*abi*/TextboxAbi>,
    _: Option<&/*client*/TextboxClient>,
    _: Option</*vtable*/TextboxVTable>,
    _: Option</*struct*/SliderOptions>,
    _: Option</*enum*/LayoutTrack>,
    _: Option</*cross*/ChildCrossSize>,
    _: Option</*flags*/FillStates>,
) {
    let _ = FillStates::/*flag*/HOVER;
    let _ = /*constructor*/LayoutTrack::/*variant*/Fixed { /*field*/extent: 8 };
    let _ = SliderOptions::/*type_metadata*/IDL_TYPE;
    let _ = TextboxAbi::/*interface_metadata*/IDL_INTERFACE;
}
'''


@dataclass(frozen=True)
class Probe:
    """One actual identifier use and its independently specified authored cause."""

    marker: str
    """Unique marker immediately before the identifier in PROBE_SOURCE."""
    symbol: str
    """Actual public Rust identifier to query, not a generated helper inventory."""
    file: str
    """Known nodes IDL file, resolved through its original source location."""
    declaration: str
    """Unique authored declaration anchor that owns the expected identifier."""
    anchor: str
    """Exact identifier/member anchor within that declaration's smoke fixture."""
    authored_symbol: str
    """IDL identifier; interface companions all originate from their authored trait."""


PROBES = (
    Probe("abi", "TextboxAbi", "widgets.ncidl", "interface Textbox:", "interface Textbox:", "Textbox"),
    Probe("client", "TextboxClient", "widgets.ncidl", "interface Textbox:", "interface Textbox:", "Textbox"),
    Probe("vtable", "TextboxVTable", "widgets.ncidl", "interface Textbox:", "interface Textbox:", "Textbox"),
    Probe("struct", "SliderOptions", "widgets.ncidl", "struct SliderOptions {", "struct SliderOptions {", "SliderOptions"),
    Probe("enum", "LayoutTrack", "layout.ncidl", "enum LayoutTrack:", "enum LayoutTrack:", "LayoutTrack"),
    Probe("cross", "ChildCrossSize", "layout.ncidl", "enum ChildCrossSize:", "enum ChildCrossSize:", "ChildCrossSize"),
    Probe("flags", "FillStates", "widgets.ncidl", "bitenum FillStates:", "bitenum FillStates:", "FillStates"),
    Probe("flag", "HOVER", "widgets.ncidl", "bitenum FillStates:", "    HOVER = 2,", "HOVER"),
    Probe("constructor", "LayoutTrack", "layout.ncidl", "enum LayoutTrack:", "enum LayoutTrack:", "LayoutTrack"),
    Probe("variant", "Fixed", "layout.ncidl", "enum LayoutTrack:", "    Fixed {", "Fixed"),
    Probe("field", "extent", "layout.ncidl", "enum LayoutTrack:", "        extent: i32,", "extent"),
    Probe("type_metadata", "IDL_TYPE", "widgets.ncidl", "struct SliderOptions {", "struct SliderOptions {", "SliderOptions"),
    Probe("interface_metadata", "IDL_INTERFACE", "widgets.ncidl", "interface Textbox:", "interface Textbox:", "Textbox"),
)


def probe_position(source, probe):
    """Locate exactly one marked use in zero-based LSP UTF-16 coordinates."""
    needle = f"/*{probe.marker}*/{probe.symbol}"
    if source.count(needle) != 1:
        raise ValueError(f"probe must have one actual marked use: {needle}")
    before = source[:source.index(needle) + len(f"/*{probe.marker}*/")]
    return {"line": before.count("\n"), "character": len(before.rsplit("\n", 1)[-1].encode("utf-16-le")) // 2}


def scalar_position(source, cursor):
    """Convert LSP UTF-16 to one-based Unicode scalars; reject partial surrogates."""
    line = source.splitlines()[cursor["line"]]
    units = cursor["character"]
    encoded = line.encode("utf-16-le")
    if not 0 <= 2 * units <= len(encoded):
        raise ValueError("definition cursor is outside its generated line")
    prefix = encoded[:2 * units].decode("utf-16-le")
    return cursor["line"] + 1, len(prefix) + 1


def expected_origin(repository, probe):
    """Read the expected authored token without consulting the generated map.

    These few smoke declarations have an unindented closing brace. Restrict
    anchors to that declaration so another enum's Fixed/extent cannot substitute
    for LayoutTrack's member. This is fixture expectation lookup, not an IDL parser.
    """
    path = repository / "crates/ui/schemas/neocogi/ui/nodes" / probe.file
    text = path.read_text(encoding="utf-8")
    if text.count(probe.declaration) != 1:
        raise ValueError("expected authored declaration is ambiguous: " + probe.declaration)
    start = text.index(probe.declaration)
    scope = text[start:text.index("\n}", start)]
    if scope.count(probe.anchor) != 1:
        raise ValueError("expected authored member is ambiguous: " + probe.anchor)
    offset = start + scope.index(probe.anchor) + probe.anchor.index(probe.authored_symbol)
    before = text[:offset]
    first = {"line": before.count("\n") + 1, "column": len(before.rsplit("\n", 1)[-1]) + 1}
    last = {"line": first["line"], "column": first["column"] + len(probe.authored_symbol)}
    return path.resolve(), {"start": first, "end": last}


def definition_location(result):
    """Select one Location or a LocationLink's precise targetSelectionRange."""
    if isinstance(result, list):
        if len(result) != 1:
            raise ValueError(f"expected one definition, received {result}")
        result = result[0]
    if not isinstance(result, dict):
        raise ValueError(f"missing definition: {result}")
    if "targetUri" in result:
        return result["targetUri"], result["targetSelectionRange"]
    return result["uri"], result["range"]


def lookup(mapping, position):
    """Match RustSourceMap::lookup: contained ranges win; equal ranges prefer later."""
    def point(value):
        """Order one-based scalar map coordinates consistently with Rust."""
        return value["line"], value["column"]

    selected = None
    for candidate in mapping["mappings"]:
        span = candidate["generated"]
        if not point(span["start"]) <= position < point(span["end"]):
            continue
        if selected is not None:
            previous = selected["generated"]
            if point(span["start"]) < point(previous["start"]) or point(span["end"]) > point(previous["end"]):
                continue
        selected = candidate
    if selected is None:
        raise ValueError(f"unmapped generated definition: {position}")
    return selected


def validate_definition(repository, probe, result):
    """Require the real generated file and its exact expected authored identifier."""
    uri, span = definition_location(result)
    parsed = urlparse(uri)
    if parsed.scheme != "file" or parsed.netloc not in ("", "localhost"):
        raise ValueError("definition is not a local generated file: " + uri)
    generated = Path(unquote(parsed.path)).resolve()
    if generated.suffix != ".rs":
        raise ValueError("definition did not reach generated Rust: " + str(generated))
    # BuildOutput owns filenames. The adjacent map supplies exact module/file
    # identity, so navigation does not reconstruct a logical-name-derived stem.
    position = scalar_position(generated.read_text(encoding="utf-8"), span["start"])
    mapping = json.loads(generated.with_suffix(".rs.map").read_text(encoding="utf-8"))
    if mapping["schema"] != 2 or mapping["module"] != "neocogi.ui.nodes" or mapping["generated_file"] != generated.name:
        raise ValueError("unexpected generated map identity or schema")
    origin = lookup(mapping, position)["original"]
    owner = mapping["sources"][origin["source"]]
    authored = (generated.parent / owner["location"]).resolve()
    expected_file, expected_range = expected_origin(repository, probe)
    if owner["module"] != "neocogi.ui.nodes" or authored != expected_file or origin["range"] != expected_range:
        raise ValueError(f"wrong authored origin for {probe.marker}: {authored}:{origin['range']}; expected {expected_file}:{expected_range}")
    return {"probe": probe.marker, "symbol": probe.symbol, "rust": str(generated),
            "position": position, "idl": str(authored), "origin": origin["range"]}


def prepare_probe(directory, repository):
    """Write the one exact consumer; I1's dependencies supply their own macro needs."""
    source = directory / "src/lib.rs"
    source.parent.mkdir()
    source.write_text(PROBE_SOURCE, encoding="utf-8")
    dependency = json.dumps(str(repository / "crates/ui/api"))
    (directory / "Cargo.toml").write_text(
        '[package]\nname="neocogi-navigation-probe"\nversion="0.0.0"\nedition="2024"\n'
        '[workspace]\n[dependencies]\nneocogi-ui={path=' + dependency + '}\n', encoding="utf-8",
    )
    return source


def read_messages(stream, messages):
    """Frame server stdout on one reader; EOF or invalid framing wakes the waiter."""
    try:
        while True:
            headers = {}
            while (line := stream.readline()) not in (b"\r\n", b"\n", b""):
                key, value = line.decode().split(":", 1)
                headers[key.lower()] = value.strip()
            if not headers:
                raise RuntimeError("rust-analyzer closed stdout before the expected response")
            messages.put(json.loads(stream.read(int(headers["content-length"]))))
    except Exception as error:
        messages.put(error)


def receive_until(messages, send, predicate, timeout=180):
    """Answer configuration requests while waiting; server death is not a timeout."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            message = messages.get(timeout=min(1, max(.01, deadline - time.monotonic())))
        except queue.Empty:
            continue
        if isinstance(message, Exception):
            raise message
        if "method" in message and "id" in message:
            result = ([None for _ in message["params"]["items"]]
                      if message["method"] == "workspace/configuration" else None)
            send({"jsonrpc": "2.0", "id": message["id"], "result": result})
        if predicate(message):
            if "error" in message:
                raise RuntimeError("language-server request failed: " + json.dumps(message["error"]))
            return message
    raise TimeoutError("rust-analyzer did not finish the request")


def navigate(server, directory, repository, environment, source):
    """Run the actual server against the compiled source, then release its process."""
    messages = queue.Queue()
    with (directory / "server.log").open("w+") as log:
        process = subprocess.Popen([server], cwd=directory, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=log, env=environment)
        reader = threading.Thread(target=read_messages, args=(process.stdout, messages), daemon=True)
        reader.start()

        def send(message):
            """Send one complete JSON-RPC frame; only this thread writes stdin."""
            data = json.dumps(message).encode()
            process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
            process.stdin.flush()

        def receive(predicate):
            """Use the same response/error boundary for startup and all definitions."""
            return receive_until(messages, send, predicate)

        try:
            send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "processId": os.getpid(), "rootUri": directory.as_uri(),
                "capabilities": {"experimental": {"serverStatusNotification": True},
                                 "textDocument": {"definition": {"linkSupport": True}}},
                "initializationOptions": {"checkOnSave": False, "cargo": {"buildScripts": {"enable": True}},
                                          "procMacro": {"enable": True}},
                "workspaceFolders": [{"uri": directory.as_uri(), "name": "navigation-probe"}]}})
            receive(lambda message: message.get("id") == 1)
            send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
            receive(lambda message: message.get("method") == "experimental/serverStatus" and message["params"].get("quiescent"))
            send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": {"uri": source.as_uri(), "languageId": "rust", "version": 1, "text": PROBE_SOURCE}}})
            for identifier, probe in enumerate(PROBES, 2):
                send({"jsonrpc": "2.0", "id": identifier, "method": "textDocument/definition", "params": {
                    "textDocument": {"uri": source.as_uri()}, "position": probe_position(PROBE_SOURCE, probe)}})
                response = receive(lambda message: message.get("id") == identifier)
                print(json.dumps(validate_definition(repository, probe, response.get("result"))), flush=True)
            send({"jsonrpc": "2.0", "id": 100, "method": "shutdown", "params": None})
            receive(lambda message: message.get("id") == 100)
            send({"jsonrpc": "2.0", "method": "exit", "params": None})
            process.wait(timeout=10)
            if process.returncode != 0:
                raise RuntimeError(f"rust-analyzer exited with {process.returncode}")
        except BaseException:
            log.flush()
            log.seek(0)
            print(log.read()[-32000:], flush=True)
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            reader.join(timeout=5)
            process.stdin.close()
            process.stdout.close()


def main():
    """Compile first; ordinary mode also requires all actual editor/map checks."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust-analyzer", default="rust-analyzer", help="Installed server executable or explicit path")
    parser.add_argument("--compile-only", action="store_true", help="Compile the exact probe without claiming editor navigation")
    options = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    environment = child_environment(repository)
    print("toolchain: " + environment["RUSTUP_TOOLCHAIN"], flush=True)
    with tempfile.TemporaryDirectory(prefix="neocogi-navigation-") as temporary:
        directory = Path(temporary)
        environment.setdefault("CARGO_TARGET_DIR", str(directory / "target"))
        source = prepare_probe(directory, repository)
        subprocess.run(["cargo", "check", "--manifest-path", str(directory / "Cargo.toml")],
                       cwd=directory, env=environment, check=True)
        print("exact navigation probe compiled", flush=True)
        if options.compile_only:
            return
        selected = shutil.which(options.rust_analyzer, path=environment.get("PATH"))
        if selected is None:
            raise FileNotFoundError("rust-analyzer unavailable; install the selected toolchain component or use --rust-analyzer")
        # Keep a rustup shim's basename: resolving its symlink would invoke rustup
        # itself rather than dispatch the requested rust-analyzer component.
        server = os.path.abspath(selected)
        version = subprocess.check_output([server, "--version"], cwd=repository, env=environment, text=True).strip()
        print("server: " + version, flush=True)
        navigate(server, directory, repository, environment, source)
        print(f"{len(PROBES)} actual editor definitions and exact IDL origins passed", flush=True)


if __name__ == "__main__":
    main()
