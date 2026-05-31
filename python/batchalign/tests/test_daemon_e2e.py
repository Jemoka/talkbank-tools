"""Simple daemon end-to-end smoke tests (Landing 8 #35).

Spins the FastAPI app up via TestClient (in-process, no socket bind)
and exercises the core endpoints. No real audio / NLP work — these
guards the HTTP contract.

Per user direction 2026-05-31: simple pytest, not the Rust
tests/daemon_e2e.rs harness.
"""

from __future__ import annotations

import pytest

try:
    from fastapi.testclient import TestClient
    from batchalign.api import app
    _HAVE_API = True
except Exception as exc:
    _HAVE_API = False
    _IMPORT_ERR = exc


pytestmark = pytest.mark.skipif(not _HAVE_API, reason="API extras not installed")


@pytest.fixture(scope="module")
def client():
    with TestClient(app) as c:
        yield c


def test_health_returns_ok(client) -> None:
    response = client.get("/health")
    assert response.status_code == 200
    body = response.json()
    assert body["ok"] is True
    assert "recipes" in body
    assert "backend_kinds" in body


def test_health_includes_sha_header(client) -> None:
    response = client.get("/health")
    assert response.status_code == 200
    assert "X-Batchalign-SHA" in response.headers
    assert response.headers["X-Batchalign-SHA"]  # non-empty


def test_capabilities_endpoint(client) -> None:
    response = client.get("/capabilities")
    assert response.status_code == 200
    body = response.json()
    assert "recipes" in body or "backends" in body


def test_recipes_listing(client) -> None:
    response = client.get("/recipes")
    assert response.status_code == 200
    body = response.json()
    assert isinstance(body, dict) or isinstance(body, list)


def test_backends_listing(client) -> None:
    response = client.get("/backends")
    assert response.status_code == 200


def test_unknown_route_404(client) -> None:
    response = client.get("/this/does/not/exist")
    assert response.status_code == 404


def test_openapi_schema_present(client) -> None:
    response = client.get("/openapi.json")
    assert response.status_code == 200
    body = response.json()
    assert "openapi" in body
    assert "paths" in body
    assert "/health" in body["paths"]
    assert "/capabilities" in body["paths"]
