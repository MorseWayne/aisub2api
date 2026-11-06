#!/usr/bin/env python3
"""Offline fixtures for an installed Claude Code and a built aisub2api CLI.

Run with Python 3.10+, OpenSSL, and loopback socket access:
  python3 -I scripts/claude-compat-smoke.py --aisub2api target/debug/aisub2api

No model account or API credential is used. Synthetic responses drive tools;
this tests client configuration and tool execution, not real model quality or
the proxy's upstream converters. The HTTPS fixture never forwards traffic.
"""

import argparse
from contextlib import contextmanager
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import signal
import ssl
import subprocess
import sys
import tempfile
import threading
from urllib.parse import urlsplit


MODEL = "fixture/gpt-compat-smoke"
MCP_TOOL = "mcp__smoke__lookup_record_001"
READ_MARKER = "AISUB2API_SMOKE_READ"
PAGE_MARKER = "AISUB2API_SMOKE_PAGE"
MCP_MARKER = "AISUB2API_SMOKE_MCP"
DONE = "AISUB2API_SMOKE_DONE"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def run(command, *, env, cwd, timeout=45):
    process = subprocess.Popen(
        command, cwd=cwd, env=env, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        start_new_session=os.name == "posix",
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except BaseException as error:
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
        except ProcessLookupError:
            pass
        process.communicate()
        if isinstance(error, subprocess.TimeoutExpired):
            raise RuntimeError(f"{Path(command[0]).name} exceeded {timeout}s") from None
        raise
    require(process.returncode == 0,
            f"{Path(command[0]).name} exited {process.returncode}: {stderr[-1500:]}")
    return stdout


@contextmanager
def serve(handler):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def blocks(request, kind):
    return [block for message in request.get("messages", [])
            if isinstance(message.get("content"), list)
            for block in message["content"] if block.get("type") == kind]


def sse_response(model, sequence, tool=None):
    events = [("message_start", {"type": "message_start", "message": {
        "id": f"msg_smoke_{sequence}", "type": "message", "role": "assistant",
        "model": model, "content": [], "stop_reason": None,
        "usage": {"input_tokens": 100, "output_tokens": 0},
    }})]
    if tool:
        name, arguments = tool
        block = {"type": "tool_use", "id": f"call_smoke_{sequence}",
                 "name": name, "input": {}}
        delta = {"type": "input_json_delta", "partial_json": json.dumps(arguments)}
    else:
        block = {"type": "text", "text": ""}
        delta = {"type": "text_delta", "text": DONE}
    events += [
        ("content_block_start", {"type": "content_block_start", "index": 0,
                                 "content_block": block}),
        ("content_block_delta", {"type": "content_block_delta", "index": 0,
                                 "delta": delta}),
        ("content_block_stop", {"type": "content_block_stop", "index": 0}),
        ("message_delta", {"type": "message_delta", "delta": {
            "stop_reason": "tool_use" if tool else "end_turn", "stop_sequence": None,
        }, "usage": {"output_tokens": 20}}),
        ("message_stop", {"type": "message_stop"}),
    ]
    return "".join(f"event: {name}\ndata: {json.dumps(value)}\n\n"
                   for name, value in events).encode()


class QuietHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, status, data, content_type="application/json"):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True


def messages_handler(case, root, requests):
    class Messages(QuietHandler):
        def do_GET(self):
            self.reply(404, b"{}")

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
            path = urlsplit(self.path).path
            if path == "/v1/messages/count_tokens":
                self.reply(200, b'{"input_tokens":100}')
                return
            if path != "/v1/messages":
                self.reply(404, b"{}")
                return
            requests.append(body)
            called = {b["name"] for b in blocks(body, "tool_use")}
            tool = None
            if case == "tasks-and-read":
                if "TaskCreate" not in called:
                    tool = ("TaskCreate", {"subject": "Synthetic smoke task",
                            "description": "Local fixture only; no delegation."})
                elif "Read" not in called:
                    tool = ("Read", {"file_path": str(root / "fixture.txt")})
            elif case == "tool-search":
                if "ToolSearch" not in called:
                    tool = ("ToolSearch", {"query": f"select:{MCP_TOOL}", "max_results": 1})
                elif MCP_TOOL not in called:
                    tool = (MCP_TOOL, {"query": "synthetic"})
            elif len(requests) == 1:
                tool = ("WebFetch", {"url": "https://example.com/aisub2api-smoke",
                        "prompt": "Return the marker in this synthetic page."})
            self.reply(200, sse_response(body["model"], len(requests), tool), "text/event-stream")
    return Messages


def proxy_handler(context, network, allow_preflight):
    class Proxy(QuietHandler):
        protocol_version = "HTTP/1.1"

        def do_CONNECT(self):
            # Terminate TLS locally. No socket is opened to the requested host.
            if self.path not in {"api.anthropic.com:443", "example.com:443"}:
                self.reply(502, b"{}")
                return
            self.send_response(200)
            self.end_headers()
            self.close_connection = True
            try:
                with context.wrap_socket(self.connection, server_side=True) as tunnel:
                    tunnel.settimeout(10)
                    Proxy(tunnel, self.client_address, self.server)
            except (OSError, ssl.SSLError):
                network.append("tls-error")

        def do_GET(self):
            parsed = urlsplit(self.path)
            host = parsed.hostname or self.headers.get("Host", "")
            if host == "api.anthropic.com" and parsed.path == "/api/web/domain_info":
                network.append("preflight")
                self.reply(200 if allow_preflight else 503,
                           b'{"can_fetch":true}' if allow_preflight else b'{"error":"fixture"}')
            elif host == "example.com" and parsed.path == "/aisub2api-smoke":
                network.append("page")
                self.reply(200, PAGE_MARKER.encode(), "text/plain")
            else:
                network.append("blocked-other-request")
                self.reply(502, b"{}")

        def do_POST(self):
            network.append("blocked-other-request")
            self.reply(502, b"{}")
    return Proxy


def mcp_fixture():
    for line in sys.stdin:
        request = json.loads(line)
        if "id" not in request:
            continue
        method = request.get("method")
        if method == "initialize":
            result = {"protocolVersion": request["params"]["protocolVersion"],
                      "capabilities": {"tools": {}},
                      "serverInfo": {"name": "aisub2api-smoke", "version": "1"}}
        elif method == "tools/list":
            result = {"tools": [{
                "name": f"lookup_record_{i:03}",
                "description": "Read a synthetic compatibility record. " * 6,
                "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}},
                                "required": ["query"]},
            } for i in range(30)]}
        elif method == "tools/call":
            valid = request.get("params", {}).get("name") == "lookup_record_001"
            result = {"isError": not valid, "content": [{"type": "text",
                      "text": MCP_MARKER if valid else "Unexpected fixture tool"}]}
        else:
            result = {}
        print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)


def isolated_env(root):
    env = {key: os.environ[key] for key in ("PATH", "LANG", "LC_ALL", "TMPDIR", "SYSTEMROOT")
           if key in os.environ}
    env.update({
        "XDG_CONFIG_HOME": str(root / "config"), "XDG_CACHE_HOME": str(root / "cache"),
        "XDG_DATA_HOME": str(root / "data"), "CLAUDE_CONFIG_DIR": str(root / "claude"),
        "ANTHROPIC_API_KEY": "sk-ant-smoke-not-a-real-key",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1", "DISABLE_TELEMETRY": "1",
        "DISABLE_ERROR_REPORTING": "1", "DISABLE_AUTOUPDATER": "1",
        "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL": "1",
        "NO_PROXY": "127.0.0.1,localhost", "no_proxy": "127.0.0.1,localhost",
    })
    return env


def run_case(case, root, claude, aisub2api, context, certificate, timeout):
    root.mkdir()
    (root / "fixture.txt").write_text(READ_MARKER, encoding="utf-8")
    requests, network = [], []
    env = isolated_env(root)
    with serve(messages_handler(case, root, requests)) as messages, serve(
        proxy_handler(context, network, case == "webfetch-check-restored")
    ) as proxy:
        for key in ("HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"):
            env[key] = f"http://127.0.0.1:{proxy.server_port}"
        env["NODE_EXTRA_CA_CERTS"] = env["SSL_CERT_FILE"] = str(certificate)
        env["ANTHROPIC_BASE_URL"] = f"http://127.0.0.1:{messages.server_port}"
        (root / "input.toml").write_text(f'''
[providers.fixture]
provider_type = "openai"
api_key = "synthetic-upstream-key"
base_url = "http://127.0.0.1:{messages.server_port}/v1"
[model]
default = "{MODEL}"
[server]
host = "127.0.0.1"
port = {messages.server_port}
auth_token = "sk-ant-smoke-not-a-real-key"
''', encoding="utf-8")
        run([aisub2api, "config", "import", "input.toml"], env=env, cwd=root)
        settings_path = root / "claude/settings.json"
        if case in {"webfetch-skipped", "webfetch-check-restored"}:
            run([aisub2api, "config", "claude", "--webfetch-preflight", "skip"], env=env, cwd=root)
            if case == "webfetch-check-restored":
                run([aisub2api, "config", "claude", "--webfetch-preflight", "check"], env=env, cwd=root)
            run([aisub2api, "config", "claude"], env=env, cwd=root)
        before = settings_path.read_bytes()
        run([aisub2api, "config", "validate"], env=env, cwd=root)
        require(settings_path.read_bytes() == before, "config validate changed Claude settings")
        settings = json.loads(before)
        if case == "webfetch-skipped":
            require(settings.get("skipWebFetchPreflight") is True, "normal sync lost the WebFetch option")
        elif case == "webfetch-check-restored":
            require(settings.get("skipWebFetchPreflight") is False, "check did not restore preflight")
        else:
            require("skipWebFetchPreflight" not in settings, "sync opted out of WebFetch checks by default")
        # Explicit --settings also works in restricted mode; no user/project
        # settings or pre-existing MCP servers are loaded.
        settings["env"]["ENABLE_TOOL_SEARCH"] = "true" if case == "tool-search" else "auto"
        (root / "client-settings.json").write_text(json.dumps(settings), encoding="utf-8")
        mcp = {"mcpServers": {}}
        if case == "tool-search":
            mcp["mcpServers"]["smoke"] = {
                "command": sys.executable,
                "args": ["-I", str(Path(__file__).resolve()), "--mcp-fixture"],
            }
        command = [claude, "-p", "Run the local synthetic compatibility fixture.",
                   "--restricted", "--tools", "Read,TaskCreate,TaskGet,TaskList,TaskUpdate,ToolSearch,WebFetch",
                   "--allowedTools", f"Read,TaskCreate,ToolSearch,WebFetch,{MCP_TOOL}",
                   "--permission-mode", "dontAsk", "--model", MODEL,
                   "--system-prompt", "Local deterministic compatibility fixture. No delegation.",
                   "--no-session-persistence", "--output-format", "json", "--no-chrome",
                   "--disable-slash-commands", "--strict-mcp-config", "--mcp-config", json.dumps(mcp),
                   "--setting-sources", "", "--settings", str(root / "client-settings.json")]
        result = json.loads(run(command, env=env, cwd=root, timeout=timeout))
    require(result.get("is_error") is False and result.get("result") == DONE,
            "Claude did not complete the synthetic conversation")
    require(requests and all(q["model"] == MODEL for q in requests), "unexpected model route")
    results = blocks(requests[-1], "tool_result")
    if case == "tasks-and-read":
        names = {t["name"] for t in requests[0].get("tools", [])}
        require({"Read", "TaskCreate", "TaskGet", "TaskList", "TaskUpdate"} <= names,
                "generated settings did not expose task/read tools")
        require(READ_MARKER in json.dumps(results), "Read did not return the synthetic file")
        require(len(results) == 2, "TaskCreate/Read result pair is missing")
    elif case == "tool-search":
        require(any(t["name"] == "ToolSearch" for t in requests[0].get("tools", [])), "ToolSearch not registered")
        initial = [t for t in requests[0].get("tools", []) if t["name"] == MCP_TOOL]
        require(not initial or all(t.get("defer_loading") for t in initial), "MCP schema was not deferred")
        # A discovered tool may retain defer_loading; the contract is that its
        # complete schema arrives and its actual result returns to the model.
        require(any(t["name"] == MCP_TOOL
                    and "query" in t.get("input_schema", {}).get("properties", {})
                    for q in requests[1:] for t in q.get("tools", [])), "discovered MCP schema never arrived")
        require(MCP_MARKER in json.dumps(results), "discovered MCP tool was not executed")
    elif case == "webfetch-failed":
        require(network.count("preflight") == 1 and "page" not in network,
                "failed preflight did not stop the page fetch")
        require(len(results) == 1 and results[0].get("is_error") is True,
                "WebFetch failure was hidden by the final successful conversation")
        return {"case": case, "passed": True, "messages_requests": len(requests), "expected_tool_error": True}
    else:
        require("page" in network, "WebFetch never requested the synthetic page")
        require(("preflight" in network) == (case == "webfetch-check-restored"), "preflight option had no effect")
        require(any(PAGE_MARKER in json.dumps(q) for q in requests[1:]), "page content never reached the model")
    require(results and not any(b.get("is_error") for b in results), "a required tool failed")
    return {"case": case, "passed": True, "messages_requests": len(requests)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", default=shutil.which("claude"), help="installed Claude Code executable")
    parser.add_argument("--aisub2api", default="target/debug/aisub2api", help="built CLI executable")
    parser.add_argument("--timeout", type=float, default=45, help="per-client timeout in seconds")
    parser.add_argument("--output", type=Path, help="also save the JSON summary")
    args = parser.parse_args()
    require(args.claude is not None, "Claude Code not found; pass --claude")
    require(args.timeout > 0, "--timeout must be positive")
    claude = str(Path(args.claude).resolve())
    aisub2api = str(Path(args.aisub2api).resolve())
    require(Path(claude).is_file() and Path(aisub2api).is_file(), "build aisub2api and install Claude Code first")
    require(shutil.which("openssl") is not None, "OpenSSL is required for the local HTTPS fixture")
    report = {"claude_sha256": hashlib.sha256(Path(claude).read_bytes()).hexdigest(),
              "aisub2api_sha256": hashlib.sha256(Path(aisub2api).read_bytes()).hexdigest(),
              "scope": "generated client configuration with synthetic model/MCP/HTTPS fixtures",
              "cases": []}
    try:
        with tempfile.TemporaryDirectory(prefix="aisub2api-claude-smoke-") as directory:
            root = Path(directory)
            env = isolated_env(root)
            report["claude_version"] = run([claude, "--version"], env=env, cwd=root, timeout=10).strip()
            certificate, key = root / "cert.pem", root / "key.pem"
            run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                 "-keyout", str(key), "-out", str(certificate), "-subj", "/CN=api.anthropic.com",
                 "-addext", "subjectAltName=DNS:api.anthropic.com,DNS:example.com"], env=env, cwd=root, timeout=15)
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(str(certificate), str(key))
            for case in ("tasks-and-read", "tool-search", "webfetch-failed", "webfetch-skipped", "webfetch-check-restored"):
                print(f"Checking {case}...", file=sys.stderr, flush=True)
                try:
                    report["cases"].append(run_case(case, root / case, claude, aisub2api,
                                                    context, certificate, args.timeout))
                except Exception as error:
                    report["cases"].append({"case": case, "passed": False, "error": str(error)})
    except Exception as error:
        report["error"] = str(error)
    report["passed"] = len(report["cases"]) == 5 and all(c["passed"] for c in report["cases"])
    output = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(output, encoding="utf-8")
    print(output, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    if sys.argv[1:] == ["--mcp-fixture"]:
        mcp_fixture()
    else:
        try:
            sys.exit(main())
        except RuntimeError as error:
            print(str(error), file=sys.stderr)
            sys.exit(1)
