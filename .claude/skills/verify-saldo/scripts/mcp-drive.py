"""Drive an MCP stdio server through a scripted session and print every exchange.

Usage: python3 -I mcp-drive.py <steps.json> [--expect-db <path>] -- <server command...>

The steps file is a JSON list. Each step is one of
  {"call": "<tool>", "args": {...}}          a tools/call
  {"list": true}                               a tools/list
  {"raw": {"method": "...", "params": {...}}}  any other request
The client sends initialize and notifications/initialized first. With --expect-db it then calls
saldo_context and stops before any step unless databasePath is exactly <path>. It exits non-zero
when the server closes early, answers with a JSON-RPC error, or the database does not match.
"""

import json
import subprocess
import sys


def main() -> int:
    sep = sys.argv.index("--")
    options = sys.argv[1:sep]
    steps_path = options[0]
    expect_db = options[options.index("--expect-db") + 1] if "--expect-db" in options else None
    with open(steps_path) as f:
        steps = json.load(f)

    proc = subprocess.Popen(
        sys.argv[sep + 1 :],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=sys.stderr,
        text=True,
        bufsize=1,
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
        if "call" in step:
            reply = request("tools/call", {"name": step["call"], "arguments": step.get("args", {})})
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
