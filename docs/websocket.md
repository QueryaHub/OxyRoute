# WebSockets (native RSGI)

OxyRoute v0.3.0 ships a **native RSGI WebSocket** binding — the Granian
[`RSGIWebsocketProtocol`](https://github.com/emmett-framework/granian/blob/master/granian/rsgi.py)
is matched and dispatched directly inside the Rust `_oxyroute` extension. There is no ASGI
bridge: the legacy `--interface asgi` WebSocket path was removed alongside the rest of the
ASGI shim.

## Quick start

```python
from oxyroute import App, WebSocket

app = App()


@app.websocket("/ws/:room")
async def chat(ws: WebSocket) -> None:
    await ws.accept()
    room = ws.path_params["room"]
    await ws.send_text(f"hello, {room}")
    while True:
        msg = await ws.receive_text()
        if msg == "bye":
            break
        await ws.send_text(f"echo:{msg}")
    await ws.close()
```

Run it like any other OxyRoute app:

```bash
granian app:app --interface rsgi --host 127.0.0.1 --port 8000
```

## API

`oxyroute.WebSocket` is a thin Rust pyclass exported by the native module.

| Member | Kind | Description |
|---|---|---|
| `scope` | property | The Granian RSGI scope (`proto == "websocket"`). |
| `path_params` | property | `dict[str, str]` of path parameters extracted by the router. |
| `is_closed` | property | `True` once `close()` has run or the peer disconnected. |
| `await ws.accept()` | coroutine | Performs the handshake; required before send/receive. |
| `await ws.receive()` | coroutine | Returns the next frame as `str` **or** `bytes`. Raises `RuntimeError` if the peer closed. |
| `await ws.receive_text()` | coroutine | Like `receive`, but raises `ValueError` on a binary frame. |
| `await ws.receive_bytes()` | coroutine | Like `receive`, but raises `ValueError` on a text frame. |
| `await ws.send_text(s)` | coroutine | Send a text frame. |
| `await ws.send_bytes(b)` | coroutine | Send a binary frame. |
| `await ws.send_json(obj)` | coroutine | `json.dumps(obj)` then `send_text`. |
| `await ws.close(code=None)` | coroutine | Close the connection (defaults to 1000). Idempotent. |

Sync handlers are accepted by `@app.websocket(path)` for symmetry but should generally be
async — Granian dispatches WebSockets on its event loop and most useful patterns require
`await`.

## Routing semantics

* WebSocket routes live in their own `matchit::Router`. They never collide with HTTP routes,
  so `GET /ws/:room` and `WS /ws/:room` can coexist.
* Path syntax mirrors HTTP routes: `/ws/:room`, `/ws/:room/*rest`. Captured params are
  available via `ws.path_params`.
* Unknown WebSocket paths trigger a polite `protocol.close(1000)` (no 404 — close codes are
  the WebSocket equivalent).
* Calling `app.freeze()` locks WebSocket route registration the same way it locks HTTP
  routes; further `@app.websocket(...)` raises `ValueError`.

## Error handling

* If the handler raises before completing, OxyRoute logs the error and calls
  `protocol.close(1011)` (server-side error). The peer sees a clean close, never an open
  connection that hangs.
* If the **peer** closes (Granian sends `WebsocketMessageType.close`, kind `0`), the next
  `receive*` raises `RuntimeError("WebSocket closed by peer")` and `ws.is_closed` becomes
  `True`. A subsequent `await ws.close()` is a no-op (no double close).

## Testing

Drive the dispatcher in-process with mocked Granian-style scope/protocol/transport
objects — see [`tests/test_websocket_native.py`](../tests/test_websocket_native.py) for a
worked example. The pattern:

1. Build an `_WSScope` dataclass with `proto = "websocket"`, the request `path`, etc.
2. Build a mock `protocol` with an `async accept()` returning a transport, a
   `close(status)` setter, and the transport's `async receive() / send_str / send_bytes`.
3. Run `await app.handle_rsgi(scope, protocol)` from inside `asyncio.run(...)`.

For end-to-end coverage with a real Granian server use a subprocess test similar to
[`tests/test_granian_e2e.py`](../tests/test_granian_e2e.py), connecting with a real
WebSocket client (e.g. `websockets`).

## Migration from the previous ASGI WebSocket spike

The pre-v0.3.0 ASGI WebSocket helper was removed. If you were using it:

* Replace `from oxyroute.asgi import WebSocket` with `from oxyroute import WebSocket`.
* Drop any `--interface asgi` Granian command lines — OxyRoute is RSGI-only.
* `await ws.accept(subprotocol="…")` no longer accepts subprotocols (Granian's RSGI
  WebSocket handshake selects them via headers; document that explicitly if you need it).
