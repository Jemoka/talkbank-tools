# chatter-cli

`chatter` is the command-line interface for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html) validation, normalization, JSON conversion, and CLAN-style commands.

## Common Commands

```bash
chatter validate file.cha
chatter validate corpus/ --format json
chatter normalize file.cha -o normalized.cha
chatter to-json file.cha -o file.json
chatter from-json file.json -o file.cha
chatter lint corpus/ --fix
chatter cache stats
chatter schema
chatter clan freq corpus/
```

See the book’s CLI reference for the verified public surface.

## Installation

```bash
cargo install --path crates/chatter/chatter-cli
```

Or via the canonical Bazel build (from the repo root):

```bash
just chatter build       # bazel build -c opt //crates/chatter/chatter-cli:chatter
```

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
