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

### Convenience methods

```python
f = jaq.compile(".users[].name")

f.first(data)   # first output only; evaluation stops early (lazy).
                # Raises IndexError if the filter yields no output.
f.text(data)    # all outputs serialized as JSON, joined by newlines
                # (like the jq CLI): '"alice"\n"bob"'
```

Both accept the same `text=` and `vars=` keywords as `run()`.

### Variables

Declare variable names at compile time and provide their values per run:

```python
f = jaq.compile(".[] | select(.type == $context)", vars=["context"])
f.run(items, vars={"context": "order"})
```

Values can be any convertible Python value (like the jq CLI's `--argjson`).
A missing or undeclared variable raises `ValueError`.

`jaq.analyze()` answers what a program *reads*, from its parse tree alone,
so it also works on programs that call filters jaq does not define:

```python
a = jaq.analyze('{id: $context.id, token: $secrets["api"]} | .id')
a.reads_input      # False — `.id` sees the object on the left, not the input
a.vars             # {'context': frozenset({'id'}), 'secrets': frozenset({'api'})}

jaq.analyze("length").reads_input          # True — builtins run on the input
jaq.analyze("$context | keys").vars        # {'context': None} — used whole
```

It is conservative: anything not provably input-free counts as reading the
input (unknown calls, formats such as `@base64`, `{a}` shorthand, an `if`
without `else`), and any use of a variable other than a literal key access
reports the whole variable.

`vars` lists only *free* variables — names bound by the program itself
(`… as $x`, `reduce`/`foreach`, `def f($x):`) are excluded, and a `$name`
inside a string literal or comment is not counted — so it is exactly what
`compile()` has to declare:

```python
code = ".[] | select(.type == $context) | .n > $min"

jaq.analyze(code).vars           # {'context': None, 'min': None}
f = jaq.compile(code, vars=list(jaq.analyze(code).vars))
```

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
