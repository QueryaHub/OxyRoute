# Dependencies (`Depends` and `dependencies=…`)

[← Documentation index](index.md)

OxyRoute supports a **linear** list of **named** dependency factories. At request time, each factory is called in order; its return value is injected into the route handler as a **keyword argument** with the given name. Factories that appear **later** in the list are called with **keyword arguments** for every **earlier** name and value (so a factory can depend on a previous one by using the same parameter name, e.g. `def b(a: int): …` when the first tuple is `("a", make_a)`).

### Request context (optional)

If a factory’s signature includes a parameter named `request`, the extension passes an `oxyroute.Request` object (once per request, shared). The `Request` object has typed accessors: `method`, `path`, `query_string`, `headers`, `client`, and `cookies`. For backwards compatibility, it can still be accessed like a dictionary (e.g. `request["headers"]`).

### Lazy Headers
To avoid performance overhead when headers are not needed by the handler or dependencies, the `Request.headers` dict is populated lazily. The full header map is only built on the first access of `.headers` (or `request["headers"]`).

Factories that do **not** declare `request` are still called with **no** extra arguments when they have no prior dependencies, preserving older behavior.

## Declaring on a route

Pass a list of two-tuples `(name, factory)` to a route decorator, for example:

```python
def get_db() -> str:
    return "db-conn"

@app.get("/items", dependencies=[("db", get_db)])
def list_items(db: str) -> str:
    return f"ok {db}"
```

`factory` can be **sync** or **async** (the extension detects `async` factories and awaits them in order). Dependency **names** must be **unique** in the list.

**Example (chaining):** `dependencies=[("a", make_a), ("b", make_b)]` with `def make_b(a): return a + 1` — the second callable receives the value bound to `a`.

## `Depends` marker

`Depends(callable)` returns a small native `PyDepends` object so you can use a FastAPI-style appearance:

```python
from oxyroute import App, Depends

def get_settings():
    return {"env": "dev"}

@app.get("/x", dependencies=[("settings", Depends(get_settings))])
def x(**kwargs) -> str:
    return "ok"
```

The underlying callables are unwrapped in Python and passed to the native `add_route` as plain `(name, fn)` pairs.

## Freezing route registration

Calling **`app.freeze()`** (forwarded to the native `App`) sets the app to **no longer accept** new routes—use this when you want a final route table before serving (future-proofing for DI graphs and similar). The native layer also **clones the per-method `matchit` routers** into a read-only snapshot so that **path matching no longer takes per-router mutexes** on the hot request path (the mutable copies remain for introspection alignment with the snapshot).

## See also

- [Handlers](handlers.md) — how kwargs are merged
- [RSGI and Granian](rsgi.md)
