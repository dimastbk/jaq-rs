# jaq-rs

Python bindings for [jaq](https://github.com/01mf02/jaq), a jq-like JSON processor written in Rust.

## Usage

```python
import jaq

f = jaq.compile(".foo[] | . + 1")

f.run({"foo": [1, 2]})             # [2, 3]  — list of ALL outputs
f.run('{"foo": [1]}', text=True)   # [2]     — raw JSON text input
f.run("1 2 3", text=True)          # [1, 2, 3] — multiple JSON documents
```

- `jaq.compile(code)` compiles a jq filter program and returns a reusable `Filter`.
  Invalid programs raise `jaq.CompileError` with a jq-style error report.
- `Filter.run(input, *, text=False)` runs the filter on one input and returns a
  list of all outputs (a jq filter can produce zero, one, or many values).
  - By default `input` is a Python value (`None`, `bool`, `int`, `float`, `str`,
    `bytes`, `list`, `tuple`, `dict`).
  - With `text=True`, `input` must be a JSON string or bytes; multiple
    whitespace-separated JSON documents are each fed to the filter, and their
    outputs are concatenated (like the `jq` CLI over a stream).

### Variables

Declare variable names at compile time and provide their values per run:

```python
f = jaq.compile(".[] | select(.type == $context)", vars=["context"])
f.run(items, vars={"context": "order"})
```

Values can be any convertible Python value (like the jq CLI's `--argjson`).
A missing or undeclared variable raises `ValueError`.

### Exceptions

```text
Exception
└── jaq.JaqError
    ├── jaq.CompileError    — invalid filter program
    ├── jaq.ParseError      — text=True input is not valid JSON
    └── jaq.ExecutionError  — runtime error (error("..."), bad index, ...)
```

Unconvertible Python inputs raise the builtin `TypeError`.

### Value conversion

| Python | jaq | notes |
| --- | --- | --- |
| `None` / `bool` / `int` / `float` / `str` | null / bool / number / string | ints of any size are lossless |
| `bytes` | byte string | jaq is a JSON superset |
| `list` / `tuple` | array | tuples come back as lists |
| `dict` | object | non-string keys are allowed and preserved |

Decimal literals from JSON text (e.g. `0.1`) are returned as `float`.
Strings with invalid UTF-8 are decoded lossily.

## Building

Requires Rust (cargo) and Python ≥ 3.10.

```bash
python3 -m venv .venv
.venv/bin/pip install maturin pytest
.venv/bin/maturin develop        # build + install into the venv
.venv/bin/python -m pytest tests/
```

## Free-threading

The extension declares free-threaded support (`gil_used = false`): on a
free-threaded CPython build (e.g. 3.14t) the GIL stays disabled, and a
`Filter` (which is immutable) can be shared across threads to run filters
truly in parallel. As with any Python code, don't mutate an input object
from another thread while `run()` is converting it.

## Limitations

- On GIL-enabled builds, the GIL is held while a filter runs (jaq values are
  not thread-safe to move), so a long-running filter blocks other Python
  threads. `compile()` releases the GIL.
- No support yet for jq command-line features such as `--slurp` or loading
  filter files.
