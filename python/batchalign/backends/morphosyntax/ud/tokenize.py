"""Stanza `tokenize_postprocessor` — faithful port of BA2.

BA2 forces Stanza's tokenizer to honor the upstream (space-split) word
boundaries when `retokenize=False`, and applies a set of per-language clitic
/ MWT fixes. This matters for parity on contractions (English `it's`), Italian
`lei`, French `aujourd'hui`, and CJK languages where Stanza's own
segmentation would otherwise diverge from the main-tier tokenization.

Port of `batchalign2/batchalign/pipelines/morphosyntax/ud.py`:
`tokenizer_processor`, `conform`, `matches`, `matches_in`, `front_matches`,
`adlist_postprocessor`. The DP char-aligner lives in `dp.py` (copied verbatim).

A Stanza `tokenize_postprocessor` receives a list of sentences, each a list of
tokens (a token is a `str`, or a `(text, is_mwt)` tuple). We rewrite each
sentence's token list so it aligns to the original passage's whitespace split.
"""

from __future__ import annotations

import copy
import re
from itertools import groupby

from .dp import PayloadTarget, ReferenceTarget, align


def conform(i):
    """A token may be a bare string or a `(text, is_mwt)` tuple; get its text."""
    return i[0] if type(i) == tuple else i


def matches(i, word):
    return (type(i) == tuple and i[0] == word) or (i == word)


def matches_in(i, fragment):
    return (type(i) == tuple and fragment in i[0]) or (fragment in i)


def front_matches(i, word):
    return (type(i) == tuple and i[: len(word)] == word) or (i[: len(word)] == word)


def tokenizer_processor(tokenized, lang, sent):
    """Re-align Stanza's tokenization of one sentence to `sent`'s word split.

    `lang` is a list of language codes (BA2 passes `list(langs_alpha2)`), so the
    per-language branches use `"en" in lang` membership tests.
    """
    # split tokenized in case stuff got combined
    tokenized = [j for i in tokenized for j in conform(i).split(" ")]
    res: list = []
    split_passage = sent.split(" ")

    # char-level alignment backplates: split_passage is reference, tokenized is payload
    targets: list[PayloadTarget] = []
    refs: list[ReferenceTarget] = []
    for indx, i in enumerate(tokenized):
        for char in conform(i):
            if char.strip() != "":
                targets.append(PayloadTarget(char.strip(), indx))
    for indx, i in enumerate(split_passage):
        for char in i:
            if char.strip() != "":
                refs.append(ReferenceTarget(char.strip(), indx))

    # group tokenized indices that map to the same reference word → combine
    groups = []
    alignment = align(targets, refs, tqdm=False)
    alignments = groupby(alignment, lambda x: x.reference_payload)
    for key, grp in alignments:
        group = []
        for elem in grp:
            group.append(elem.payload)
        groups.append(list(sorted(set(group))))

    seen = []
    new_toks = []
    for i in groups:
        i = list(filter(lambda x: x not in seen, i))
        if len(i) == 1:
            new_toks.append(tokenized[i[0]])
        elif len(i) == 0:
            continue
        else:
            new_toks.append(("".join([conform(tokenized[j]) for j in i]), False))
        seen += i

    tokenized = new_toks

    indx = 0
    while indx < len(tokenized):
        i = tokenized[indx]
        if ("it" in lang) and type(i) == tuple and i[0] == "l'" and i[1] is True:
            res.append("l'")
        elif ("it" in lang) and matches(i, "i") and len(res) != 0 and matches(res[-1], "le"):
            res.pop(-1)
            res.append("lei")
        elif ("pt" in lang) and matches(i, "d'água"):
            res.append(("d'água", True))
        elif ("fr" in lang) and matches(i, "aujourd'hui"):
            res.append("aujourd'hui")
        elif ("fr" in lang) and matches(i, "aujourd'"):
            res.append("aujourd'hui")
            indx += 1
        elif ("fr" in lang) and matches(i, "au"):
            res.append((conform(i), True))
        elif ("fr" in lang) and re.match(r"(\w')+\w+", conform(i)):
            parts = conform(i).split("'")
            with_clitic, without = parts[:-1], parts[-1]
            for elem in with_clitic:
                res.append((f"{elem}'", False))
            res.append((f"{without}", False))
        elif ("fr" in lang) and conform(i).split("'")[0] in ["jusqu", "puisqu", "quelqu", "aujourd"]:
            before, after = conform(i).split("'")
            res.append((f"{before}'", False))
            res.append((after, False))
        elif (
            ("en" in lang)
            and matches_in(i, "'")
            and not (len(conform(i).split("'")) > 1 and conform(i).split("'")[0].strip() == "o")
        ):
            res.append((conform(i), True))
        elif ("nl" in lang) and conform(i).endswith("'s"):
            res.append((conform(i), False))
        else:
            res.append(i)
        indx += 1

    return res


def adlist_postprocessor(i, lang, adlist):
    """Apply a custom MWT lexicon (`adlist`) to a tokenized sentence."""
    cpy = copy.deepcopy(i)
    adlist = {k.lower(): v for k, v in adlist.items()}

    for indx, tok in enumerate(cpy):
        if conform(tok).lower() in adlist:
            cpy[indx] = (conform(tok), list(adlist[conform(tok).lower()]))

    return cpy


__all__ = [
    "conform",
    "matches",
    "matches_in",
    "front_matches",
    "tokenizer_processor",
    "adlist_postprocessor",
]
