from pathlib import Path

import oxyroute


def test_py_typed_exists():
    pkg_dir = Path(oxyroute.__file__).parent
    py_typed = pkg_dir / "py.typed"
    assert py_typed.exists(), "oxyroute/py.typed must exist for PEP 561 compliance"


def test_oxyroute_stubs_exist():
    pkg_dir = Path(oxyroute.__file__).parent
    stubs = pkg_dir / "_oxyroute.pyi"
    assert stubs.exists(), "oxyroute/_oxyroute.pyi must exist for IDE type hinting"
