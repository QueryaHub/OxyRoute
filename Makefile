# Local full check in one command — same intent as .github/workflows/ci.yml
# Needs: https://github.com/astral-sh/uv, Rust (rustfmt, clippy), C toolchain (PyO3)
#
# All installs go into  ./.venv  (maturin develop --uv, uv run). Do NOT point package installs
# at /usr/bin/python3 on Debian/Ubuntu (PEP 668 “externally managed”).
#
# To choose which *base* interpreter  uv venv  uses when creating  .venv  (only if missing):
#   make test PY_BOOTSTRAP=/usr/bin/python3.12
#   rm -rf .venv  &&  make test PY_BOOTSTRAP=...  —  recreate venv with another base

SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c

UV ?= uv
ROOT := $(abspath .)
export UV_PROJECT := $(ROOT)

# Used only to *create* .venv; package installs use .venv, never the base interpreter.
PY_BOOTSTRAP ?= $(shell command -v python3)
VENV_PY := $(ROOT)/.venv/bin/python
export UV_PYTHON := $(VENV_PY)

# Dev speed:  make test MATURIN_FLAGS=     |  same as CI release build:  (default) --release
MATURIN_FLAGS ?= --release
RUFF_PATHS := oxyroute tests examples

.PHONY: help all test lint fix sync develop wheel install pytest build \
	_need-uv _need-python-bootstrap _ensure-venv

help:
	@echo "make test  —  venv + uv sync + ruff (check) + ruff format --check + rustfmt (check) + clippy +"
	@echo "             maturin develop --uv  +  pytest  (isolated temp dir, like CI)"
	@echo "make lint  —  venv + sync + Python/Rust checks only  (no maturin, no tests)"
	@echo "make fix   —  venv + sync + ruff format (write) + cargo fmt  (not in make test)"
	@echo "make build / develop  —  venv + sync +  maturin develop --uv  (no linters, no tests)"
	@echo "make wheel —  venv + build  target/wheels/*.whl  (packaging; no install to venv)"
	@echo "  MATURIN_FLAGS=  —  debug build  (drop --release )"
	@echo "  PY_BOOTSTRAP=   —  base  python3  to create  .venv  (default:  which python3 )"

all: test

test: _need-uv _need-python-bootstrap _ensure-venv
	$(UV) sync --frozen --extra dev
	$(UV) run ruff check $(RUFF_PATHS)
	$(UV) run ruff format --check $(RUFF_PATHS)
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings
	$(UV) run maturin develop --uv $(MATURIN_FLAGS)
	@_d=$$(mktemp -d); trap 'rm -rf "$$_d"' EXIT; \
		(cd "$$_d" && UV_PROJECT="$(ROOT)" $(UV) run python -m pytest "$(ROOT)/tests" -v)
	@echo OK

lint: _need-uv _need-python-bootstrap _ensure-venv
	$(UV) sync --frozen --extra dev
	$(UV) run ruff check $(RUFF_PATHS)
	$(UV) run ruff format --check $(RUFF_PATHS)
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings

fix: _need-uv _need-python-bootstrap _ensure-venv
	$(UV) sync --frozen --extra dev
	$(UV) run ruff format $(RUFF_PATHS)
	cargo fmt --all

sync: _need-uv _need-python-bootstrap _ensure-venv
	$(UV) sync --frozen --extra dev

develop: _need-uv sync
	$(UV) run maturin develop --uv $(MATURIN_FLAGS)

# Release wheel in target/wheels/ (as in CI “build” job); does not install to venv
wheel: _need-uv sync
	rm -f target/wheels/oxyroute-*.whl
	$(UV) run maturin build $(MATURIN_FLAGS)

# Historical alias: same as  develop
build: develop
install: develop

pytest: _need-uv _need-python-bootstrap _ensure-venv
	$(UV) sync --frozen --extra dev
	@_d=$$(mktemp -d); trap 'rm -rf "$$_d"' EXIT; \
		(cd "$$_d" && UV_PROJECT="$(ROOT)" $(UV) run python -m pytest "$(ROOT)/tests" -v)

_need-uv:
	@command -v $(UV) >/dev/null 2>&1 || { \
		echo "error: '$(UV)' not on PATH. Install: https://docs.astral.sh/uv/"; \
		exit 1; \
	}

_need-python-bootstrap:
	@test -n "$(PY_BOOTSTRAP)" || { \
		echo "error: no python3 on PATH (set PY_BOOTSTRAP=/path/to/python)" >&2; \
		exit 1; \
	}

# Create  .venv  if missing. First run must not set  UV_PYTHON  to a non-existent  .venv  binary
# or  uv venv  can get confused; clear it for this one line only.
_ensure-venv: _need-uv _need-python-bootstrap
	@if [ -x "$(VENV_PY)" ]; then exit 0; fi
	@env -u UV_PYTHON $(UV) venv --python "$(PY_BOOTSTRAP)" "$(ROOT)/.venv"
