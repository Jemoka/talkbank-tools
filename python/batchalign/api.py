"""Thin FastAPI wrapper over `batchalign.recipes` — zero manual mirroring.

This module is the HTTP face of the batchalign Python package. Its
overriding design goal is that `batchalign.recipes` stays the source of
truth: adding a recipe, renaming a parameter, or adding a backend class
must surface in the API automatically. There is no per-recipe handler
function, no per-backend Pydantic model written by hand.

Three reflection passes wire everything at import time:

1. ``RECIPES``        — every public function in `batchalign.recipes`.
2. ``BACKEND_CLASSES`` — every concrete `Backend` subclass exported by
   `batchalign.backends`. The marker ABCs (`ASR`, `FA`, ...) are
   excluded.
3. ``InputSpec``       — uniform input shape (`media` / `chat` / `paired`),
   materialized via the existing `batchalign.inputs` helpers.

Drift between these passes and the source modules is caught by
`tests/test_api_introspection.py`. If the asserts there fail, the API
has fallen out of sync with `recipes.py` — fix the API, not the test.

The runtime model is async jobs + SSE for `progress_v2` events, matching
the time-transparency rule in CLAUDE.md §11. `Pipeline.run` is blocking
Rust, so jobs run in `asyncio.to_thread`; the Rust callback bridges
events into a per-job `asyncio.Queue` drained by the SSE handler.

Optional dependencies (`fastapi`, `sse_starlette`, `python-multipart`,
`httpx`) live under the `[api]` extra in `pyproject.toml`. Importing
this module without them raises a clear `ImportError` at import time.
"""

from __future__ import annotations

import asyncio
import inspect
import os
import shutil
import time
import traceback
import uuid
from dataclasses import asdict, dataclass, field, is_dataclass
from enum import Enum
from pathlib import Path
from typing import Any, Callable, Iterator, Literal

# Hard imports — api.py is opt-in via the [api] extra. We don't try to
# stub these out; a missing dep should fail loudly with a single line.
from fastapi import FastAPI, HTTPException, UploadFile, File
from pydantic import BaseModel, ConfigDict, Field, create_model
from sse_starlette.sse import EventSourceResponse

from batchalign import backends as ba_backends
from batchalign import inputs as ba_inputs
from batchalign import recipes as ba_recipes
from batchalign.backends.base import Backend


# ---------------------------------------------------------------------------
# Workdir for uploads. Cleaned per-job; the parent directory persists.
# ---------------------------------------------------------------------------

def _default_workdir() -> Path:
    """Pick a writable workdir. Honors ``BATCHALIGN_API_WORKDIR``; falls
    back to the user cache; falls back to a tmpdir if both are denied
    (Bazel sandbox, locked-down containers)."""
    env = os.environ.get("BATCHALIGN_API_WORKDIR")
    candidates: list[Path] = []
    if env:
        candidates.append(Path(env).absolute())
    candidates.append((Path.home() / ".cache" / "batchalign-api").absolute())
    import tempfile

    candidates.append(Path(tempfile.gettempdir()) / "batchalign-api")
    for c in candidates:
        try:
            uploads = c / "uploads"
            uploads.mkdir(parents=True, exist_ok=True)
            # mkdir succeeds when the directory already exists, even if
            # the current process can't write into it (e.g. Bazel
            # sandbox over a host dir from a prior unsandboxed run).
            # Probe with a real touch to catch that.
            probe = uploads / f".probe-{os.getpid()}"
            probe.write_bytes(b"")
            probe.unlink()
            return c
        except (OSError, PermissionError):
            continue
    # Last resort — a unique tmpdir always works.
    return Path(tempfile.mkdtemp(prefix="batchalign-api-"))


_WORKDIR = _default_workdir()


# ---------------------------------------------------------------------------
# Layer 2: backend discriminator (built first because recipe models
# reference `BackendSpec`).
# ---------------------------------------------------------------------------

_MARKER_ABCS = {
    ba_backends.ASR,
    ba_backends.FA,
    ba_backends.Speaker,
    ba_backends.UtSeg,
    ba_backends.Morphosyntax,
    ba_backends.Translate,
    ba_backends.Coref,
    ba_backends.OpenSmile,
    ba_backends.AVQI,
    Backend,
}


def _discover_backend_classes() -> dict[str, type[Backend]]:
    """Concrete `Backend` subclasses exported by `batchalign.backends`.

    Excludes the marker ABCs (which are themselves `Backend` subclasses
    but have no constructor surface a client could fill in).
    """
    found: dict[str, type[Backend]] = {}
    for name in ba_backends.__all__:
        obj = getattr(ba_backends, name, None)
        if (
            inspect.isclass(obj)
            and issubclass(obj, Backend)
            and obj not in _MARKER_ABCS
        ):
            found[name] = obj
    return found


BACKEND_CLASSES: dict[str, type[Backend]] = _discover_backend_classes()


class BackendSpec(BaseModel):
    """Discriminated JSON for a backend.

    The set of valid ``kind`` strings is the set of names in
    :data:`BACKEND_CLASSES`. ``kwargs`` is forwarded straight to the
    constructor; we validate it against ``inspect.signature`` at
    materialization (not at request-parse) so nested backend kwargs
    (e.g. ``stanza_fallback``) can themselves be ``BackendSpec``.
    """

    model_config = ConfigDict(extra="forbid")
    kind: str
    kwargs: dict[str, Any] = Field(default_factory=dict)


def _is_backend_spec_dict(value: Any) -> bool:
    return isinstance(value, dict) and "kind" in value


def build_backend(spec: BackendSpec | dict[str, Any]) -> Backend:
    """Materialize a `BackendSpec` into a real backend instance.

    Recurses into nested kwargs whose values look like backend specs
    (have a ``kind`` field) — that handles things like
    ``UtSegBackend(stanza_fallback=StanzaBackend(...))``.
    """
    data = spec.model_dump() if isinstance(spec, BackendSpec) else spec
    kind = data["kind"]
    cls = BACKEND_CLASSES.get(kind)
    if cls is None:
        raise HTTPException(
            status_code=400,
            detail=f"Unknown backend kind {kind!r}. Known: {sorted(BACKEND_CLASSES)}",
        )

    raw_kwargs = dict(data.get("kwargs") or {})
    resolved: dict[str, Any] = {}
    for k, v in raw_kwargs.items():
        if _is_backend_spec_dict(v):
            resolved[k] = build_backend(v)
        else:
            resolved[k] = v

    sig = inspect.signature(cls.__init__)
    try:
        # bind_partial validates names without requiring every parameter.
        # We drop `self` since `cls(...)` supplies it.
        params = {
            name: param
            for name, param in sig.parameters.items()
            if name != "self"
        }
        # Reject unknown kwargs explicitly so the client gets a helpful
        # error rather than a TypeError from deep inside Python.
        unknown = set(resolved) - set(params)
        if unknown:
            raise HTTPException(
                status_code=400,
                detail=f"Backend {kind} got unexpected kwargs: {sorted(unknown)}",
            )
        return cls(**resolved)
    except HTTPException:
        raise
    except TypeError as exc:
        raise HTTPException(
            status_code=400,
            detail=f"Constructing {kind}: {exc}",
        ) from exc


# ---------------------------------------------------------------------------
# Layer 3: input materialization.
# ---------------------------------------------------------------------------

InputKind = Literal["media", "chat", "paired"]


class InputSpec(BaseModel):
    """Uniform input descriptor.

    Exactly one of ``upload_id`` / ``url`` / ``path`` must be set for the
    primary slot. ``paired`` additionally needs a gold counterpart via
    ``gold_upload_id`` / ``gold_url`` / ``gold_path``.

    ``path`` is honored only when the server is configured to trust local
    paths (off by default — set ``BATCHALIGN_API_ALLOW_PATHS=1``).
    """

    model_config = ConfigDict(extra="forbid")
    kind: InputKind = "media"
    upload_id: str | None = None
    url: str | None = None
    path: str | None = None
    gold_upload_id: str | None = None
    gold_url: str | None = None
    gold_path: str | None = None
    source_id: str | None = None


def _allow_paths() -> bool:
    return os.environ.get("BATCHALIGN_API_ALLOW_PATHS", "") == "1"


def _resolve_one(
    spec: InputSpec,
    *,
    use_gold: bool = False,
    workdir: Path,
) -> Path:
    """Resolve a single slot of an `InputSpec` to a filesystem path."""
    upload = spec.gold_upload_id if use_gold else spec.upload_id
    url = spec.gold_url if use_gold else spec.url
    path = spec.gold_path if use_gold else spec.path

    set_count = sum(x is not None for x in (upload, url, path))
    if set_count != 1:
        raise HTTPException(
            status_code=400,
            detail=("Need exactly one of upload_id/url/path "
                    f"(gold={use_gold}); got {set_count}"),
        )

    if upload is not None:
        resolved = _WORKDIR / "uploads" / upload
        if not resolved.is_file():
            raise HTTPException(status_code=404, detail=f"upload_id not found: {upload}")
        return resolved

    if url is not None:
        # Lazy import — httpx is part of the [api] extra but we'd rather
        # not load it when only uploads are in play.
        import httpx

        suffix = Path(url.split("?", 1)[0]).suffix or ".bin"
        target = workdir / f"fetched-{uuid.uuid4().hex}{suffix}"
        with httpx.stream("GET", url, follow_redirects=True, timeout=300.0) as resp:
            resp.raise_for_status()
            with target.open("wb") as fp:
                for chunk in resp.iter_bytes():
                    fp.write(chunk)
        return target

    # path branch
    assert path is not None
    if not _allow_paths():
        raise HTTPException(
            status_code=403,
            detail="Server paths are disabled. Set BATCHALIGN_API_ALLOW_PATHS=1.",
        )
    resolved = Path(path).absolute()
    if not resolved.is_file():
        raise HTTPException(status_code=404, detail=f"path not found: {resolved}")
    return resolved


def materialize_input(spec: InputSpec, *, workdir: Path) -> Any:
    """Turn an `InputSpec` into the matching `MediaInput` / `ChatInput`
    / `PairedInput` using the existing `batchalign.inputs` helpers."""
    primary = _resolve_one(spec, use_gold=False, workdir=workdir)
    if spec.kind == "media":
        return ba_inputs.media_from_path(primary, source_id=spec.source_id)
    if spec.kind == "chat":
        return ba_inputs.chat_from_path(primary, source_id=spec.source_id)
    if spec.kind == "paired":
        gold = _resolve_one(spec, use_gold=True, workdir=workdir)
        return ba_inputs.paired_from_paths(primary, gold, source_id=spec.source_id)
    raise HTTPException(status_code=400, detail=f"Unknown input kind: {spec.kind!r}")


# ---------------------------------------------------------------------------
# Layer 1: recipe registry.
# ---------------------------------------------------------------------------


def _discover_recipes() -> dict[str, Callable[..., Any]]:
    out: dict[str, Callable[..., Any]] = {}
    for name in getattr(ba_recipes, "__all__", []):
        obj = getattr(ba_recipes, name, None)
        if inspect.isfunction(obj):
            out[name] = obj
    return out


RECIPES: dict[str, Callable[..., Any]] = _discover_recipes()


def _is_backend_param(param: inspect.Parameter) -> bool:
    """Recipe params that should be filled by a `BackendSpec`.

    A param qualifies if (a) its annotation is a subclass of `Backend`,
    or (b) its name ends with ``_backend`` (the convention used by every
    current recipe — even when the annotation is `Any`).
    """
    if param.name.endswith("_backend"):
        return True
    ann = param.annotation
    try:
        return inspect.isclass(ann) and issubclass(ann, Backend)
    except TypeError:
        return False


def _build_recipe_request_model(name: str, fn: Callable[..., Any]) -> type[BaseModel]:
    """Generate a Pydantic request model from a recipe's signature."""
    sig = inspect.signature(fn)
    fields: dict[str, Any] = {}
    for pname, param in sig.parameters.items():
        if param.kind is inspect.Parameter.VAR_KEYWORD:
            # `**opts` -> forwarded to Pipeline(...). We surface it as a
            # passthrough dict rather than mirror Pipeline's surface.
            fields["pipeline_opts"] = (
                dict[str, Any] | None,
                Field(default=None, description="Forwarded to Pipeline(...)."),
            )
            continue
        if _is_backend_param(param):
            optional = param.default is None or param.default is inspect.Parameter.empty and False
            if param.default is None or param.default is inspect.Parameter.empty:
                # Required iff there's no default. (`Parameter.empty`
                # means truly required; `None` means optional.)
                if param.default is inspect.Parameter.empty:
                    fields[pname] = (BackendSpec, Field(...))
                else:
                    fields[pname] = (BackendSpec | None, Field(default=None))
            else:
                fields[pname] = (BackendSpec | None, Field(default=None))
        else:
            # Non-backend recipe params (none today — but future-proof).
            default = (... if param.default is inspect.Parameter.empty else param.default)
            fields[pname] = (Any, Field(default=default))

    fields["inputs"] = (
        list[InputSpec],
        Field(..., description="One or more inputs to process."),
    )

    model = create_model(  # type: ignore[call-overload]
        f"{name.capitalize()}Request",
        __base__=BaseModel,
        **fields,
    )
    model.model_config = ConfigDict(extra="forbid")
    return model


RECIPE_REQUEST_MODELS: dict[str, type[BaseModel]] = {
    name: _build_recipe_request_model(name, fn) for name, fn in RECIPES.items()
}


# ---------------------------------------------------------------------------
# Job registry + SSE bridge.
# ---------------------------------------------------------------------------


class JobState(str, Enum):
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class Job:
    id: str
    recipe: str
    state: JobState = JobState.PENDING
    created_at: float = field(default_factory=time.time)
    started_at: float | None = None
    finished_at: float | None = None
    progress: float = 0.0
    error: str | None = None
    result: Any = None
    workdir: Path | None = None
    # asyncio.Queue is loop-bound — we create it on the event loop the
    # API is running on. The job thread pushes via run_coroutine_threadsafe.
    events: asyncio.Queue[dict[str, Any] | None] = field(
        default_factory=lambda: asyncio.Queue()  # type: ignore[arg-type]
    )


JOBS: dict[str, Job] = {}


def _event_to_dict(event: Any) -> dict[str, Any]:
    """Serialize a `ProgressEvent` (PyO3 object) for SSE.

    The Rust class doesn't have a `__dict__`; we read the documented
    fields by name. Falls back to `repr` for anything unexpected.
    """
    if is_dataclass(event):
        return asdict(event)
    payload: dict[str, Any] = {}
    for attr in ("source_id", "kind", "task", "completed", "total", "label"):
        if hasattr(event, attr):
            val = getattr(event, attr)
            if isinstance(val, Enum):
                val = val.value
            elif val is None or isinstance(val, (str, int, float, bool)):
                pass
            else:
                val = str(val)
            payload[attr] = val
    if not payload:
        payload["repr"] = repr(event)
    return payload


def _make_callback(
    job: Job, loop: asyncio.AbstractEventLoop
) -> Callable[[Any], None]:
    """Build the per-source callback handed to `pipeline.run`."""

    def cb(event: Any) -> None:
        data = _event_to_dict(event)
        # Update coarse progress when the event carries completed/total.
        total = data.get("total") or 0
        completed = data.get("completed") or 0
        if total:
            job.progress = max(job.progress, completed / total)
        asyncio.run_coroutine_threadsafe(job.events.put(data), loop)

    return cb


def _run_job_blocking(
    job: Job,
    recipe_fn: Callable[..., Any],
    recipe_kwargs: dict[str, Any],
    pipeline_opts: dict[str, Any],
    inputs_resolved: list[Any],
    loop: asyncio.AbstractEventLoop,
) -> None:
    """Body that runs in `asyncio.to_thread`. Owns the Rust call."""
    try:
        pipeline = recipe_fn(**recipe_kwargs, **pipeline_opts)
        cb = _make_callback(job, loop)
        callbacks = {
            str(getattr(inp, "source_id", "") or ""): cb for inp in inputs_resolved
        }
        outcomes = pipeline.run(inputs_resolved, callbacks=callbacks)
        # Outcomes are Rust BAValue objects; stringify for JSON.
        job.result = [_event_to_dict(o) if hasattr(o, "source_id") else repr(o)
                      for o in outcomes]
        job.state = JobState.COMPLETED
    except Exception as exc:  # noqa: BLE001
        job.error = f"{type(exc).__name__}: {exc}\n{traceback.format_exc()}"
        job.state = JobState.FAILED
    finally:
        job.finished_at = time.time()
        # Sentinel so the SSE drain loop knows to stop.
        asyncio.run_coroutine_threadsafe(job.events.put(None), loop)
        if job.workdir and job.workdir.exists():
            # Best-effort cleanup of fetched URL files. Uploads live
            # under the shared uploads dir and are *not* removed here.
            shutil.rmtree(job.workdir, ignore_errors=True)


# ---------------------------------------------------------------------------
# FastAPI app.
# ---------------------------------------------------------------------------


app = FastAPI(
    title="batchalign HTTP API",
    description=(
        "Thin wrapper over `batchalign.recipes`. Endpoints under "
        "`/recipes/*` are auto-generated from `recipes.__all__`; the "
        "set of valid backend `kind` strings is auto-discovered from "
        "`batchalign.backends`."
    ),
    version="0.1.0",
)


@app.get("/health")
def health() -> dict[str, Any]:
    return {
        "ok": True,
        "recipes": sorted(RECIPES),
        "backend_kinds": sorted(BACKEND_CLASSES),
    }


def _param_spec(p: inspect.Parameter) -> dict[str, Any]:
    return {
        "name": p.name,
        "required": p.default is inspect.Parameter.empty,
        "default": (
            None if p.default is inspect.Parameter.empty else repr(p.default)
        ),
        "annotation": (
            repr(p.annotation)
            if p.annotation is not inspect.Parameter.empty
            else None
        ),
    }


def _recipe_capability(name: str, fn: Callable[..., Any]) -> dict[str, Any]:
    sig = inspect.signature(fn)
    params: list[dict[str, Any]] = []
    has_pipeline_opts = False
    for pname, p in sig.parameters.items():
        if p.kind is inspect.Parameter.VAR_KEYWORD:
            has_pipeline_opts = True
            continue
        spec = _param_spec(p)
        spec["is_backend"] = _is_backend_param(p)
        params.append(spec)
    return {
        "doc": inspect.getdoc(fn),
        "params": params,
        "accepts_pipeline_opts": has_pipeline_opts,
        "endpoint": f"/recipes/{name}",
        "request_schema_ref": f"#/components/schemas/{RECIPE_REQUEST_MODELS[name].__name__}",
    }


def _backend_capability(name: str, cls: type[Backend]) -> dict[str, Any]:
    sig = inspect.signature(cls.__init__)
    kwargs = [_param_spec(p) for n, p in sig.parameters.items() if n != "self"]
    tasks = [
        t.__name__
        for t in cls.__mro__
        if t in _MARKER_ABCS and t is not Backend
    ]
    return {
        "doc": inspect.getdoc(cls),
        "tasks": tasks,
        "kwargs": kwargs,
        "module": cls.__module__,
    }


@app.get("/capabilities")
def capabilities() -> dict[str, Any]:
    """One-stop discovery: every recipe, every backend, input shape, job lifecycle.

    Clients should hit this to learn what the server can do without
    reading the OpenAPI schema. The contents are derived entirely from
    introspection — adding a recipe or backend in the source updates
    this response automatically.
    """
    return {
        "api_version": app.version,
        "recipes": {n: _recipe_capability(n, fn) for n, fn in RECIPES.items()},
        "backends": {n: _backend_capability(n, cls) for n, cls in BACKEND_CLASSES.items()},
        "backends_by_task": _backends_by_task(),
        "input_kinds": list(InputSpec.model_fields["kind"].annotation.__args__),  # type: ignore[attr-defined]
        "job_states": [s.value for s in JobState],
        "endpoints": {
            "upload": {"method": "POST", "path": "/uploads"},
            "run_recipe": {"method": "POST", "path": "/recipes/{name}"},
            "job_status": {"method": "GET", "path": "/jobs/{id}"},
            "job_events_sse": {"method": "GET", "path": "/jobs/{id}/events"},
            "job_result": {"method": "GET", "path": "/jobs/{id}/result"},
            "job_cancel": {"method": "DELETE", "path": "/jobs/{id}"},
        },
        "config": {
            "workdir": str(_WORKDIR),
            "allow_server_paths": _allow_paths(),
        },
    }


def _backends_by_task() -> dict[str, list[str]]:
    """Inverted index: task name -> backend kinds that service it."""
    out: dict[str, list[str]] = {}
    for marker in _MARKER_ABCS:
        if marker is Backend:
            continue
        out[marker.__name__] = sorted(
            n for n, cls in BACKEND_CLASSES.items() if issubclass(cls, marker)
        )
    return out


@app.get("/recipes")
def list_recipes() -> dict[str, Any]:
    return {
        name: {
            "doc": inspect.getdoc(fn),
            "params": [
                pname
                for pname, p in inspect.signature(fn).parameters.items()
                if p.kind is not inspect.Parameter.VAR_KEYWORD
            ],
        }
        for name, fn in RECIPES.items()
    }


@app.get("/backends")
def list_backends() -> dict[str, Any]:
    out: dict[str, Any] = {}
    for name, cls in BACKEND_CLASSES.items():
        sig = inspect.signature(cls.__init__)
        params: list[dict[str, Any]] = []
        for pname, p in sig.parameters.items():
            if pname == "self":
                continue
            params.append({
                "name": pname,
                "required": p.default is inspect.Parameter.empty,
                "default": None if p.default is inspect.Parameter.empty else repr(p.default),
                "annotation": (
                    repr(p.annotation)
                    if p.annotation is not inspect.Parameter.empty
                    else None
                ),
            })
        out[name] = {
            "doc": inspect.getdoc(cls),
            "tasks": [t.__name__ for t in cls.__mro__
                      if t in _MARKER_ABCS and t is not Backend],
            "kwargs": params,
        }
    return out


@app.post("/uploads")
async def upload(file: UploadFile = File(...)) -> dict[str, str]:
    upload_id = uuid.uuid4().hex
    suffix = Path(file.filename or "").suffix
    target = _WORKDIR / "uploads" / f"{upload_id}{suffix}"
    with target.open("wb") as fp:
        while chunk := await file.read(1 << 20):
            fp.write(chunk)
    return {"upload_id": target.name}


@app.delete("/uploads/{upload_id}")
def delete_upload(upload_id: str) -> dict[str, bool]:
    target = _WORKDIR / "uploads" / upload_id
    if target.is_file():
        target.unlink()
        return {"deleted": True}
    raise HTTPException(status_code=404, detail=f"upload_id not found: {upload_id}")


# ---- recipe endpoints (one POST per recipe, registered in a loop) ---------


def _register_recipe_route(name: str, fn: Callable[..., Any]) -> None:
    Model = RECIPE_REQUEST_MODELS[name]

    # No @app.post decorator here — see the manual registration below
    # which sets __annotations__ on _handler before FastAPI sees it.
    async def _handler(req) -> dict[str, str]:  # noqa: ANN001 — annotation set below
        data = req.model_dump()
        pipeline_opts = data.pop("pipeline_opts", None) or {}
        inputs_specs = [InputSpec.model_validate(i) for i in data.pop("inputs")]

        recipe_kwargs: dict[str, Any] = {}
        for key, value in data.items():
            if value is None:
                continue
            if _is_backend_spec_dict(value):
                recipe_kwargs[key] = build_backend(value)
            else:
                recipe_kwargs[key] = value

        job_id = uuid.uuid4().hex
        job_workdir = _WORKDIR / "jobs" / job_id
        job_workdir.mkdir(parents=True, exist_ok=True)
        inputs_resolved = [
            materialize_input(s, workdir=job_workdir) for s in inputs_specs
        ]

        job = Job(id=job_id, recipe=name, workdir=job_workdir)
        JOBS[job_id] = job
        job.state = JobState.RUNNING
        job.started_at = time.time()

        # Resolve the recipe through the registry at call time so test
        # monkeypatches of RECIPES[name] take effect, and so that any
        # future hot-swap (e.g. a debug subclass) lands without
        # re-registering routes.
        current_fn = RECIPES.get(name, fn)

        loop = asyncio.get_running_loop()
        asyncio.create_task(
            asyncio.to_thread(
                _run_job_blocking,
                job, current_fn, recipe_kwargs, pipeline_opts, inputs_resolved, loop,
            )
        )
        return {"job_id": job_id}

    _handler.__doc__ = inspect.getdoc(fn) or f"Run recipe {name}."
    # FastAPI introspects __annotations__ to decide body vs query.
    # The handler is defined inside a closure, so we set the annotation
    # explicitly *after* defining the function — this lets FastAPI see
    # the dynamically-generated Pydantic model as the body type.
    _handler.__annotations__ = {"req": Model, "return": dict[str, str]}
    app.post(f"/recipes/{name}", name=f"run_{name}")(_handler)


for _name, _fn in RECIPES.items():
    _register_recipe_route(_name, _fn)


# ---- job endpoints --------------------------------------------------------


def _job_or_404(job_id: str) -> Job:
    job = JOBS.get(job_id)
    if job is None:
        raise HTTPException(status_code=404, detail=f"job not found: {job_id}")
    return job


@app.get("/jobs/{job_id}")
def get_job(job_id: str) -> dict[str, Any]:
    job = _job_or_404(job_id)
    return {
        "id": job.id,
        "recipe": job.recipe,
        "state": job.state.value,
        "progress": job.progress,
        "created_at": job.created_at,
        "started_at": job.started_at,
        "finished_at": job.finished_at,
        "error": job.error,
    }


@app.get("/jobs/{job_id}/events")
async def stream_events(job_id: str) -> EventSourceResponse:
    job = _job_or_404(job_id)

    async def gen() -> Iterator[dict[str, Any]]:  # type: ignore[misc]
        while True:
            item = await job.events.get()
            if item is None:
                # Final state marker so clients can close cleanly.
                yield {"event": "done", "data": job.state.value}
                return
            yield {"event": "progress", "data": _json_dump(item)}

    return EventSourceResponse(gen())


@app.get("/jobs/{job_id}/result")
def get_result(job_id: str) -> Any:
    job = _job_or_404(job_id)
    if job.state in (JobState.PENDING, JobState.RUNNING):
        raise HTTPException(status_code=409, detail=f"job not finished: {job.state.value}")
    if job.state == JobState.FAILED:
        raise HTTPException(status_code=500, detail=job.error)
    return {"job_id": job.id, "outcomes": job.result}


@app.delete("/jobs/{job_id}")
def cancel_job(job_id: str) -> dict[str, bool]:
    job = _job_or_404(job_id)
    if job.state in (JobState.PENDING, JobState.RUNNING):
        # We can't yank a thread out of native Rust code. Mark as
        # cancelled; the result will still arrive (and be discarded).
        job.state = JobState.CANCELLED
    if job.workdir and job.workdir.exists():
        shutil.rmtree(job.workdir, ignore_errors=True)
    return {"cancelled": True}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _json_dump(obj: Any) -> str:
    """SSE data must be a string. Use stdlib json (no extra deps)."""
    import json

    return json.dumps(obj, default=str)


__all__ = [
    "app",
    "RECIPES",
    "BACKEND_CLASSES",
    "RECIPE_REQUEST_MODELS",
    "BackendSpec",
    "InputSpec",
    "Job",
    "JobState",
    "JOBS",
    "build_backend",
    "materialize_input",
]
