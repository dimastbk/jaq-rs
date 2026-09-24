from __future__ import annotations

import jaq
import pytest

READS_INPUT = [
    ".",
    ".a",
    ".[0]",
    "..",
    "length",
    "not",
    "tojson",
    "@base64",
    "map(.id)",
    "{a}",
    '{"a"}',
    "{(.k): 1}",
    '"id-\\(.id)"',
    "if $x then 1 end",
    "if . then 1 else 2 end",
    "$x[.k]",
    "$x as $y | .",
    "reduce .[] as $v (0; . + $v)",
    "reduce $xs[] as $v (.; . + $v)",
    "try . catch 0",
    "def f: .; f",
    # Local definitions shadow the input-free builtins.
    "def empty: .; empty",
    "def null: .; [null]",
    "def f: def true: .; true; f",
    "module {}; .",
    "$x // .",
    ".a = 1",
    "[.[] | .id]",
]

INPUT_FREE = [
    "1",
    '"a.b"',
    "null",
    "true",
    "false",
    "empty",
    "$x",
    "$x.a.b",
    '$x["a"]',
    "$y[0]",
    "{a: $x.a, b: 1}",
    "{$x}",
    '"id-\\($x.id)"',
    '@base64 "\\($x)"',
    "[$x, $y]",
    "($x).a",
    "if $x then 1 else 2 end",
    "$x | .a",
    "$x | ., .",
    "$x | length",
    "$y | map(. + 1)",
    "$x as $y | $y",
    "reduce $y[] as $v (0; . + $v)",
    "try $x catch .",
    "$x // 1",
    "-$n",
    # Only zero-argument definitions shadow them.
    "def null(f): f; null",
    "module {a: 1}; $x.a",
    "module def f: .; {}; $x",
    # Just a variable to jq, whatever a caller's convention calls it.
    "$input",
    "$input.items",
]


@pytest.mark.parametrize("code", READS_INPUT)
def test_reads_input(code: str) -> None:
    assert jaq.analyze(code).reads_input is True


@pytest.mark.parametrize("code", INPUT_FREE)
def test_input_free(code: str) -> None:
    assert jaq.analyze(code).reads_input is False


_VALUES = {"x": {"a": {"b": 1}, "id": 7}, "y": [1, 2], "n": 3, "input": {"items": [1]}}


@pytest.mark.parametrize(
    "code",
    INPUT_FREE,
)
def test_input_free_programs_ignore_their_input(code: str) -> None:
    """The claim behind `reads_input=False`, checked by running the program."""
    names = list(jaq.analyze(code).vars)
    f = jaq.compile(code, vars=names)
    values = {name: _VALUES[name] for name in names}

    assert f.run({}, vars=values) == f.run({"a": 1, "k": "a", "id": 99}, vars=values)


def test_vars_collects_literal_members() -> None:
    analysis = jaq.analyze('{a: $context.a, b: $context["b"], c: $context.c.d, s: $secrets.tok}')

    assert analysis.vars == {"context": frozenset({"a", "b", "c"}), "secrets": frozenset({"tok"})}


@pytest.mark.parametrize(
    "code",
    ["$context", "$context | keys", "$context[$k]", '$context["a\\($k)"]', "$context + {}"],
)
def test_vars_whole_use(code: str) -> None:
    assert jaq.analyze(code).vars["context"] is None


def test_vars_whole_use_wins_over_members() -> None:
    assert jaq.analyze("$context.a, $context").vars == {"context": None}


def test_vars_in_nested_positions() -> None:
    analysis = jaq.analyze('"\\($a.x)" + (if $b.y then [$c.z] else {k: $d.w} end) | .[$e.v]')

    assert analysis.vars == {
        "a": frozenset({"x"}),
        "b": frozenset({"y"}),
        "c": frozenset({"z"}),
        "d": frozenset({"w"}),
        "e": frozenset({"v"}),
    }


def test_vars_order_of_first_appearance() -> None:
    assert list(jaq.analyze("$b, $a, $b").vars) == ["b", "a"]


@pytest.mark.parametrize(
    ("code", "free"),
    [
        (".[] as $x | $x + $outer", ["outer"]),
        (". as [$a, {b: $c}] | $a + $c + $d", ["d"]),
        (". as {$a} | $a + $b", ["b"]),
        ("reduce .[] as $x (0; . + $x + $step)", ["step"]),
        ("foreach .[] as $x ($init; . + $x; [$x, $out])", ["init", "out"]),
        ("def f($a): $a + $b; f(1)", ["b"]),
        ("def f(g): g + $b; f(1)", ["b"]),
        ("label $out | if . == $stop then break $out else . end", ["stop"]),
        ("(. as $x | $x), $x", ["x"]),
        (". as {($k): $v} | $v", ["k"]),
    ],
    ids=[
        "as",
        "destructuring",
        "object-shorthand-pattern",
        "reduce",
        "foreach",
        "def-param",
        "def-filter-param",
        "label",
        "binding-scope-ends",
        "computed-pattern-key",
    ],
)
def test_vars_are_free_vars_only(code: str, free: list[str]) -> None:
    assert list(jaq.analyze(code).vars) == free


def test_dollar_in_strings_and_comments_is_not_a_var() -> None:
    assert list(jaq.analyze('"hello $notavar \\($real)"').vars) == ["real"]
    assert jaq.analyze('{"$fake": 1}').vars == {}
    assert jaq.analyze("# $comment\n.").vars == {}


def test_vars_feed_compile() -> None:
    code = ".[] | select(.type == $context)"
    f = jaq.compile(code, vars=list(jaq.analyze(code).vars))

    assert f.run([{"type": "order"}, {"type": "user"}], vars={"context": "order"}) == [
        {"type": "order"}
    ]


def test_unknown_filters_still_analyse() -> None:
    """Parse-only: a filter jaq does not define is a call like any other."""
    analysis = jaq.analyze("$x.a | nosuchfilter")

    assert analysis.reads_input is False
    assert analysis.vars == {"x": frozenset({"a"})}
    with pytest.raises(jaq.CompileError):
        jaq.compile("$x.a | nosuchfilter", vars=["x"])


@pytest.mark.parametrize("code", [".[", "(", '"unterminated', "$"])
def test_invalid_program(code: str) -> None:
    with pytest.raises(jaq.CompileError):
        jaq.analyze(code)


def test_repr() -> None:
    assert (
        repr(jaq.analyze("$x.a")) == "jaq.Analysis(reads_input=False, vars={'x': frozenset({'a'})})"
    )


@pytest.mark.parametrize("code", ["module {}; .a", "module def f: 1; {}; .a"])
def test_module_header_is_skipped(code: str) -> None:
    """`compile()` accepts a module header, so `analyze()` must too."""
    assert jaq.analyze(code).reads_input is True
    assert jaq.compile(code).run({"a": 1}) == [1]
