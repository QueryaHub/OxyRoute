# Native Rate Limiting

OxyRoute includes a high-performance in-memory **Token Bucket** rate limiter implemented natively in Rust (issue #162).

Because rate limiting is evaluated entirely in Rust before request body reading, JWT decoding, or entering Python, rejected requests are dropped in **~20–40 nanoseconds** without consuming Python GIL or worker CPU resources.

---

## Basic Usage

Declare rate limits on any route decorator:

```python
from oxyroute import App

app = App()

# 100 requests per minute per client IP
@app.get("/api/items", rate_limit="100/minute")
def get_items() -> dict[str, list[str]]:
    return {"items": ["item1", "item2"]}
```

---

## Rate Limit Windows

The `rate_limit` string accepts `<count>/<window>`:

* Per second: `"10/second"`, `"10/s"`
* Per minute: `"100/minute"`, `"100/m"`
* Per hour: `"1000/hour"`, `"1000/h"`
* Per day: `"10000/day"`, `"10000/d"`
* Custom seconds: `"50/30s"` (50 requests per 30 seconds)

---

## Key Strategies (`rate_limit_key`)

By default, rate limiting is tracked per client IP (`rate_limit_key="ip"`). You can customize the key:

### 1. Client IP (default)
Tracks client IP using the `client` tuple or `X-Forwarded-For` / `X-Real-IP` headers when behind reverse proxies:
```python
@app.get("/login", rate_limit="5/minute", rate_limit_key="ip")
def login() -> dict[str, str]:
    return {"status": "ok"}
```

### 2. Header-Based (e.g. API Keys or User IDs)
Rate limit by custom HTTP header (e.g. `header:X-API-Key` or `header:Authorization`):
```python
@app.get("/api/data", rate_limit="500/hour", rate_limit_key="header:X-API-Key")
def get_data() -> dict[str, str]:
    return {"data": "premium"}
```

### 3. Global Rate Limiting
All clients share a single rate bucket for the route:
```python
@app.post("/heavy-computation", rate_limit="10/second", rate_limit_key="global")
def heavy_job() -> dict[str, str]:
    return {"status": "queued"}
```

---

## Response on Limit Exceeded

When a client exceeds the limit, OxyRoute immediately returns a `429 Too Many Requests` response with IETF draft RateLimit headers:

* `RateLimit-Limit`: Maximum requests permitted in the window.
* `RateLimit-Remaining`: `0` when blocked.
* `RateLimit-Reset`: Number of seconds until tokens refill.
* `Retry-After`: Number of seconds the client should wait before retrying.

```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json; charset=utf-8
RateLimit-Limit: 100
RateLimit-Remaining: 0
RateLimit-Reset: 42
Retry-After: 42

{"detail":"Too Many Requests"}
```
