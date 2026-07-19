"""Explicit Stanza constituency fallback for unsupported UtSeg languages."""

from __future__ import annotations

from itertools import groupby
from typing import Any

from batchalign.backends.base import BatchPolicy, UtSeg
from batchalign.backends.morphosyntax.ud.lang import to_stanza


def _leaf_count(tree: Any) -> int:
    count = 0
    for child in tree.children:
        count += 1 if child.is_leaf() else _leaf_count(child)
    return count


def _phrase_ranges(subtree: Any, offset: int) -> list[list[int]]:
    """Extract coordinated S-clause leaf ranges from a constituency tree."""
    children = subtree.children
    labels = [(child.label or "").lower() for child in children]
    coordinated = any(label in {"cc", "conj"} for label in labels)
    ranges: list[list[int]] = []
    child_offset = offset
    for child in children:
        if child.is_leaf():
            child_offset += 1
            continue
        leaves = _leaf_count(child)
        if coordinated and child.label == "S":
            ranges.append(list(range(child_offset, child_offset + leaves)))
        ranges.extend(_phrase_ranges(child, child_offset))
        child_offset += leaves
    return ranges


def _compute_assignments(words: list[str], nlp: Any) -> list[int]:
    """Match the fork's constituency-tree-to-utterance assignment policy."""
    count = len(words)
    if count <= 1:
        return [0] * count
    sentences = nlp(" ".join(words)).sentences
    if not sentences or sentences[0].constituency is None:
        return [0] * count

    ranges = sorted(_phrase_ranges(sentences[0].constituency, 0), key=len)
    unique: list[list[int]] = []
    for candidate in list(reversed(ranges)) + [list(range(count))]:
        remaining = set(candidate)
        for existing in unique:
            remaining -= set(existing)
        if remaining and not any(remaining.issubset(set(item)) for item in unique):
            unique.append(sorted(remaining))
    unique = [item for item in reversed(unique) if len(item) > 1]
    if not unique:
        return [0] * count

    phrase_for_word = [-1] * count
    for phrase_id, indices in enumerate(unique):
        for index in indices:
            if 0 <= index < count:
                phrase_for_word[index] = phrase_id
    for index, phrase_id in enumerate(phrase_for_word):
        if phrase_id != -1:
            continue
        right = next((value for value in phrase_for_word[index + 1 :] if value != -1), None)
        left = next(
            (value for value in reversed(phrase_for_word[:index]) if value != -1),
            None,
        )
        phrase_for_word[index] = right if right is not None else (left or 0)

    contiguous = [
        list(indices)
        for _, indices in groupby(range(count), key=phrase_for_word.__getitem__)
    ]
    merged: list[list[int]] = []
    pending: list[int] = []
    for indices in contiguous:
        if len(indices) < 3:
            pending.extend(indices)
        else:
            merged.append(pending + indices)
            pending = []
    if pending:
        if merged:
            merged[-1].extend(pending)
        else:
            merged.append(pending)

    assignments = [0] * count
    for group_id, indices in enumerate(merged):
        for index in indices:
            assignments[index] = group_id
    return assignments


class StanzaUtSegBackend(UtSeg):
    """Opt-in constituency fallback when no TalkBank boundary model exists."""

    def __init__(
        self,
        *,
        lang: str,
        batch_size: int = 16,
        batch_window_ms: int = 50,
    ) -> None:
        import stanza  # type: ignore[import-not-found]

        from batchalign.backends.morphosyntax.stanza import (
            _refresh_stanza_resources_manifest_once,
        )

        _refresh_stanza_resources_manifest_once(stanza)
        stanza_lang = to_stanza(lang)
        self._nlp = stanza.Pipeline(
            lang=stanza_lang,
            processors="tokenize,constituency",
            download_method=stanza.DownloadMethod.REUSE_RESOURCES,
            verbose=False,
        )
        self._lang = stanza_lang
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        return f"stanza-utseg:{self._lang}:constituency-v1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import UtSegInput, UtSegOutput, UtteranceSpan

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, UtSegInput):
                raise TypeError(
                    f"StanzaUtSegBackend does not handle: {type(item).__name__}"
                )
            spans: list[Any] = []
            for segment in item.segments:
                words = list(segment.words)
                if not words:
                    continue
                assignments = _compute_assignments(
                    [str(word.text) for word in words], self._nlp
                )
                groups = [
                    list(indexed)
                    for _, indexed in groupby(
                        range(len(words)), key=assignments.__getitem__
                    )
                ]
                duration = max(0, segment.end_ms - segment.start_ms)
                for indices in groups:
                    grouped_words = [words[index] for index in indices]
                    timed = [
                        word for word in grouped_words if word.end_ms > word.start_ms
                    ]
                    if timed:
                        start_ms = timed[0].start_ms
                        end_ms = timed[-1].end_ms
                    elif duration > 0:
                        start_ms = segment.start_ms + round(
                            indices[0] / len(words) * duration
                        )
                        end_ms = segment.start_ms + round(
                            (indices[-1] + 1) / len(words) * duration
                        )
                    else:
                        start_ms = end_ms = 0
                    spans.append(
                        UtteranceSpan(
                            start_ms=start_ms,
                            end_ms=end_ms,
                            text=" ".join(str(word.text) for word in grouped_words),
                            words=grouped_words,
                        )
                    )
            outputs.append(UtSegOutput(source_id=item.source_id, utterances=spans))
        return outputs


__all__ = ["StanzaUtSegBackend"]
