"""Benchmarks comparing jaq-rs with the jq package (libjq bindings).

Run with: pytest benches/ --benchmark-group-by=group
"""

import json

import jaq
import pytest

jq = pytest.importorskip("jq")

FILTER = ".users[] | select(.age >= 30) | .name"


def make_data(n: int) -> dict:
    return {
        "users": [
            {
                "name": f"user{i}",
                "age": (i * 7) % 60,
                "tags": ["python", "rust"],
                "active": i % 2 == 0,
            }
            for i in range(n)
        ]
    }


SMALL = make_data(10)
LARGE = make_data(10_000)
LARGE_TEXT = json.dumps(LARGE)


@pytest.mark.benchmark(group="compile")
def test_compile_jaq(benchmark) -> None:
    benchmark(jaq.compile, FILTER)


@pytest.mark.benchmark(group="compile")
def test_compile_jq(benchmark) -> None:
    benchmark(jq.compile, FILTER)


@pytest.mark.benchmark(group="run-small")
def test_run_small_jaq(benchmark) -> None:
    f = jaq.compile(FILTER)
    assert benchmark(f.run, SMALL)


@pytest.mark.benchmark(group="run-small")
def test_run_small_jq(benchmark) -> None:
    f = jq.compile(FILTER)
    assert benchmark(lambda: f.input(SMALL).all())


@pytest.mark.benchmark(group="run-large")
def test_run_large_jaq(benchmark) -> None:
    f = jaq.compile(FILTER)
    assert benchmark(f.run, LARGE)


@pytest.mark.benchmark(group="run-large")
def test_run_large_jq(benchmark) -> None:
    f = jq.compile(FILTER)
    assert benchmark(lambda: f.input(LARGE).all())


@pytest.mark.benchmark(group="run-text")
def test_run_text_jaq(benchmark) -> None:
    f = jaq.compile(FILTER)
    assert benchmark(lambda: f.run(LARGE_TEXT, text=True))


@pytest.mark.benchmark(group="run-text")
def test_run_text_jq(benchmark) -> None:
    f = jq.compile(FILTER)
    assert benchmark(lambda: f.input(text=LARGE_TEXT).all())


@pytest.mark.benchmark(group="many-outputs")
def test_many_outputs_jaq(benchmark) -> None:
    f = jaq.compile("range(100000)")
    assert benchmark(f.run, None)


@pytest.mark.benchmark(group="many-outputs")
def test_many_outputs_jq(benchmark) -> None:
    f = jq.compile("range(100000)")
    assert benchmark(lambda: f.input(None).all())
