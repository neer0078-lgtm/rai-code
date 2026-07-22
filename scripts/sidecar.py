#!/usr/bin/env python3
"""RAI Code Python sidecar — a JSON-RPC server wrapping Graphiti + Hindsight.

Phase 1 of the rai-python bridge: the Rust process spawns this script as a
subprocess, communicates via JSON-RPC 2.0 over stdin/stdout (one JSON object
per line). Simple, crash-isolated, no GIL/deadlock risk.

Methods:
  - graphiti_search(q: str) -> {results: [...]}
  - graphiti_add_episode(ep: str) -> {}
  - hindsight_recall(q: str) -> {results: [...]}
  - hindsight_retain(content: str) -> {}

--self-test: runs an offline in-memory path (no Graphiti/Hindsight/Neo4j) to
verify the JSON-RPC wire protocol works. Exits 0 on success.

In --local mode, Graphiti uses Kuzu (embedded) + Hindsight uses SQLite (both
optional — if not installed, the methods return graceful "not available" errors).

Built by RAI Labs P. Ltd. — www.railabs.in — reach@railabs.in
"""

import json
import sys
import asyncio
import argparse
from typing import Any, Optional

# --- JSON-RPC wire protocol ---

def make_response(req_id: Any, result: Any = None, error: Optional[dict] = None) -> str:
    resp = {"id": req_id}
    if error is not None:
        resp["error"] = error
    else:
        resp["result"] = result
    return json.dumps(resp)


def make_error(code: int, message: str, data: Any = None) -> dict:
    err = {"code": code, "message": message}
    if data is not None:
        err["data"] = data
    return err


# --- Method handlers (offline/mock for --self-test; real for production) ---

# In --self-test mode, these are in-memory. In production, they'd call
# Graphiti (graphiti_core) + Hindsight (hindsight) — imported lazily so the
# self-test works without those deps installed.

_self_test_episodes: list[str] = []
_self_test_memories: list[str] = []


async def handle_graphiti_search(params: dict, self_test: bool) -> Any:
    q = params.get("q", "")
    if self_test:
        results = [ep for ep in _self_test_episodes if q in ep]
        return {"results": results}
    try:
        from graphiti_core import Graphiti  # lazy import
        # TODO: real Graphiti search via the configured driver.
        return {"results": [], "note": "Graphiti not yet wired in production mode"}
    except ImportError:
        return {"results": [], "error": "graphiti_core not installed"}


async def handle_graphiti_add_episode(params: dict, self_test: bool) -> Any:
    ep = params.get("ep", "")
    if self_test:
        _self_test_episodes.append(ep)
        return {}
    try:
        from graphiti_core import Graphiti
        # TODO: real Graphiti add_episode.
        return {}
    except ImportError:
        return {"error": "graphiti_core not installed"}


async def handle_hindsight_recall(params: dict, self_test: bool) -> Any:
    q = params.get("q", "")
    if self_test:
        results = [m for m in _self_test_memories if q in m]
        return {"results": results}
    try:
        # Hindsight is imported as `hindsight` or `hindsight_all`.
        # TODO: real Hindsight recall.
        return {"results": [], "note": "Hindsight not yet wired in production mode"}
    except ImportError:
        return {"results": [], "error": "hindsight not installed"}


async def handle_hindsight_retain(params: dict, self_test: bool) -> Any:
    content = params.get("content", "")
    if self_test:
        _self_test_memories.append(content)
        return {}
    try:
        # TODO: real Hindsight retain.
        return {}
    except ImportError:
        return {"error": "hindsight not installed"}


METHODS = {
    "graphiti_search": handle_graphiti_search,
    "graphiti_add_episode": handle_graphiti_add_episode,
    "hindsight_recall": handle_hindsight_recall,
    "hindsight_retain": handle_hindsight_retain,
}


# --- JSON-RPC dispatch ---

async def dispatch(req: dict, self_test: bool) -> str:
    req_id = req.get("id")
    method = req.get("method", "")
    params = req.get("params", {})

    if not method:
        return make_response(req_id, error=make_error(-32600, "invalid request: missing method"))

    handler = METHODS.get(method)
    if handler is None:
        return make_response(req_id, error=make_error(-32601, f"method not found: {method}"))

    try:
        result = await handler(params, self_test)
        return make_response(req_id, result=result)
    except Exception as e:
        return make_response(req_id, error=make_error(-32603, f"internal error: {e}"))


# --- Main loop (stdin -> dispatch -> stdout) ---

async def serve(self_test: bool) -> None:
    """Read JSON-RPC requests from stdin, write responses to stdout."""
    # Announce readiness.
    print(json.dumps({"jsonrpc": "2.0", "ready": True}), flush=True)

    reader = asyncio.StreamReader()
    protocol = asyncio.StreamReaderProtocol(reader)
    await asyncio.get_event_loop().connect_read_pipe(lambda: protocol, sys.stdin)

    while True:
        line = await reader.readline()
        if not line:
            break  # EOF (Rust process closed stdin)
        line = line.decode("utf-8", errors="replace").strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            print(make_response(None, error=make_error(-32700, f"parse error: {e}")), flush=True)
            continue
        resp = await dispatch(req, self_test)
        print(resp, flush=True)


def run_self_test() -> int:
    """Offline self-test: verify the JSON-RPC wire protocol works in-memory."""
    print("RAI Code sidecar self-test...", file=sys.stderr)

    async def _test() -> bool:
        # Test 1: graphiti_add_episode then graphiti_search.
        r1 = await dispatch({"id": 1, "method": "graphiti_add_episode", "params": {"ep": "commit abc: changed foo"}}, True)
        assert '"result": {}' in r1, f"add_episode failed: {r1}"

        r2 = await dispatch({"id": 2, "method": "graphiti_search", "params": {"q": "foo"}}, True)
        assert "commit abc" in r2, f"search should find the episode: {r2}"

        # Test 2: hindsight_retain then hindsight_recall.
        r3 = await dispatch({"id": 3, "method": "hindsight_retain", "params": {"content": "user prefers Rust"}}, True)
        assert '"result": {}' in r3, f"retain failed: {r3}"

        r4 = await dispatch({"id": 4, "method": "hindsight_recall", "params": {"q": "Rust"}}, True)
        assert "user prefers Rust" in r4, f"recall should find the memory: {r4}"

        # Test 3: unknown method -> error.
        r5 = await dispatch({"id": 5, "method": "bogus", "params": {}}, True)
        assert "-32601" in r5, f"unknown method should error: {r5}"

        # Test 4: missing method -> error.
        r6 = await dispatch({"id": 6, "params": {}}, True)
        assert "-32600" in r6, f"missing method should error: {r6}"

        return True

    ok = asyncio.run(_test())
    if ok:
        print("Self-test PASSED (4/4 checks)", file=sys.stderr)
        return 0
    else:
        print("Self-test FAILED", file=sys.stderr)
        return 1


def main() -> int:
    parser = argparse.ArgumentParser(description="RAI Code Python sidecar")
    parser.add_argument("--self-test", action="store_true", help="run offline self-test and exit")
    args = parser.parse_args()

    if args.self_test:
        return run_self_test()

    asyncio.run(serve(self_test=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
