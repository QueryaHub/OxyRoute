# Dependencies (`Depends` and `dependencies=…`)

[← Documentation index](index.md)

OxyRoute supports a **linear** list of **named** dependency factories. At request time, each factory is called in order; its return value is injected into the route handler as a **keyword argument** with the given name.

## Declaring on a route

Pass a list of two-tuples `(name, factory)` to a route decorator, for example:

```python
def get_db() -> str:
    return "db-conn"

@app.get("/items", dependencies=[("db", get_db)])
def list_items(db: str) -> str:
    return f"ok {db}"
```

`factory` can be **sync** or **async** (the extension detects `async` factories and awaits them in order).

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

Calling **`app.freeze()`** (forwarded to the native `App`) sets the app to **no longer accept** new routes—use this when you want a final route table before serving (future-proofing for DI graphs and similar).

## See also

- [Handlers](handlers.md) — how kwargs are merged
- [RSGI and Granian](rsgi.md)
