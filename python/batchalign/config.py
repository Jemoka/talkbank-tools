"""Configuration loader for batchalign.

We read ``~/.batchalign.ini`` in **BA2-compatible** form so users
migrating from BA2 keep their credentials without manual editing.
The file uses ``configparser`` syntax with flat dotted keys::

    [asr]
    engine = rev
    engine.rev.key = <api key>

    engine.tencent.id     = <secret-id>
    engine.tencent.key    = <secret-key>
    engine.tencent.region = ap-guangzhou
    engine.tencent.bucket = <cos-bucket>

    engine.aliyun.ak_id     = <ak-id>
    engine.aliyun.ak_secret = <ak-secret>
    engine.aliyun.ak_appkey = <appkey>

    engine.openai.key = <api key>

    [translate]
    engine.google.key = <api key>

    [auth]
    hf_token = <huggingface token>

    [ud]
    model_version = 1.7.0

Two access shapes:

* :func:`get_api_key` for the common "one provider has one key" case
  (Rev.AI, OpenAI, HuggingFace, Google Translate).
* :func:`get_provider` for engines like Tencent / Aliyun that need
  several fields. Returns a dict of every ``engine.<provider>.<field>``
  pair found under the requested section.

Both functions accept ``interactive=True`` — when the lookup would
otherwise return nothing and stdin is a TTY, a rich-rendered form
prompts the user for the missing fields and writes them back to
``~/.batchalign.ini`` so the next run is silent. Callsites must opt in
explicitly; default behavior is non-interactive (so library use, CI,
and tests never block on input).

All lookups also honor environment variables (``BATCHALIGN_<PROVIDER>_*``)
so CI and containerized runs can avoid touching the home directory.
"""

from __future__ import annotations

import configparser
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping

CONFIG_PATH = Path.home() / ".batchalign.ini"

# Map of provider names (used in `get_api_key`) to the (section, option)
# pair in BA2's ini layout. New providers go here.
_API_KEY_LOCATIONS: Mapping[str, tuple[str, str]] = {
    "rev": ("asr", "engine.rev.key"),
    "revai": ("asr", "engine.rev.key"),
    "openai": ("asr", "engine.openai.key"),
    "qwen": ("asr", "engine.aliyun.ak_secret"),
    "google_translate": ("translate", "engine.google.key"),
    "google": ("translate", "engine.google.key"),
    "hf": ("auth", "hf_token"),
    "huggingface": ("auth", "hf_token"),
}

# Map of provider -> (section, prefix). `get_provider("tencent")` returns
# every option under `[asr]` whose name starts with `engine.tencent.`, with
# the prefix stripped (e.g. ``engine.tencent.id`` → key ``id``).
_PROVIDER_LOCATIONS: Mapping[str, tuple[str, str]] = {
    "tencent": ("asr", "engine.tencent."),
    "aliyun": ("asr", "engine.aliyun."),
    "funasr": ("asr", "engine.funasr."),
    "qwen": ("asr", "engine.qwen."),
    "openai": ("asr", "engine.openai."),
    "rev": ("asr", "engine.rev."),
    "google": ("translate", "engine.google."),
}


@dataclass(frozen=True)
class _Field:
    """One field in an interactive credential form.

    ``secret`` masks input so keys don't echo to the terminal.
    ``required`` lets us skip optional fields (base_url, model, etc.)
    without rejecting the form when the user presses enter.
    """

    name: str
    label: str
    secret: bool = True
    required: bool = True
    default: str | None = None


# Form schemas for `get_provider("...", interactive=True)`. Each entry
# lists the fields the backend reads from `creds`, in the order shown
# to the user. Field names must match what the backends look up.
_PROVIDER_FIELDS: Mapping[str, tuple[_Field, ...]] = {
    "tencent": (
        _Field("id", "Secret ID"),
        _Field("key", "Secret Key"),
        _Field("region", "Region (e.g. ap-guangzhou)", secret=False, default="ap-guangzhou"),
        _Field("bucket", "COS Bucket", secret=False),
    ),
    "aliyun": (
        _Field("ak_id", "AccessKey ID"),
        _Field("ak_secret", "AccessKey Secret"),
        _Field("ak_appkey", "AppKey"),
    ),
    "qwen": (
        _Field("api_key", "API Key"),
        _Field("base_url", "Base URL (optional)", secret=False, required=False),
        _Field("model", "Model (optional)", secret=False, required=False),
    ),
    "funasr": (
        _Field("api_key", "API Key"),
    ),
    "openai": (
        _Field("key", "API Key"),
    ),
    "rev": (
        _Field("key", "API Key"),
    ),
    "google": (
        _Field("key", "API Key"),
    ),
}

# Display labels shown in the form header for `get_api_key` prompts.
_API_KEY_DISPLAY: Mapping[str, tuple[str, str]] = {
    "rev": ("Rev.AI", "API Key"),
    "revai": ("Rev.AI", "API Key"),
    "openai": ("OpenAI", "API Key"),
    "qwen": ("Qwen (Aliyun)", "AccessKey Secret"),
    "google_translate": ("Google Translate", "API Key"),
    "google": ("Google Translate", "API Key"),
    "hf": ("HuggingFace", "Access Token"),
    "huggingface": ("HuggingFace", "Access Token"),
}


def _load_config(path: Path = CONFIG_PATH) -> configparser.ConfigParser | None:
    """Read and return a parsed config, or ``None`` if the file doesn't exist."""
    if not path.exists():
        return None
    cfg = configparser.ConfigParser()
    try:
        cfg.read(path)
    except (configparser.Error, OSError):
        return None
    return cfg


def get(section: str, option: str, *, path: Path = CONFIG_PATH) -> str | None:
    """Generic config getter. Returns ``None`` if absent."""
    cfg = _load_config(path)
    if cfg is None or not cfg.has_option(section, option):
        return None
    return cfg.get(section, option)


def get_api_key(
    provider: str,
    *,
    path: Path = CONFIG_PATH,
    interactive: bool = False,
) -> str | None:
    """Return the API key for ``provider``, or ``None`` if not configured.

    Resolution order:
      1. ``BATCHALIGN_<PROVIDER>_KEY`` env var (uppercased).
      2. ``~/.batchalign.ini`` per :data:`_API_KEY_LOCATIONS`.
      3. If ``interactive=True`` and stdin is a TTY, pop a rich form
         and persist the entered value to ``path`` for next time.
    """
    env_var = f"BATCHALIGN_{provider.upper()}_KEY"
    env_val = os.environ.get(env_var)
    if env_val:
        return env_val
    loc = _API_KEY_LOCATIONS.get(provider.lower())
    if loc is None:
        return None
    value = get(loc[0], loc[1], path=path)
    if value:
        return value
    if interactive and _can_prompt():
        return _prompt_single(provider, loc, path)
    return None


def get_provider(
    provider: str,
    *,
    path: Path = CONFIG_PATH,
    interactive: bool = False,
) -> dict[str, str]:
    """Return every ``engine.<provider>.<field>`` value as a flat dict.

    Empty dict when the section / prefix is absent. Environment variables
    of the form ``BATCHALIGN_<PROVIDER>_<FIELD>`` override the file values
    (uppercased; e.g. ``BATCHALIGN_TENCENT_ID``).

    With ``interactive=True``, missing **required** fields (per
    :data:`_PROVIDER_FIELDS`) trigger a rich form covering only the
    gaps; entered values are written back to ``path``.
    """
    out: dict[str, str] = {}
    loc = _PROVIDER_LOCATIONS.get(provider.lower())
    if loc is not None:
        section, prefix = loc
        cfg = _load_config(path)
        if cfg is not None and cfg.has_section(section):
            for name, value in cfg.items(section):
                if name.startswith(prefix):
                    out[name[len(prefix):]] = value
    env_prefix = f"BATCHALIGN_{provider.upper()}_"
    for env_name, env_value in os.environ.items():
        if env_name.startswith(env_prefix):
            out[env_name[len(env_prefix):].lower()] = env_value
    if interactive:
        fields = _PROVIDER_FIELDS.get(provider.lower())
        if fields is not None:
            missing = [f for f in fields if f.required and not out.get(f.name)]
            if missing and _can_prompt() and loc is not None:
                entered = _prompt_form(provider, missing, loc, path)
                out.update(entered)
    return out


def has_config(path: Path = CONFIG_PATH) -> bool:
    """Whether ``~/.batchalign.ini`` exists and is readable."""
    return _load_config(path) is not None


# Process-wide kill switch for interactive prompting. The CLI flips this
# when `-q/--quiet` is passed so the user gets the original silent-fail
# behavior even though backend callsites request `interactive=True`.
_INTERACTIVE_SUPPRESSED: bool = False


def suppress_interactive(suppressed: bool = True) -> None:
    """Disable interactive prompting process-wide.

    Used by the CLI under `--quiet`: backends still pass `interactive=True`
    at their callsites, but this flag makes the prompt a no-op so quiet
    mode stays silent.
    """
    global _INTERACTIVE_SUPPRESSED
    _INTERACTIVE_SUPPRESSED = suppressed


def _can_prompt() -> bool:
    """True when stdin and stderr are TTYs and prompting isn't suppressed."""
    if _INTERACTIVE_SUPPRESSED:
        return False
    try:
        return sys.stdin.isatty() and sys.stderr.isatty()
    except (AttributeError, ValueError):
        return False


def _persist(path: Path, section: str, option: str, value: str) -> None:
    """Write ``value`` into ``path`` under ``[section] option = value``.

    Creates the file and section as needed; preserves other keys.
    """
    cfg = configparser.ConfigParser()
    if path.exists():
        try:
            cfg.read(path)
        except (configparser.Error, OSError):
            cfg = configparser.ConfigParser()
    if not cfg.has_section(section):
        cfg.add_section(section)
    cfg.set(section, option, value)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        cfg.write(fh)


def _render_header(provider: str, intro: str) -> None:
    """Print a styled rich panel introducing the form."""
    from rich.console import Console
    from rich.panel import Panel

    console = Console(stderr=True)
    title = f"[bold]{provider}[/bold] credentials needed"
    body = (
        f"{intro}\n"
        f"Values are saved to [cyan]{CONFIG_PATH}[/cyan] so this prompt\n"
        f"won't appear again. Press [bold]Ctrl-C[/bold] to abort."
    )
    console.print(Panel(body, title=title, border_style="yellow", expand=False))


def _ask(label: str, *, secret: bool, default: str | None) -> str:
    """Prompt for one field via rich.prompt.Prompt."""
    from rich.prompt import Prompt

    return Prompt.ask(
        f"[bold cyan]{label}[/bold cyan]",
        password=secret,
        default=default if default is not None else "",
        show_default=default is not None and not secret,
    )


def _prompt_single(
    provider: str,
    location: tuple[str, str],
    path: Path,
) -> str | None:
    """Prompt for and persist a single API key. Returns None on abort."""
    display_name, label = _API_KEY_DISPLAY.get(
        provider.lower(), (provider, "API Key")
    )
    section, option = location
    _render_header(display_name, f"Enter your {display_name} {label}.")
    try:
        value = _ask(label, secret=True, default=None).strip()
    except (KeyboardInterrupt, EOFError):
        return None
    if not value:
        return None
    _persist(path, section, option, value)
    return value


def _prompt_form(
    provider: str,
    fields: list[_Field],
    location: tuple[str, str],
    path: Path,
) -> dict[str, str]:
    """Prompt for the missing provider fields. Persists each non-empty entry."""
    section, prefix = location
    _render_header(
        provider,
        f"Enter the {len(fields)} credential field(s) needed for {provider}.",
    )
    collected: dict[str, str] = {}
    for field in fields:
        try:
            raw = _ask(field.label, secret=field.secret, default=field.default)
        except (KeyboardInterrupt, EOFError):
            break
        value = raw.strip()
        if not value:
            if field.required:
                break
            continue
        _persist(path, section, f"{prefix}{field.name}", value)
        collected[field.name] = value
    return collected


__all__ = [
    "CONFIG_PATH",
    "get",
    "get_api_key",
    "get_provider",
    "has_config",
    "suppress_interactive",
]
