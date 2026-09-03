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


_ENGLISH_APOSTROPHELESS_MWT = re.compile(
    r"^(?:(?:ai|are|ca|could|did|does|do|had|has|have|is|must|need|sha|should|was|were|wo|would)nt"
    r"|(?:could|might|must|should|they|we|would|you)ve"
    r"|cannot|heres|im|ive|thats|theres|theyre|whats|youre)$"
)

_ENGLISH_CONVENTIONAL_WORDS = frozenset(
    "cmon dunno gimme gonna gotta kinda lemme lotta outta sorta wanna whatnot".split()
)

_ITALIAN_ARTICULATED_PREPOSITIONS = frozenset(
    "al allo alla ai agli alle col collo colla coi cogli colle dal dallo dalla "
    "dai dagli dalle del dello della dei degli delle nel nello nella nei negli "
    "nelle pel pei sul sullo sulla sui sugli sulle".split()
)

_ITALIAN_CLITIC_ENDING = re.compile(
    r"(?:"
    r"(?:ce|glie|me|se|te|ve)(?:la|le|li|lo)|"
    r"ci|gli|la|le|li|lo|mi|ne|si|ti|vi"
    r")$"
)


def conform(i):
    """A token may be a bare string or a `(text, is_mwt)` tuple; get its text."""
    return i[0] if isinstance(i, tuple) else i


def matches(i, word):
    return (isinstance(i, tuple) and i[0] == word) or (i == word)


def matches_in(i, fragment):
    return (isinstance(i, tuple) and fragment in i[0]) or (fragment in i)


def front_matches(i, word):
    return (isinstance(i, tuple) and i[: len(word)] == word) or (i[: len(word)] == word)


def tokenizer_processor(tokenized, lang, sent):
    """Re-align Stanza's tokenization of one sentence to `sent`'s word split.

    `lang` is a list of language codes (BA2 passes `list(langs_alpha2)`), so the
    per-language branches use `"en" in lang` membership tests.
    """
    # Split candidates that contain spaces, but retain Stanza's MWT marker
    # when the candidate itself remains intact. The marker authorizes the MWT
    # processor to analyze components *inside* one authoritative CHAT word;
    # the alignment below still prevents native tokenization from changing
    # boundaries between CHAT words.
    split_candidates = []
    for candidate in tokenized:
        fragments = conform(candidate).split(" ")
        if len(fragments) == 1:
            split_candidates.append(candidate)
        else:
            split_candidates.extend(fragments)
    tokenized = split_candidates
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
        if (
            isinstance(i, tuple)
            and i[1] is True
            and (
                (("it" in lang) and i[0].casefold() == "dai")
                or (
                    ("en" in lang)
                    and i[0].casefold() in _ENGLISH_CONVENTIONAL_WORDS
                )
            )
        ):
            # Preserve the established whole-word analyses for conventional
            # English CHAT spellings. ``dunno`` is not the unrelated verb
            # ``dun`` plus ``no``; ``whatnot`` is lexical; and ``gonna`` /
            # ``wanna`` are dictionary-recognized informal spellings. Italian
            # ``dai`` is ambiguous between the verb ``dare`` and ``da + i``.
            # Keep these intact for contextual POS tagging.
            res.append(i[0])
        elif (
            ("en" in lang)
            and isinstance(i, tuple)
            and i[1] is True
            and "'" not in i[0]
            and _ENGLISH_APOSTROPHELESS_MWT.fullmatch(i[0].casefold()) is None
        ):
            # GUM's tokenizer occasionally marks ordinary words and names as
            # MWT candidates (observed examples include ``connor`` and
            # ``anna``). Admit punctuation-free English candidates only when
            # their surface is a known contraction; otherwise retain the
            # authoritative CHAT word without authorizing an internal split.
            res.append(i[0])
        elif (
            ("it" in lang)
            and isinstance(i, tuple)
            and i[1] is True
            and "'" not in i[0]
            and "’" not in i[0]
            and i[0].casefold() not in _ITALIAN_ARTICULATED_PREPOSITIONS
            and _ITALIAN_CLITIC_ENDING.search(i[0].casefold()) is None
        ):
            # Italian's tokenizer can mark ordinary inflected words as MWT
            # candidates.  The MWT model then invents components: observed
            # ``bianche`` became ``bia + nce + he`` and even emitted the
            # invalid dependency label ``iob``.  Native Italian MWTs without
            # apostrophes are articulated prepositions or verb/pronominal
            # clitic forms; reject candidates outside those shapes before
            # the destructive MWT expansion runs.
            res.append(i[0])
        elif ("it" in lang) and isinstance(i, tuple) and i[0] == "l'" and i[1] is True:
            res.append("l'")
        elif (
            ("it" in lang)
            and matches(i, "i")
            and len(res) != 0
            and matches(res[-1], "le")
        ):
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
        elif ("fr" in lang) and conform(i).split("'")[0] in [
            "jusqu",
            "puisqu",
            "quelqu",
            "aujourd",
        ]:
            before, after = conform(i).split("'")
            res.append((f"{before}'", False))
            res.append((after, False))
        elif (
            ("en" in lang)
            and matches_in(i, "'")
            and not (
                len(conform(i).split("'")) > 1
                and conform(i).split("'")[0].strip() == "o"
            )
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
