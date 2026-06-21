import asyncio

import httpx
from oxyroute import App, Response
from tests._rsgi_test_transport import asgi_test_app


def test_exception_handlers():
    class CustomError(Exception):
        def __init__(self, msg: str):
            self.msg = msg

    class SubCustomError(CustomError):
        pass

    app = App()

    # Note: the `add_exception_handler` method might be used as a decorator or a regular method.
    # We didn't implement it as a decorator returning the function, but in our `app.py` it's just:
    # def add_exception_handler(self, exc_type: type[BaseException], handler: Callable[..., Any]) -> None:
    # So we call it directly.

    def handle_custom_error(scope, exc):
        return Response(status=400, body=exc.msg.encode())

    async def handle_sub_custom_error(scope, exc):
        return {"status": 418, "body": "sub error"}

    app.add_exception_handler(CustomError, handle_custom_error)
    app.add_exception_handler(SubCustomError, handle_sub_custom_error)

    @app.get("/error1")
    def error1() -> str:
        raise CustomError("test1")

    @app.get("/error2")
    async def error2() -> str:
        raise SubCustomError("test2")

    @app.get("/unhandled")
    def unhandled() -> str:
        raise ValueError("oh no")

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r1 = await c.get("/error1")
            assert r1.status_code == 400
            assert r1.text == "test1"

            r2 = await c.get("/error2")
            assert r2.status_code == 418
            assert r2.text == "sub error"

            r3 = await c.get("/unhandled")
            assert r3.status_code == 500

    asyncio.run(_run())
