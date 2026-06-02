"""Drift-prevention tests for `batchalign.api`.

The whole point of `api.py` is that it introspects `recipes` and
`backends` rather than mirroring them. These tests are the safety net
that makes that contract enforceable — if `recipes.py` grows a new
function and the API doesn't pick it up, one of these asserts fires.

If you're here because a test failed: **fix `api.py`, not the test.**
"""

from __future__ import annotations

import inspect

import pytest

# These imports require fastapi / sse_starlette to be installed (the
# [api] extra). Skip the whole module gracefully when they aren't.
fastapi = pytest.importorskip("fastapi")
pytest.importorskip("sse_starlette")

from batchalign import backends as ba_backends
from batchalign import recipes as ba_recipes
from batchalign.api import (
    BACKEND_CLASSES,
    RECIPES,
    RECIPE_REQUEST_MODELS,
    _MARKER_ABCS,
    _is_backend_param,
    app,
)
from batchalign.backends.base import Backend


# --- Test 1: every public recipe has an endpoint -----------------------


def test_recipes_discovery_matches_dunder_all():
    public_recipes = {
        name
        for name in ba_recipes.__all__
        if inspect.isfunction(getattr(ba_recipes, name))
    }
    assert set(RECIPES) == public_recipes, (
        "RECIPES drift: api.py introspection diverged from recipes.__all__"
    )


def test_every_recipe_has_a_post_route():
    paths = {r.path for r in app.router.routes}
    for name in RECIPES:
        assert f"/recipes/{name}" in paths, f"no POST /recipes/{name} route"


# --- Test 2: every recipe param has a request-model field --------------


def test_recipe_request_models_cover_every_param():
    for name, fn in RECIPES.items():
        Model = RECIPE_REQUEST_MODELS[name]
        sig = inspect.signature(fn)
        for pname, p in sig.parameters.items():
            if p.kind is inspect.Parameter.VAR_KEYWORD:
                # `**opts` is exposed via the catch-all `pipeline_opts`.
                assert "pipeline_opts" in Model.model_fields, (
                    f"{name}: **opts not surfaced as pipeline_opts"
                )
                continue
            assert pname in Model.model_fields, (
                f"{name}: param {pname!r} missing from request model"
            )
        assert "inputs" in Model.model_fields, f"{name}: no inputs field"


# --- Test 3: backend discovery covers every exported concrete class ----


def test_backend_classes_cover_batchalign_backends_all():
    expected = set()
    for n in ba_backends.__all__:
        obj = getattr(ba_backends, n, None)
        if (
            inspect.isclass(obj)
            and issubclass(obj, Backend)
            and obj not in _MARKER_ABCS
        ):
            expected.add(n)
    assert set(BACKEND_CLASSES) == expected, (
        "BACKEND_CLASSES drift: api.py introspection diverged from "
        "batchalign.backends.__all__"
    )


# --- Test 4: backend kwargs surface accurately -------------------------


def test_backend_capability_lists_constructor_kwargs():
    from batchalign.api import _backend_capability

    for name, cls in BACKEND_CLASSES.items():
        cap = _backend_capability(name, cls)
        sig = inspect.signature(cls.__init__)
        expected_kwargs = [n for n in sig.parameters if n != "self"]
        listed = [k["name"] for k in cap["kwargs"]]
        assert listed == expected_kwargs, (
            f"{name}: kwargs drift {listed} != {expected_kwargs}"
        )


# --- Test 5: OpenAPI schema generates ----------------------------------


def test_openapi_schema_generates():
    schema = app.openapi()
    paths = schema["paths"]
    for name in RECIPES:
        assert f"/recipes/{name}" in paths
    for canonical in ("/uploads", "/jobs/{job_id}", "/capabilities"):
        assert canonical in paths


# --- Test 6: capabilities endpoint is self-consistent ------------------


def test_capabilities_endpoint_round_trips():
    # `fastapi.testclient` pulls in `starlette.testclient`, which now
    # requires the `httpx2` package; skip cleanly when it isn't installed.
    pytest.importorskip("httpx2")
    from fastapi.testclient import TestClient

    with TestClient(app) as client:
        resp = client.get("/capabilities")
        assert resp.status_code == 200
        body = resp.json()
        assert set(body["recipes"]) == set(RECIPES)
        assert set(body["backends"]) == set(BACKEND_CLASSES)
        # Every backend listed under a task marker must declare that task.
        for task_name, kinds in body["backends_by_task"].items():
            for kind in kinds:
                assert task_name in body["backends"][kind]["tasks"], (
                    f"{kind} listed under {task_name} but doesn't declare it"
                )


# --- Helper-level sanity ----------------------------------------------


def test_backend_param_predicate_recognises_convention():
    # Recipe params named *_backend are backend slots by convention,
    # even when annotated `Any`. The recipes module relies on this.
    for name, fn in RECIPES.items():
        sig = inspect.signature(fn)
        for pname, p in sig.parameters.items():
            if pname.endswith("_backend"):
                assert _is_backend_param(p), (
                    f"{name}.{pname}: convention-based detection failed"
                )
