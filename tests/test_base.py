import sys
import threading

import jaq
import pytest


def test_identity():
    assert jaq.compile(".").run({"a": 1}) == [{"a": 1}]


def test_multiple_outputs():
    assert jaq.compile(".foo[] | . + 1").run({"foo": [1, 2]}) == [2, 3]


def test_zero_outputs():
    assert jaq.compile("empty").run(1) == []


def test_string_is_value():
    assert jaq.compile(".").run("hi") == ["hi"]


def test_filter_reuse():
    f = jaq.compile(". + 1")
    assert f.run(1) == [2]
    assert f.run(2) == [3]


def test_repr():
    assert repr(jaq.compile(".")) == 'jaq.Filter(".")'


def test_text_single():
    assert jaq.compile(".foo").run('{"foo": [1]}', text=True) == [[1]]


def test_text_multi_docs():
    assert jaq.compile(".").run("1 2 3", text=True) == [1, 2, 3]


def test_text_bytes():
    assert jaq.compile(".").run(b'{"a": 1}', text=True) == [{"a": 1}]


def test_text_invalid_json():
    with pytest.raises(jaq.ParseError):
        jaq.compile(".").run("{oops", text=True)


def test_text_wrong_type():
    with pytest.raises(TypeError):
        jaq.compile(".").run({"a": 1}, text=True)


def test_compile_error():
    with pytest.raises(jaq.CompileError):
        jaq.compile(".foo[")


def test_undefined_function_error():
    with pytest.raises(jaq.CompileError):
        jaq.compile("nosuchfunction")


def test_runtime_error():
    with pytest.raises(jaq.ExecutionError, match="boom"):
        jaq.compile('error("boom")').run(None)


def test_error_hierarchy():
    assert issubclass(jaq.CompileError, jaq.JaqError)
    assert issubclass(jaq.ParseError, jaq.JaqError)
    assert issubclass(jaq.ExecutionError, jaq.JaqError)


def test_unconvertible_input():
    with pytest.raises(TypeError):
        jaq.compile(".").run(object())


@pytest.mark.parametrize(
    "v",
    [
        None,
        True,
        False,
        0,
        -1,
        42,
        1.5,
        "",
        "héllo ünïcode 🎉",
        [1, [2, [3]]],
        {"a": {"b": [None, True]}},
        {},
        [],
    ],
)
def test_roundtrip(v):
    assert jaq.compile(".").run(v) == [v]


def test_bool_stays_bool():
    out = jaq.compile(".").run(True)
    assert out == [True]
    assert isinstance(out[0], bool)


def test_tuple_to_list():
    assert jaq.compile(".").run((1, 2)) == [[1, 2]]


def test_bigint_roundtrip():
    n = 2**100
    assert jaq.compile(".").run(n) == [n]


def test_nonstring_keys():
    assert jaq.compile(".").run({1: "a"}) == [{1: "a"}]


def test_bytes_roundtrip():
    assert jaq.compile(".").run(b"\xff\x00") == [b"\xff\x00"]


def test_text_float():
    assert jaq.compile(".").run("0.1", text=True) == [0.1]


def test_recursive_input():
    lst = []
    lst.append(lst)
    with pytest.raises(TypeError):
        jaq.compile(".").run(lst)


def test_jq_builtins():
    assert jaq.compile("keys").run({"b": 1, "a": 2}) == [["a", "b"]]
    assert jaq.compile("map(. * 2)").run([1, 2]) == [[2, 4]]
    assert jaq.compile("tojson").run({"a": 1}) == ['{"a":1}']


def test_vars_basic():
    f = jaq.compile("$context", vars=["context"])
    assert f.run(None, vars={"context": {"user": "d"}}) == [{"user": "d"}]


def test_vars_in_expression():
    f = jaq.compile(".[] | select(. > $min)", vars=["min"])
    assert f.run([1, 5, 10], vars={"min": 4}) == [5, 10]


def test_vars_multiple():
    f = jaq.compile("$a + $b", vars=["a", "b"])
    assert f.run(None, vars={"a": 1, "b": 2}) == [3]


def test_vars_dollar_prefix_tolerated():
    f = jaq.compile("$x", vars=["$x"])
    assert f.run(None, vars={"$x": 1}) == [1]


def test_vars_reuse_with_different_values():
    f = jaq.compile("$x", vars=["x"])
    assert f.run(None, vars={"x": 1}) == [1]
    assert f.run(None, vars={"x": 2}) == [2]


def test_vars_with_text_mode():
    f = jaq.compile(". + $inc", vars=["inc"])
    assert f.run("1 2", text=True, vars={"inc": 10}) == [11, 12]


def test_vars_missing_value():
    f = jaq.compile("$x", vars=["x"])
    with pytest.raises(ValueError, match=r"missing value for variable \$x"):
        f.run(None)


def test_vars_undeclared_in_run():
    f = jaq.compile(".", vars=[])
    with pytest.raises(ValueError, match=r"\$x was not declared"):
        f.run(None, vars={"x": 1})


def test_vars_undeclared_in_program():
    with pytest.raises(jaq.CompileError):
        jaq.compile("$nope")


def test_vars_nonstring_name():
    f = jaq.compile("$x", vars=["x"])
    with pytest.raises(TypeError, match="variable names must be str"):
        f.run(None, vars={1: "a"})


@pytest.mark.skipif(sys.platform == "emscripten", reason="no threads on wasm")
def test_concurrent_runs_share_filter():
    f = jaq.compile("[range(10000)] | map(. * 2) | add")
    expected = f.run(None)
    results: list = [None] * 8

    def worker(i: int) -> None:
        results[i] = f.run(None)

    threads = [threading.Thread(target=worker, args=(i,)) for i in range(8)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert results == [expected] * 8
