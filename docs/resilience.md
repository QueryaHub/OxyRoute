# Concurrency Limiting & Load Shedding

OxyRoute includes native Rust-level concurrency limiting and fast load shedding to prevent memory spikes (OOM), queue buildup, and tail latency degradation during traffic surges.

---

## Static Concurrency Limiting

You can configure a fixed maximum concurrency limit at the application level:

```python
from oxyroute import App

# Limit concurrent in-flight requests to 100
app = App(max_concurrency=100)

@app.get("/items")
def list_items():
    return [{"id": 1}]
```

You can also adjust or inspect the limit at runtime:

```python
app.set_max_concurrency(200)
limit = app.get_max_concurrency()
active = app.get_in_flight()
```

### Fast Load Shedding

When in-flight requests exceed `max_concurrency`, the Rust dispatcher immediately rejects incoming requests with `503 Service Unavailable` and a `Retry-After: 1` header before invoking Python handlers or allocating asyncio futures.

---

## Adaptive Concurrency Limiting

Under variable workloads or downstream latency degradation (e.g. slow database queries), `AdaptiveConcurrencyLimiter` uses a TCP Vegas / Little's Law gradient algorithm to dynamically adjust `max_concurrency`:

```python
from oxyroute import App, AdaptiveConcurrencyLimiter

app = App()
limiter = AdaptiveConcurrencyLimiter(
    app,
    min_limit=8,
    max_limit=256,
    initial_limit=32,
)

# Connect to access_log_hook to measure request latency automatically
app.access_log_hook = limiter.as_access_log_hook()
```
