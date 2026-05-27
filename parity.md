hello! please make Batcahlign3 parity with Batchalign2.
Batchalign2 is at /Users/houjun/Documents/Projects/batchalign2; entrypoint is execute.py
Batchalign3 is trigger with just batchalign::cli

functions that need parity are

- transcribe
- align
- morphotag
- compare
- translate

Parity means:

- all Engines BA2 supported is supported by BA3; this includes, but is not limited to
    * rev
    * whisperx
    * whisper
    * openai's implementation of whisper
    * tested on at least chinese, cantonese, english, and spanish
- for morphotag: multilingual codeswitch, and --retokenize should be supported; tested on japanese

Supported means:

the output of the "important lines" are identical; thatis

- speaker segemntations identical
- per utterance segmentation identical
- mor tier identical
- retokenizatino identical
- wor tier identical

cosmetic markings can change (such as header.)

Parity does not need to mean:

- cli structure identical; you should have some sensible way to specify engines, and its not necesasry that they are the same
- BA3 writes in place or optionally with -o, that's ok

Also, compare structure is bad; the recipe should lok for a template.gold.cha in the input folder as the template for all input folder things, instead of a folder of templates


