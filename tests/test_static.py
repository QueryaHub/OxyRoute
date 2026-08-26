import os
from tempfile import TemporaryDirectory

from oxyroute import App, StaticFiles
from oxyroute.testing import TestClient


def test_static_files():
    with TemporaryDirectory() as tmpdir:
        with open(os.path.join(tmpdir, "test.txt"), "w") as f:
            f.write("hello world")

        app = App()
        app.mount("/static", StaticFiles(tmpdir))

        with TestClient(app) as client:
            resp = client.get("/static/test.txt")
            assert resp.status_code == 200
            assert resp.content == b"hello world"
            assert resp.headers["content-type"] == "text/plain"


def test_static_files_missing():
    with TemporaryDirectory() as tmpdir:
        app = App()
        app.mount("/static", StaticFiles(tmpdir))

        with TestClient(app) as client:
            resp = client.get("/static/missing.txt")
            assert resp.status_code == 404


def test_static_files_traversal():
    with TemporaryDirectory() as tmpdir:
        # Create a file outside the static directory
        with open(os.path.join(tmpdir, "secret.txt"), "w") as f:
            f.write("secret")

        static_dir = os.path.join(tmpdir, "static")
        os.makedirs(static_dir)

        app = App()
        app.mount("/static", StaticFiles(static_dir))

        with TestClient(app) as client:
            resp = client.get("/static/../secret.txt")
            assert resp.status_code in (403, 404)


def test_static_files_index_html():
    with TemporaryDirectory() as tmpdir:
        with open(os.path.join(tmpdir, "index.html"), "w") as f:
            f.write("<h1>Hello</h1>")

        app = App()
        app.mount("/static", StaticFiles(tmpdir, html=True))

        with TestClient(app) as client:
            resp = client.get("/static/")
            assert resp.status_code == 200
            assert resp.content == b"<h1>Hello</h1>"
            assert resp.headers["content-type"] == "text/html"

            resp = client.get("/static")
            assert resp.status_code == 200
            assert resp.content == b"<h1>Hello</h1>"


def test_static_files_symlink_escape():
    with TemporaryDirectory() as tmpdir:
        outside_file = os.path.join(tmpdir, "outside_secret.txt")
        with open(outside_file, "w") as f:
            f.write("top-secret-data")

        static_dir = os.path.join(tmpdir, "static")
        os.makedirs(static_dir)

        # Create a symlink inside static pointing outside
        symlink_path = os.path.join(static_dir, "symlink_secret.txt")
        os.symlink(outside_file, symlink_path)

        app = App()
        app.mount("/static", StaticFiles(static_dir))

        with TestClient(app) as client:
            resp = client.get("/static/symlink_secret.txt")
            assert resp.status_code == 403


def test_static_files_sibling_prefix_traversal():
    with TemporaryDirectory() as tmpdir:
        # Sibling directory starting with the same prefix name
        static_dir = os.path.join(tmpdir, "static")
        os.makedirs(static_dir)
        sibling_dir = os.path.join(tmpdir, "static_private")
        os.makedirs(sibling_dir)

        with open(os.path.join(sibling_dir, "passwords.txt"), "w") as f:
            f.write("private-passwords")

        app = App()
        app.mount("/static", StaticFiles(static_dir))

        with TestClient(app) as client:
            resp = client.get("/static/../static_private/passwords.txt")
            assert resp.status_code in (403, 404)

