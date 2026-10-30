"""Drive an MCP stdio server through a scripted session and print every exchange.

Usage: python3 -I mcp-drive.py <steps.json> [--expect-db <path>] -- <server command...>

The steps file is a JSON list. Each step is one of
  {"call": "<tool>", "args": {...}}          a tools/call
  {"record": {...record_batch args...}}        a dry run, then the write with its reviewToken
  {"list": true}                               a tools/list
  {"raw": {"method": "...", "params": {...}}}  any other request
A record step skips the write when the dry run is refused.
A call step with "save": "<name>" keeps its structuredContent. A later step's argument string
"$<name>.<field>.<field>" is replaced by that saved value, so a read_receipt answer's receipt
object can go into a record step. "${FIXTURES}" in the steps file is the folder holding it.
The server runs with "/" as its working directory, as it does under Claude Code.
The client sends initialize and notifications/initialized first. With --expect-db it then calls
saldo_context and stops before any step unless databasePath is exactly <path>. It exits non-zero
when the server closes early, answers with a JSON-RPC error, or the database does not match.
"""

import json
import os
import subprocess
import sys


def main() -> int:
    sep = sys.argv.index("--")
    options = sys.argv[1:sep]
    steps_path = options[0]
    expect_db = options[options.index("--expect-db") + 1] if "--expect-db" in options else None
    with open(steps_path) as f:
        fixtures = os.path.dirname(os.path.abspath(steps_path))
        steps = json.loads(f.read().replace("${FIXTURES}", fixtures))
    saved = {}

    def resolve(value):
        if isinstance(value, dict):
            return {key: resolve(item) for key, item in value.items()}
        if isinstance(value, list):
            return [resolve(item) for item in value]
        if isinstance(value, str) and value.startswith("$") and value[1:].split(".")[0] in saved:
            name, *path = value[1:].split(".")
            found = saved[name]
            for key in path:
                found = found[key]
            return found
        return value

    proc = subprocess.Popen(
        sys.argv[sep + 1 :],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr,
        text=True,
        bufsize=1,
        cwd="/",
    )
    next_id = 0

    def request(method, params=None):
        nonlocal next_id
        next_id += 1
        message = {"jsonrpc": "2.0", "id": next_id, "method": method}
        if params is not None:
            message["params"] = params
        print(">>> " + json.dumps(message, ensure_ascii=False))
        proc.stdin.write(json.dumps(message) + "\n")
        proc.stdin.flush()
        while True:
            line = proc.stdout.readline()
            if not line:
                raise SystemExit("server closed stdout before answering " + method)
            reply = json.loads(line)
            if reply.get("id") == next_id:
                print("<<< " + json.dumps(reply, ensure_ascii=False, indent=2))
                return reply
            print("<<< (notification) " + line.strip())

    def notify(method):
        message = {"jsonrpc": "2.0", "method": method}
        print(">>> " + json.dumps(message))
        proc.stdin.write(json.dumps(message) + "\n")
        proc.stdin.flush()

    failed = "error" in request(
        "initialize",
        {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "mcp-drive", "version": "1"},
        },
    )
    notify("notifications/initialized")

    if expect_db is not None:
        context = request("tools/call", {"name": "saldo_context", "arguments": {}})
        opened = context.get("result", {}).get("structuredContent", {}).get("databasePath")
        if opened != expect_db:
            proc.kill()
            raise SystemExit(f"refusing to run the steps: the server opened {opened}, not {expect_db}")

    for step in steps:
        step = resolve(step)
        if "record" in step:
            batch = {key: value for key, value in step["record"].items() if key not in ("dryRun", "reviewToken")}
            reply = request("tools/call", {"name": "record_batch", "arguments": {**batch, "dryRun": True}})
            preview = reply.get("result", {})
            if "error" not in reply and not preview.get("isError"):
                token = preview.get("structuredContent", {}).get("reviewToken")
                reply = request(
                    "tools/call",
                    {"name": "record_batch", "arguments": {**batch, "dryRun": False, "reviewToken": token}},
                )
        elif "call" in step:
            reply = request("tools/call", {"name": step["call"], "arguments": step.get("args", {})})
            if "save" in step:
                saved[step["save"]] = reply.get("result", {}).get("structuredContent", {})
        elif step.get("list"):
            reply = request("tools/list", {})
        else:
            reply = request(step["raw"]["method"], step["raw"].get("params"))
        failed |= "error" in reply

    proc.stdin.close()
    code = proc.wait(timeout=10)
    print(f"server exited with {code}")
    return 1 if failed or code != 0 else 0


if __name__ == "__main__":
    sys.exit(main())
