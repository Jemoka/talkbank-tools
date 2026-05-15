# Overlap Points

Overlap points mark the beginning and end of overlapping speech in CHAT transcripts.

## Action Within Overlap

When an action marker `0` appears within overlap markers, they should be parsed as separate elements:

- `⌈` - overlap point (top begin)
- `0` - action (omitted action)
- content (e.g., events, words)
- `⌉` - overlap point (top end)

### Example

```chat
*CHI:	⌈0 &=laughter⌉ .
```

**Expected parse:**
1. `⌈` - overlap_point (top begin marker)
2. `0` - action_with_optional_annotations (action marker)
3. `&=laughter` - event_with_optional_annotations
4. `⌉` - overlap_point (top end marker)
5. `.` - terminator

**NOT** parsed as:
- ❌ `⌈0` as a single word with overlap inside

## References

- [CHAT Manual - Overlaps](https://talkbank.org/0info/manuals/CHAT.html#_Toc110793493)
- [CHAT Manual - Actions](https://talkbank.org/0info/manuals/CHAT.html#OmittedWord_Code)
