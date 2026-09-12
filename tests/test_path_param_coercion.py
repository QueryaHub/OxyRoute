from oxyroute import App
from oxyroute.testing import TestClient


def test_path_parameter_coercion():
    app = App()

    @app.get("/items/:val")
    def get_item(val: object):
        return {"val": val, "type": type(val).__name__}

    with TestClient(app) as client:
        # Exact integer
        resp = client.get("/items/42")
        assert resp.status_code == 200
        assert resp.json() == {"val": 42, "type": "int"}

        resp = client.get("/items/0")
        assert resp.status_code == 200
        assert resp.json() == {"val": 0, "type": "int"}

        resp = client.get("/items/-10")
        assert resp.status_code == 200
        assert resp.json() == {"val": -10, "type": "int"}

        # Leading zeros preserved as string
        resp = client.get("/items/0123")
        assert resp.status_code == 200
        assert resp.json() == {"val": "0123", "type": "str"}

        resp = client.get("/items/007")
        assert resp.status_code == 200
        assert resp.json() == {"val": "007", "type": "str"}

        resp = client.get("/items/00")
        assert resp.status_code == 200
        assert resp.json() == {"val": "00", "type": "str"}

        # Float
        resp = client.get("/items/3.14")
        assert resp.status_code == 200
        assert resp.json() == {"val": 3.14, "type": "float"}

        resp = client.get("/items/-0.5")
        assert resp.status_code == 200
        assert resp.json() == {"val": -0.5, "type": "float"}

        # Non-numeric / specials preserved as string
        resp = client.get("/items/nan")
        assert resp.status_code == 200
        assert resp.json() == {"val": "nan", "type": "str"}

        resp = client.get("/items/inf")
        assert resp.status_code == 200
        assert resp.json() == {"val": "inf", "type": "str"}

        resp = client.get("/items/+42")
        assert resp.status_code == 200
        assert resp.json() == {"val": "+42", "type": "str"}

        # Boolean
        resp = client.get("/items/true")
        assert resp.status_code == 200
        assert resp.json() == {"val": True, "type": "bool"}

        resp = client.get("/items/false")
        assert resp.status_code == 200
        assert resp.json() == {"val": False, "type": "bool"}
