from fastapi import FastAPI
from fastapi.responses import PlainTextResponse

app = FastAPI(title="Perf Test FastAPI")


@app.get("/", response_class=PlainTextResponse)
def hello() -> str:
    return "hello world"
