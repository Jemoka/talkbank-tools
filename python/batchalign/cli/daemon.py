"""`daemon` command — serve `batchalign.api` in production.

Production posture (the defaults):

* No auto-reload. The watcher is dev-only and disables uvicorn's own
  signal handlers; never run it under a service manager.
* uvloop + httptools event loop / parser (uvicorn picks these up
  automatically when installed via the `[standard]` extra) for higher
  throughput than the pure-Python asyncio loop.
* Access logs to stdout so the surrounding service manager
  (systemd, supervisord, k8s) captures them; structured-JSON
  formatting is left to the supervisor — the daemon stays pid-1
  agnostic.
* Graceful shutdown: SIGTERM drains in-flight jobs up to
  ``--graceful-timeout`` seconds before the worker is killed.

Single-worker constraint
------------------------
The job registry in ``batchalign.api`` is an in-memory dict, so jobs
created by worker A are not visible to worker B. ``--workers`` is
therefore pinned to 1 by default and a warning is printed if the user
overrides it. A multi-worker deployment needs a shared registry
(Redis/Postgres) — that swap is a one-class change in ``api.py``
(``JobRegistry`` is a Protocol; see the plan file).

For horizontal scale today: front N single-worker daemons with a
sticky-session load balancer keyed on ``job_id``.
"""

from __future__ import annotations

import sys

import typer


def run_pyapp_entry() -> None:
    """PyApp entry point for the ``daemonapp`` standalone binary.

    The ``//python/batchalign:daemonapp`` Bazel target bundles batchalign
    + a Python runtime into a single executable via
    `PyApp <https://ofek.dev/pyapp/>`__. PyApp invokes whatever callable
    ``PYAPP_EXEC_SPEC`` names; this function is that callable.

    Behavior: prepend ``daemon`` to the argv the bundled binary received,
    then hand off to the regular Typer app. The shipped binary IS the
    daemon — users do not type ``daemonapp daemon ...``, just
    ``daemonapp --port 8765``.
    """
    from batchalign.cli import app

    sys.argv = [sys.argv[0], "daemon", *sys.argv[1:]]
    app()


def register(app: typer.Typer) -> None:
    @app.command()
    def daemon(
        host: str = typer.Option(
            "127.0.0.1",
            "--host",
            help="Bind address. Use 0.0.0.0 to accept off-host connections; "
                 "default loopback-only so an unsecured daemon can't be "
                 "exposed accidentally.",
        ),
        port: int = typer.Option(8765, "--port", help="Bind port."),
        workers: int = typer.Option(
            1,
            "--workers",
            help="Worker process count. Pinned to 1 by default — the in-memory "
                 "job registry is per-process. Overriding requires a shared "
                 "registry backend (not yet wired).",
        ),
        log_level: str = typer.Option(
            "info",
            "--log-level",
            help="uvicorn log level: critical/error/warning/info/debug/trace.",
        ),
        access_log: bool = typer.Option(
            True,
            "--access-log/--no-access-log",
            help="Emit HTTP access logs (one line per request).",
        ),
        proxy_headers: bool = typer.Option(
            True,
            "--proxy-headers/--no-proxy-headers",
            help="Trust X-Forwarded-* headers from the upstream reverse proxy. "
                 "Combine with --forwarded-allow-ips when fronted by nginx/ALB.",
        ),
        forwarded_allow_ips: str = typer.Option(
            "127.0.0.1",
            "--forwarded-allow-ips",
            help="Comma-separated upstream IPs trusted for X-Forwarded-*. "
                 "Use '*' only when the daemon is behind a trusted proxy.",
        ),
        graceful_timeout: int = typer.Option(
            30,
            "--graceful-timeout",
            help="Seconds to wait for in-flight requests to drain on SIGTERM.",
        ),
        dev: bool = typer.Option(
            False,
            "--dev",
            help="Development mode: enable --reload, drop production defaults. "
                 "Never use in production — disables clean shutdown handling.",
        ),
    ) -> None:
        """Start the batchalign HTTP daemon in production mode.

        The API surface is auto-generated from `batchalign.recipes` —
        hit ``GET /capabilities`` after startup to discover every
        recipe, backend, and endpoint the server exposes.

        Run under a service manager (systemd, supervisord, k8s) for
        process supervision; the daemon itself does not background or
        daemonize.
        """
        try:
            import uvicorn
        except ImportError as exc:  # pragma: no cover
            raise typer.BadParameter(
                "uvicorn is not installed. Install the API extra: "
                "`pip install 'batchalign[api]'`."
            ) from exc

        if dev:
            # Development path: reload, single worker, app loaded via
            # import-string so the reloader can re-import on change.
            uvicorn.run(
                "batchalign.api:app",
                host=host,
                port=port,
                reload=True,
                workers=1,
                log_level=log_level,
                access_log=access_log,
            )
            return

        # Production path.
        if workers != 1:
            typer.secho(
                f"WARNING: --workers={workers} with the default in-memory job "
                "registry will produce inconsistent /jobs/{id} responses "
                "(each worker has its own JOBS dict). Pin to --workers 1 or "
                "switch to a shared registry backend.",
                fg=typer.colors.YELLOW,
                err=True,
            )

        # Import-string form is required for multi-worker (uvicorn
        # forks; the workers re-import the module). It also lets us
        # avoid loading fastapi in the supervisor process when the
        # daemon is started under a service manager.
        config = uvicorn.Config(
            "batchalign.api:app",
            host=host,
            port=port,
            workers=workers,
            log_level=log_level,
            access_log=access_log,
            proxy_headers=proxy_headers,
            forwarded_allow_ips=forwarded_allow_ips,
            timeout_graceful_shutdown=graceful_timeout,
            # uvloop + httptools are auto-picked when installed via the
            # `[standard]` extra; "auto" lets uvicorn fall back to
            # pure-Python on platforms where they're unavailable
            # (Windows, free-threaded builds).
            loop="auto",
            http="auto",
            interface="asgi3",
        )
        server = uvicorn.Server(config)
        try:
            server.run()
        except KeyboardInterrupt:  # pragma: no cover
            sys.exit(0)
