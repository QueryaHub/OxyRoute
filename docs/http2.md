# HTTP/2 with Granian

[← Documentation index](index.md)

OxyRoute does not implement HTTP transport itself. HTTP/2 is negotiated and served by the host server (typically Granian) and, in many deployments, by a reverse proxy or load balancer in front of it.

## What OxyRoute guarantees

- The same request pipeline works for HTTP/1.1 and HTTP/2 once a request reaches OxyRoute (`__rsgi__` -> native `run_rsgi`).
- Routing, body parsing, JWT checks, middleware, and response mapping are protocol-version agnostic at the framework layer.
- If `scope.http_version` is exposed by the server, handlers can inspect it via the injected `request` context (`request["scope"]`).

## What depends on server/deployment

- TLS termination and ALPN negotiation (`h2` vs `http/1.1`).
- End-to-end HTTP/2 preservation across proxy hops.
- Connection-level behavior (stream prioritization, flow control, max concurrent streams).

In short: OxyRoute guarantees HTTP semantics on requests it receives; transport negotiation is owned by Granian and your edge/proxy stack.

## Current caveats

- No framework API for HTTP/2 server push (deprecated in browsers anyway).
- No first-class API for HTTP trailers.
- No pseudo-header API (`:authority`, `:scheme`); consume normalized values from the request scope exposed by the server.

These are intentional for now and match the "server owns transport details" design.

## Practical smoke check

You can verify deployment behavior with an HTTP/2-capable client:

```python
import httpx

with httpx.Client(http2=True, verify=True) as c:
    r = c.get("https://api.example.com/health")
    print(r.http_version)  # expected: HTTP/2
```

If you see `HTTP/1.1`, the fallback happened at server/proxy/TLS level, not in OxyRoute routing code.

## See also

- [RSGI and Granian](rsgi.md)
- [ASGI bridge](asgi.md)
