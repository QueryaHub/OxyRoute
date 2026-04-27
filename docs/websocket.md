# WebSocket (ASGI spike)

[← Documentation index](index.md)

Current WebSocket support in OxyRoute is an **ASGI bridge spike**, not full RSGI-native support yet.

## Current API

Use `@app.websocket(path)` for exact-path handlers on `App.__call__` (ASGI entry):

```python
from oxyroute import App

app = App()


@app.websocket("/ws")
async def ws(sock):
    await sock.accept()
    text = await sock.receive_text()
    await sock.send_text(f"echo:{text}")
    await sock.close()
```

The handler receives a small `WebSocket` helper with:

- `accept(subprotocol=None)`
- `receive()` / `receive_text()`
- `send_text(text)` / `send_bytes(data)`
- `close(code=1000)`

## Scope and limitations

- Works on the optional ASGI entry (`app(scope, receive, send)`).
- **Not** wired into Rust request routing (`run_rsgi`) yet.
- Path matching is currently exact string match (no path-params router for WS yet).
- No first-class dependency/JWT/middleware chain for WS handlers in this spike.

## Design split (what lives where)

- **Python/ASGI now:** websocket handshake + frame loop helper and handler dispatch.
- **Rust/RSGI later:** unified route table, WS protocol lifecycle in native path, shared middleware/auth story.

This keeps a practical testable path now while preserving room for a proper RSGI-native implementation.

## See also

- [ASGI bridge](asgi.md)
- [Feature gaps](feature.md)
