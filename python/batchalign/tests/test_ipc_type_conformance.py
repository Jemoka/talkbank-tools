"""Verify that hand-written Python Pydantic models conform to generated Rust schemas.

This test catches Rust/Python IPC type drift at CI time. When a Rust type
changes shape, the generated schemas update (via ``scripts/generate_ipc_types.sh``)
and this test fails until the hand-written Python model is updated to match.

This is the bridge between the current hand-written models and the eventual
goal of fully generated types. See ``python/batchalign/generated/`` for the generated
Pydantic models and ``schemas/ipc/`` for the JSON Schema files.

Cross-language contract note: this is the Python half of the schema conformance
gate. The Rust half lives in ``crates/batchalign/tests/worker_protocol_v2_compat.rs``.
Both sides must pass independently — a change to the wire format must update both.
The ``Cmd2Task`` constant map (formerly tested in ``test_runtime.py``) is also
covered by the IPC schema drift check in CI (``scripts/check_ipc_type_drift.sh``).
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

# Find project root by looking for Cargo.toml
_here = Path(__file__).resolve().parent
ROOT = _here
while ROOT != ROOT.parent:
    if (ROOT / "Cargo.toml").exists() and (ROOT / "ipc-schema").exists():
        break
    ROOT = ROOT.parent
SCHEMA_DIR = ROOT / "ipc-schema"


def _load_schema(layer: str, type_name: str) -> dict:
    """Load a JSON Schema file for an IPC type."""
    path = SCHEMA_DIR / layer / f"{type_name}.json"
    if not path.exists():
        pytest.skip(f"Schema not found: {path}")
    return json.loads(path.read_text())


def _assert_fields_match(schema: dict, model_cls: type, *, allow_extra_python: bool = False) -> None:
    """Assert that the schema's required/optional fields match the Pydantic model."""
    props = schema.get("properties", {})
    required = set(schema.get("required", []))

    model_fields = set(model_cls.model_fields.keys())
    schema_fields = set(props.keys())

    # Every schema field must exist in the Python model
    missing_from_python = schema_fields - model_fields
    assert not missing_from_python, (
        f"{model_cls.__name__} is missing fields defined in Rust schema: {missing_from_python}"
    )

    # Python model should not have extra fields (unless allow_extra_python)
    if not allow_extra_python:
        extra_in_python = model_fields - schema_fields
        assert not extra_in_python, (
            f"{model_cls.__name__} has fields not in Rust schema: {extra_in_python}"
        )


class TestBatchItemConformance:
    """Verify batch item types match Rust schemas."""

    def test_morphosyntax_batch_item(self) -> None:
        from batchalign.inference.morphosyntax import MorphosyntaxBatchItem

        schema = _load_schema("batch_items", "MorphosyntaxBatchItem")
        _assert_fields_match(schema, MorphosyntaxBatchItem)

    def test_utseg_batch_item(self) -> None:
        from batchalign.inference.utseg import UtsegBatchItem

        schema = _load_schema("batch_items", "UtsegBatchItem")
        # Python has extra 'lang' field not in Rust — allow it
        _assert_fields_match(schema, UtsegBatchItem, allow_extra_python=True)

    def test_translate_batch_item(self) -> None:
        from batchalign.inference.translate import TranslateBatchItem

        schema = _load_schema("batch_items", "TranslateBatchItem")
        _assert_fields_match(schema, TranslateBatchItem)

    def test_coref_batch_item(self) -> None:
        from batchalign.inference.coref import CorefBatchItem

        schema = _load_schema("batch_items", "CorefBatchItem")
        _assert_fields_match(schema, CorefBatchItem)

    def test_chain_ref(self) -> None:
        from batchalign.inference.coref import ChainRef

        schema = _load_schema("batch_items", "ChainRef")
        _assert_fields_match(schema, ChainRef)


class TestWorkerV2Conformance:
    """Verify selected V2 protocol types match Rust schemas."""

    def test_execute_request(self) -> None:
        from batchalign.worker._types_v2 import ExecuteRequestV2

        schema = _load_schema("worker_v2", "ExecuteRequestV2")
        _assert_fields_match(schema, ExecuteRequestV2)

    def test_execute_response(self) -> None:
        from batchalign.worker._types_v2 import ExecuteResponseV2

        schema = _load_schema("worker_v2", "ExecuteResponseV2")
        _assert_fields_match(schema, ExecuteResponseV2)

    def test_morphosyntax_item_result(self) -> None:
        from batchalign.worker._types_v2 import MorphosyntaxItemResultV2

        schema = _load_schema("worker_v2", "MorphosyntaxItemResultV2")
        _assert_fields_match(schema, MorphosyntaxItemResultV2)

    def test_whisper_chunk_span(self) -> None:
        from batchalign.worker._types_v2 import WhisperChunkSpanV2

        schema = _load_schema("worker_v2", "WhisperChunkSpanV2")
        _assert_fields_match(schema, WhisperChunkSpanV2)

    def test_asr_element(self) -> None:
        from batchalign.worker._types_v2 import AsrElementV2

        schema = _load_schema("worker_v2", "AsrElementV2")
        _assert_fields_match(schema, AsrElementV2)

    def test_indexed_word_timing(self) -> None:
        from batchalign.worker._types_v2 import IndexedWordTimingV2

        schema = _load_schema("worker_v2", "IndexedWordTimingV2")
        _assert_fields_match(schema, IndexedWordTimingV2)

    def test_speaker_segment(self) -> None:
        from batchalign.worker._types_v2 import SpeakerSegmentV2

        schema = _load_schema("worker_v2", "SpeakerSegmentV2")
        _assert_fields_match(schema, SpeakerSegmentV2)

    def test_morphosyntax_request(self) -> None:
        from batchalign.worker._types_v2 import MorphosyntaxRequestV2

        schema = _load_schema("worker_v2", "MorphosyntaxRequestV2")
        # Python adds `kind` for Pydantic discrimination; Rust schema doesn't
        # include it (added by the tagged enum wrapper at serialization time).
        _assert_fields_match(schema, MorphosyntaxRequestV2, allow_extra_python=True)

    def test_forced_alignment_request(self) -> None:
        from batchalign.worker._types_v2 import ForcedAlignmentRequestV2

        schema = _load_schema("worker_v2", "ForcedAlignmentRequestV2")
        _assert_fields_match(schema, ForcedAlignmentRequestV2, allow_extra_python=True)

    def test_asr_request(self) -> None:
        from batchalign.worker._types_v2 import AsrRequestV2

        schema = _load_schema("worker_v2", "AsrRequestV2")
        _assert_fields_match(schema, AsrRequestV2, allow_extra_python=True)
