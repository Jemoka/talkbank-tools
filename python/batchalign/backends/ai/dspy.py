"""DSPy-backed generic AI edit backend."""

from __future__ import annotations

import hashlib
import logging
from typing import Any

from batchalign import config
from batchalign.backends.base import AI, BatchPolicy


_log = logging.getLogger("batchalign.ai")


CHAT_SYSTEM_PROMPT = """\
You edit one TalkBank CHAT utterance block at a time.

The `revised_blocks` output field must be a JSON list of one or more strings.
Each string must contain exactly one complete, valid CHAT utterance block,
with no Markdown or explanation. A CHAT utterance block starts with a main
tier such as
`*CHI:\thello .` and may include dependent tiers below it such as
`%mor:\tco|hello .` or `%com:\tcomment`. Main tiers begin with `*`, a speaker
code, `:`, a tab, utterance content, and a terminator such as `.`, `?`, or `!`.
Dependent tiers begin with `%`, a tier code, `:`, a tab, and tier content.
Preserve speaker codes, tabs, bullets, timing markers, dependent tiers, and
CHAT syntax unless the instruction specifically requires a change.
The speaker prefix and tab are structural: keep `*CODE:\t` exactly, changing
only the utterance words after the tab. In JSON strings, write the required
tab as `\\t` and line breaks as `\\n`; after JSON decoding these become real
CHAT tab/newline characters. The separator after `:` must be one tab, not
spaces. Keep media bullets exactly, including the surrounding control
characters in markers like `120_240`. Never invent timestamps, time ranges,
or media bullets. If `current` has no literal `start_end` media bullet, the
revised output must not contain any timing marker or bare numeric range like
`120_240`. Preserve CA and CHAT markup such as `(.)`, `(..)`, `+/.`, `+...`,
`[/]`, `[//]`, `⌈`, `⌉`, `⌊`, `⌋`, `&=in`, `&=laugh`, `&{l=X`, and `&}l=X`
unless the user explicitly asks to edit markup. For translation, translate
ordinary words only and leave CHAT/CA markup unchanged.
Pause markers such as `(.)`, `(..)`, and `(...)` are markup, not prose;
copy them byte-for-byte from `current`.
The `` character is a CHAT media-bullet delimiter, not the word "fifteen";
never translate it, spell it out, or replace it with `15` or `十五`.
Preserve the final CHAT terminator token exactly as it appears in `current`,
including `.`, `?`, `!`, `+/.`, `+...`, `+/`, and `+//`; do not translate it
or substitute a different terminator. In particular, keep ASCII `?`; do not
replace it with `？`.

Never return a complete transcript. Never return headers such as `@Begin`,
`@Languages`, or `@End`. The output must replace only the current source
utterance block.

If the instruction asks for utterance segmentation, you may replace one source
utterance with two or more CHAT utterance blocks. Utterance segmentation is not
ordinary written sentence segmentation: segment conversational CHAT turn units,
including incomplete utterances, interruptions, trailing off, overlaps, and
CHAT/CA markers. Do not split inside retracing, bracketed annotations,
overlap spans, nonvocal spans, or dependent-tier structures. Every emitted
main tier must start with the same speaker code unless the instruction
explicitly asks otherwise, contain exactly one tab after the colon, and end
with a legal CHAT terminator such as `.`, `?`, `!`, `+...`, `+/.`, `+//.`,
`+/?`, `+!?`, `+"/.`, `+".`, `+//?`, `+..?`, or `+.`. If the source has no
media bullet, do not add one. If the source has a media bullet and you split
it, put one monotonic non-overlapping media bullet on each emitted utterance
within the original time range. Remember, you can return more than one utterance
as output if the user asks for segmentation.

Some examples:

Input instruction:
translate only the words to Chinese; preserve CHAT markup
Input current:
*PAR:	hello [/] hello (.) there °my friend° . 120_240
Output revised_blocks:
["*PAR:\\t你好 [/] 你好 (.) 那里 °我的朋友° . 120_240"]

Input instruction:
fix punctuation
Input current:
*CHI:	&-um I want that ↑one ?
Output revised_blocks:
["*CHI:\\t&-um I want that ↑one ?"]

Input instruction:
translate to Chinese
Input current:
*PAR:	I don't know +...
%com:	whispered
Output revised_blocks:
["*PAR:\\t我不知道 +...\\n%com:\\twhispered"]

Input instruction:
split into CHAT utterances
Input current:
*PAR:	I went home then I called him and he said no ?
Output revised_blocks:
["*PAR:\\tI went home .", "*PAR:\\tthen I called him .", "*PAR:\\tand he said no ?"]

Input instruction:
split into CHAT utterances
Input current:
*PAR:	I went home then I called him and he said no ? 120_620
Output revised_blocks:
["*PAR:\\tI went home . 120_280", "*PAR:\\tthen I called him . 280_380", "*PAR:\\tand he said no ? 380_620"]

Follow the JSON output format, DO NOT return extra words, DO NOT say explanation.
"""


class DspyAIBackend(AI):
    """Generic AI edit backend powered by DSPy."""

    def __init__(
        self,
        *,
        module: Any | None = None,
        model: str = "openai/zai-org/GLM-5.2",
        api_base: str = "https://api.together.xyz/v1",
        api_key: str | None = None,
        max_tokens: int = 1024,
        timeout: int = 30,
        batch_size: int = 1,
        batch_window_ms: int = 0,
    ) -> None:
        self._model = model
        self._max_tokens = max_tokens
        self._prompt_hash = hashlib.blake2s(
            CHAT_SYSTEM_PROMPT.encode("utf-8"), digest_size=4
        ).hexdigest()
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)
        if module is None:
            key = api_key if api_key is not None else config.get_api_key(
                "together", section="ai", interactive=True
            )
            if not key:
                raise RuntimeError(
                    "DspyAIBackend missing Together API key. Add `[ai] together.key = ...` "
                    "to ~/.batchalign.ini."
                )

            import dspy  # type: ignore[import-not-found]

            class UtteranceProcessing(dspy.Signature):
                __doc__ = CHAT_SYSTEM_PROMPT

                instruction: str = dspy.InputField(
                    desc=(
                        "User instruction for this one utterance. Example: "
                        "`fix capitalization and punctuation`."
                    )
                )
                current: str = dspy.InputField(
                    desc=(
                        "Exactly one raw CHAT utterance block to revise. "
                        "Example: `*PAR:\thello there .\\n%com:\tquietly\\n`. "
                        "This is not a full transcript."
                    )
                )
                context: list[str] = dspy.InputField(
                    desc=(
                        "Neighboring raw CHAT utterance blocks for context only. "
                        "Example item: `*CHI:\tyes .\\n`. Do not revise or return "
                        "context items."
                    )
                )

                revised_blocks: list[str] = dspy.OutputField(
                    desc=(
                        "JSON list of revised raw CHAT utterance block strings. "
                        "Each list item is one complete utterance block. Use "
                        "`\\t` in JSON strings for the required CHAT tab after "
                        "the speaker colon. For segmentation, return multiple "
                        "items. Do not include transcript headers or context "
                        "utterances."
                    )
                )

            dspy.configure(
                lm=dspy.LM(
                    model,
                    api_key=key,
                    api_base=api_base,
                    max_tokens=max_tokens,
                    timeout=timeout,
                    num_retries=0,
                    temperature=0,
                )
            )
            module = dspy.Predict(UtteranceProcessing)
        self._module = module

    @property
    def name(self) -> str:
        return f"dspy-ai:{self._model}:max{self._max_tokens}:p{self._prompt_hash}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any], *, progress: Any = None, **_kwargs: Any) -> list[Any]:
        from batchalign._core.proto import AiInput, AiOutput

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, AiInput):
                raise TypeError(f"DspyAIBackend does not handle: {type(item).__name__}")
            if not item.instruction.strip():
                raise RuntimeError("DspyAIBackend requires AiInput.instruction")
            revisions = []
            total = len(item.utterances)
            for idx, utterance in enumerate(item.utterances):
                _log.debug(
                    "AI model input\nsource_id: %s\nutterance_index: %s\n"
                    "utterance_total: %s\ninstruction:\n%s\ncontext:\n%s\n"
                    "current:\n%s",
                    item.source_id,
                    idx,
                    total,
                    item.instruction,
                    "\n---\n".join(utterance.context or []),
                    utterance.chat,
                )
                try:
                    prediction = self._module(
                        instruction=item.instruction,
                        current=utterance.chat,
                        context=utterance.context or [],
                    )
                except Exception as exc:  # noqa: BLE001
                    _log.warning(
                        "AI model failure\nsource_id: %s\nutterance_index: %s\n"
                        "utterance_total: %s\ninstruction:\n%s\ncontext:\n%s\n"
                        "current:\n%s\nerror: %s",
                        item.source_id,
                        idx,
                        total,
                        item.instruction,
                        "\n---\n".join(utterance.context or []),
                        utterance.chat,
                        exc,
                    )
                    if progress is not None:
                        progress(idx + 1, total)
                    continue
                revision = _revision_from_prediction(prediction, utterance.index)
                _log.debug(
                    "AI model output\nsource_id: %s\nutterance_index: %s\n"
                    "utterance_total: %s\nrevised:\n%s",
                    item.source_id,
                    idx,
                    total,
                    revision.chat,
                )
                if revision.chat.strip() and revision.chat != utterance.chat:
                    revisions.append(revision)
                if progress is not None:
                    progress(idx + 1, total)
            outputs.append(AiOutput(source_id=item.source_id, revisions=revisions))
        return outputs


def _revision_from_prediction(prediction: Any, index: int) -> Any:
    from batchalign._core.proto import AiRevision

    blocks = getattr(prediction, "revised_blocks", None)
    if blocks is not None:
        if isinstance(blocks, str):
            chat = blocks
        else:
            chat = "\n".join(
                str(block).rstrip("\n")
                for block in blocks
                if str(block).strip()
            )
    else:
        chat = str(getattr(prediction, "revised", "") or "")
    return AiRevision(index=index, chat=chat)


__all__ = ["DspyAIBackend"]
