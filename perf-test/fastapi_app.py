from fastapi import FastAPI

app = FastAPI(title="Perf Test FastAPI")


@app.get("/")
def hello() -> str:
    return "hello world"
