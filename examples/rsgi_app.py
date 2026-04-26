"""
Run: ``granian --interface rsgi examples.rsgi_app:app`` from the project root
(after ``pip install -e .`` or ``pip install oxyroute granian``).

See https://github.com/QueryaHub/OxyRoute/blob/main/docs/rsgi.md
"""

from oxyroute import App

app = App(title="Hello OxyRoute")


@app.get("/")
def root() -> str:
    return "OxyRoute RSGI OK"


@app.get("/hello/:name")
def hello_name(**kwargs) -> str:
    # Path params are passed as keyword args (``name`` from ``:name`` in the route).
    return f"Hello, {kwargs.get('name', '')}"
