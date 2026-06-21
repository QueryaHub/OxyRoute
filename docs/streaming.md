# Streaming Responses

[← Documentation index](index.md)

OxyRoute provides streaming response helpers in `oxyroute.streaming` for returning chunked HTTP responses without buffering the entire body in memory. Server-Sent Events (SSE) are available in `oxyroute.sse`.

## Quick start

```python
import asyncio
from oxyroute import App, stream_text

app = App()

@app.get("/logs")
async def logs(protocol):
    async def tail_logs():
        for i in range(5):
            yield f"Log line {i}\n"
            await asyncio.sleep(1)

    # `stream_text` formats chunks as `text/plain`
    return await stream_text(protocol, tail_logs())
```

## Available Helpers

All helpers require the `protocol` argument and an iterable or async iterable of data.

- `stream_bytes(protocol, iterable, *, status=200, headers=None, content_type="application/octet-stream")`
  Streams raw `bytes`. Useful for file downloads and proxying binary streams.

- `stream_text(protocol, iterable, *, status=200, headers=None, content_type="text/plain; charset=utf-8")`
  Streams `str` chunks.

- `stream_jsonl(protocol, iterable, *, status=200, headers=None)`
  Takes an iterable of dicts/lists/objects and streams them as NDJSON (JSON-Lines) with `content-type: application/x-ndjson; charset=utf-8`.

- `send_sse(protocol, events, *, status=200, headers=None)`
  Streams items as Server-Sent Events with `content-type: text/event-stream; charset=utf-8`.

## Behavior & Backpressure

- On RSGI servers supporting `response_stream` (like Granian), chunks are sent incrementally.
- Awaiting the streaming helpers automatically respects TCP backpressure. If the client is slow to read, Granian pauses the underlying stream, causing `await stream.send_bytes(...)` to block, which in turn pauses your async generator.
- On transports without streaming support (e.g., the integrated test client or ASGI bridging), OxyRoute falls back to buffering all chunks into memory and returning a single response.

## Caveats

- Browser or intermediate proxy buffering can delay chunk delivery unless buffering is explicitly disabled at the edge.
- Handlers returning streams **must** be `async def` and must `await` the streaming helper, because the helpers themselves run async I/O.

## See also

- [Handlers](handlers.md)
- [HTTP/2 with Granian](http2.md)
