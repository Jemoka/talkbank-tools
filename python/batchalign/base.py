"""Core types for writing pipelines and backends — Python mirror of
`crates/batchalign/batchalign-core/src/base.rs`.

Import this module instead of the package root when you only need the
orchestration primitives (Task, Pipeline, BAValue, Backend, marker ABCs,
batch policy, progress channel). Importing `batchalign.base` does not
load the concrete backends or recipes — useful for type-checking,
library code that constructs pipelines without owning backend choice,
and tests.

The same items are also available at the package root for convenience.
"""

from __future__ import annotations

# Types that originate in the Rust extension.
from batchalign._core import (  # type: ignore[attr-defined]
    BAValue,
    BatchPolicy,
    CachePolicy,
    CacheSpec,
    ChatInput,
    AiChatInput,
    MediaInput,
    PairedInput,
    Pipeline,
    ProgressEvent,
    ProgressKind,
    Task,
    default_cache_path,
    nuke_cache,
)

# Backend trait + marker ABCs (pure Python).
from batchalign.backends.base import (
    AI,
    ASR,
    Backend,
    Coref,
    FA,
    Morphosyntax,
    Speaker,
    Translate,
    UtSeg,
    declared_tasks,
)

__all__ = [
    # Rust-side types
    "Task",
    "Pipeline",
    "BAValue",
    "MediaInput",
    "ChatInput",
    "AiChatInput",
    "PairedInput",
    "BatchPolicy",
    "CachePolicy",
    "CacheSpec",
    "ProgressEvent",
    "ProgressKind",
    "default_cache_path",
    "nuke_cache",
    # Backend trait + markers
    "Backend",
    "AI",
    "ASR",
    "FA",
    "Speaker",
    "UtSeg",
    "Morphosyntax",
    "Translate",
    "Coref",
    "declared_tasks",
]
