# Server-Sent Events (SSE)

[← Documentation index](index.md)

OxyRoute provides a small SSE helper in `oxyroute.sse` for HTTP event streams.

## Quick start

```python
from oxyroute import App, send_sse

app = App()


@app.get("/events")
async def events(protocol):
    return await send_sse(protocol, ["ready", "tick"])
```

## API

- `send_sse(protocol, events, *, status=200, headers=None)`:
  - sets `content-type: text/event-stream; charset=utf-8`,
  - formats items as SSE frames (`data: ...\n\n`),
  - returns a sentinel consumed by OxyRoute so no second response is emitted.
- Event items can be:
  - `str` (serialized as `data: <value>`),
  - `SSEEvent(data=..., event=..., id=..., retry=...)`.

## Streaming behavior

- On RSGI protocols exposing `response_stream`, chunks are written incrementally.
- On transports without streaming support (e.g. current ASGI bridge test path), OxyRoute falls back to one buffered `response_str` body with SSE framing.

## Caveats

- Browser/proxy buffering can delay event delivery unless buffering is disabled at the edge.
- SSE is one-way server-to-client messaging over HTTP; use WebSockets for bi-directional flows.
- HTTP/1.1 and HTTP/2 transport negotiation is server/proxy responsibility; SSE framing itself is unchanged.

## See also

- [Handlers](handlers.md)
- [HTTP/2 with Granian](http2.md)
