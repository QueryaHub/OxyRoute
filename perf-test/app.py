from oxyroute import App

app = App(title="Perf Test")


@app.get("/")
def hello() -> str:
    return "hello world"
