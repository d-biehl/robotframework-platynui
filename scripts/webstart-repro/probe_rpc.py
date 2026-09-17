"""Talk to a running PlatynUI agent: handshake, then read its window tree.

Newline-delimited JSON-RPC 2.0 over loopback TCP, as crates/java-agent/src/client.rs speaks it.
Usage: probe_rpc.py <handshake-file> <client-version>
"""

import json
import socket
import sys

handshake = json.loads(open(sys.argv[1], encoding="utf-8").read())
version = sys.argv[2]

sock = socket.create_connection(("127.0.0.1", handshake["port"]), timeout=5)
stream = sock.makefile("rwb")
next_id = 0


def call(method, params):
    global next_id
    next_id += 1
    request = {"jsonrpc": "2.0", "id": next_id, "method": method, "params": params}
    stream.write((json.dumps(request) + "\n").encode("utf-8"))
    stream.flush()
    while True:
        line = stream.readline()
        if not line:
            raise SystemExit(f"connection closed while waiting for {method}")
        message = json.loads(line)
        if message.get("id") == next_id:
            return message


print("handshake ->", json.dumps(call("handshake", {"token": handshake["token"], "clientVersion": version})))
print("windows   ->", json.dumps(call("ui/windows", {})))
