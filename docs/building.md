# Building

## Requirements

- Rust stable
- `make`
- `cargo-nextest` for `make test` (optional; `cargo test` also works)
- Python 3 and pip for the documentation site

## Build targets

```sh
make dev       # debug build
make release   # optimized release build
make check     # format and clippy
make test      # cargo-nextest
make clean     # remove build artifacts
```

The debug binary is written to `target/debug/deskitty`. The release binary is
written to `target/release/deskitty`.

Action sounds are loaded from `sounds/*.wav` at compile time and embedded in
the binary. Keep that directory in the repository when building. At runtime,
playback uses a system audio player; no audio library is linked into the
binary.

## Documentation

The Markdown sources live in `docs/`. The site is generated with MkDocs and
the Material theme.

Install the pinned documentation dependencies:

```sh
python3 -m pip install -r requirements-docs.txt
```

Build the static site:

```sh
make docs
```

The output is written to `site/`. Any static file server can host it.

Preview locally with live reload:

```sh
make docs-serve
```

`mkdocs build --strict` is used for the build, so broken internal links and
missing pages fail the build.
