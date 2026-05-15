"""Minimal reproducer for a Stanza Finnish MWT control-token leak.

Stanza's Finnish pipeline, when its MWT (multi-word token) processor
splits the token ``tollei`` into its two component morphemes, leaks
the character language model's ``<SOS>`` (start-of-sequence) control
token into the *first* expansion word's ``text`` and ``lemma`` fields.

This corrupts downstream consumers that read the public Document API
and write its output elsewhere — in our case, a CHAT ``%mor`` tier
containing invalid entries like ``sconj|<sos>tos~aux|ei-Fin-Neg-S3``.
The angle brackets are not valid CHAT %mor stem content, but the
parser correctly rejects them, surfacing the upstream leak as an
E316 validation error after the morphotag run.

Trigger conditions (observed on Stanza 1.11.1):

* Language: Finnish (``lang="fi"``)
* Processors include MWT: ``tokenize,pos,lemma,depparse,mwt``
* Input has ≥ 3 whitespace-separated tokens
* ``tollei`` appears as a non-boundary token

Observed output for ``"a tollei b"``::

    token.text='a'      word.text='a'        lemma='a'        upos=NOUN
    token.text='tollei' word.text='<SOS>tos' lemma='<SOS>tos' upos=SYM   ← LEAK
                        word.text='ei'       lemma='ei'       upos=VERB
    token.text='b'      word.text='b'        lemma='b'        upos=NOUN

The expected output would split ``tollei`` → ``tos`` (conjunction) +
``ei`` (negation auxiliary); both should be plain stems with no
``<SOS>`` prefix.

This file is intentionally standalone: only ``stanza`` and ``pytest``
are imported, no batchalign symbols. It is safe to copy verbatim into
a Stanza issue tracker submission.

To run as a regression test (requires real Stanza models)::

    uv run pytest -m golden -n 0 \\
        batchalign/tests/pipelines/morphosyntax/test_stanza_fi_mwt_sos_leak.py

To run as a standalone reproducer without pytest::

    uv run python -m batchalign.tests.pipelines.morphosyntax.test_stanza_fi_mwt_sos_leak
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

# Stanza is imported lazily inside helpers/tests so pytest collection
# doesn't pay the ~1-2s stanza+torch+numpy+transformers cascade when
# this file isn't selected.
if TYPE_CHECKING:
    import stanza

# ---------------------------------------------------------------------------
# Reproducer inputs
# ---------------------------------------------------------------------------

#: Three-token input that reliably triggers the leak. Any 3-word sentence
#: placing ``tollei`` in non-boundary position works — we use ASCII
#: placeholders so the reproducer reads cleanly for non-Finnish-speaking
#: Stanza maintainers.
INPUT_TEXT = "a tollei b"

#: Substring we check for in token/word text and lemma fields. Lowercase
#: match catches both ``<SOS>`` (emitted by the tokenizer/POS stages) and
#: ``<sos>`` (emitted by downstream lowercasing).
CONTROL_TOKEN_SUBSTRING = "<sos>"


def _load_pipeline() -> stanza.Pipeline:
    """Build the minimal Finnish pipeline required to reproduce the leak.

    Matches the production Stanza configuration for non-Japanese,
    MWT-capable languages in
    ``python/batchalign/worker/_stanza_loading.py`` but with zero batchalign
    customization layered on top — no tokenizer postprocessor, no
    thread lock, no package overrides.
    """
    import stanza
    from stanza import DownloadMethod

    return stanza.Pipeline(
        lang="fi",
        processors="tokenize,pos,lemma,depparse,mwt",
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        verbose=False,
    )


def _collect_control_token_leaks(
    doc: stanza.Document,
) -> list[tuple[str, str, str | None]]:
    """Walk every token and word, return any hit on ``<sos>``.

    Returns ``(field, text, lemma)`` triples where ``field`` is the
    source attribute (``token.text``, ``word.text``, or ``word.lemma``).
    An empty return value means the pipeline output is clean.
    """
    leaks: list[tuple[str, str, str | None]] = []
    for sent in doc.sentences:
        for tok in sent.tokens:
            if CONTROL_TOKEN_SUBSTRING in tok.text.lower():
                leaks.append(("token.text", tok.text, None))
            for word in tok.words:
                if CONTROL_TOKEN_SUBSTRING in word.text.lower():
                    leaks.append(("word.text", word.text, word.lemma))
                if word.lemma is not None and CONTROL_TOKEN_SUBSTRING in word.lemma.lower():
                    leaks.append(("word.lemma", word.text, word.lemma))
    return leaks


# ---------------------------------------------------------------------------
# Regression test
# ---------------------------------------------------------------------------


@pytest.mark.golden
def test_stanza_fi_mwt_does_not_leak_sos_control_token() -> None:
    """Stanza's Finnish MWT must not prepend ``<SOS>`` to expanded words.

    Currently RED on Stanza 1.11.1. Will go GREEN when either (a) Stanza
    fixes the leak upstream or (b) we downgrade Stanza to a version that
    does not exhibit it. We also need a batchalign-side ingress filter
    to protect against similar leaks in other language pipelines;
    that filter is tracked separately.
    """
    import stanza

    nlp = _load_pipeline()
    doc = nlp(INPUT_TEXT)

    leaks = _collect_control_token_leaks(doc)
    assert not leaks, (
        f"Stanza {stanza.__version__} leaked the neural-LM control token "
        f"{CONTROL_TOKEN_SUBSTRING!r} when processing the input "
        f"{INPUT_TEXT!r}.\n"
        f"  Offending (field, text, lemma) triples: {leaks}\n"
        f"  Expected: `tollei` to split into plain `tos` + `ei` with no "
        f"angle-bracket content anywhere in the Document output."
    )


# ---------------------------------------------------------------------------
# Standalone reproducer (copy into Stanza issue tracker)
# ---------------------------------------------------------------------------


def _main() -> int:
    """Print the minimal reproducer output for a Stanza bug report.

    Returns 0 on clean output, 1 on detected leak (matching pytest's
    exit convention so automation can key off the return code).
    """
    import stanza

    print(f"stanza version: {stanza.__version__}")
    print(f"input: {INPUT_TEXT!r}")
    nlp = _load_pipeline()
    doc = nlp(INPUT_TEXT)

    for sent in doc.sentences:
        for tok in sent.tokens:
            print(f"  token.text={tok.text!r}  id={tok.id}")
            for word in tok.words:
                print(
                    f"    word.text={word.text!r}  lemma={word.lemma!r}  "
                    f"upos={word.upos}"
                )

    leaks = _collect_control_token_leaks(doc)
    if leaks:
        print(f"\nBUG: control-token leak detected: {leaks}")
        return 1
    print("\nclean output — bug not reproduced")
    return 0


if __name__ == "__main__":
    import sys

    sys.exit(_main())
