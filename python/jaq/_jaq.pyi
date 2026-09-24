from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Literal, TypeAlias, final, overload

__all__ = [
    "Analysis",
    "CompileError",
    "ExecutionError",
    "Filter",
    "JaqError",
    "ParseError",
    "analyze",
    "compile",
]

_JsonValue: TypeAlias = (
    None
    | bool
    | int
    | float
    | str
    | bytes
    | list["_JsonValue"]
    | tuple["_JsonValue", ...]
    | dict[Any, "_JsonValue"]
)

class JaqError(Exception): ...
class CompileError(JaqError): ...
class ParseError(JaqError): ...
class ExecutionError(JaqError): ...

@final
class Filter:
    @overload
    def run(
        self,
        input: _JsonValue,
        *,
        text: Literal[False] = False,
        vars: dict[str, _JsonValue] | None = None,
    ) -> list[Any]:
        """Run the filter on one input and return all outputs.

        Args:
            input: The input value. With text=True, a JSON string (or
                bytes) that may contain multiple whitespace-separated
                JSON documents, each of which is fed to the filter in turn.
            text: Treat input as raw JSON text instead of a Python value.
            vars: A value for every variable declared in compile().

        Returns:
            A list of all output values.

        Raises:
            ParseError: If a text=True input is not valid JSON.
            ExecutionError: If the filter raises an error at runtime.
        """
    @overload
    def run(
        self,
        input: str | bytes,
        *,
        text: Literal[True],
        vars: dict[str, _JsonValue] | None = None,
    ) -> list[Any]: ...
    @overload
    def first(
        self,
        input: _JsonValue,
        *,
        text: Literal[False] = False,
        vars: dict[str, _JsonValue] | None = None,
    ) -> Any:
        """Run the filter and return only its first output.

        Evaluation is lazy: the filter stops as soon as one output is
        produced.

        Args:
            input: The input value. With text=True, a JSON string (or
                bytes) that may contain multiple whitespace-separated
                JSON documents.
            text: Treat input as raw JSON text instead of a Python value.
            vars: A value for every variable declared in compile().

        Returns:
            The first output value.

        Raises:
            IndexError: If the filter yields no output.
            ParseError: If a text=True input is not valid JSON.
            ExecutionError: If the filter raises an error at runtime.
        """
    @overload
    def first(
        self,
        input: str | bytes,
        *,
        text: Literal[True],
        vars: dict[str, _JsonValue] | None = None,
    ) -> Any: ...
    @overload
    def text(
        self,
        input: _JsonValue,
        *,
        text: Literal[False] = False,
        vars: dict[str, _JsonValue] | None = None,
    ) -> str:
        """Run the filter and serialize all outputs as JSON.

        Args:
            input: The input value. With text=True, a JSON string (or
                bytes) that may contain multiple whitespace-separated
                JSON documents.
            text: Treat input as raw JSON text instead of a Python value.
            vars: A value for every variable declared in compile().

        Returns:
            All outputs serialized as JSON, joined by newlines
            (like the jq CLI).

        Raises:
            ParseError: If a text=True input is not valid JSON.
            ExecutionError: If the filter raises an error at runtime.
        """
    @overload
    def text(
        self,
        input: str | bytes,
        *,
        text: Literal[True],
        vars: dict[str, _JsonValue] | None = None,
    ) -> str: ...
    def __repr__(self) -> str: ...

@final
class Analysis:
    """What a jq filter program reads, found from its parse tree by analyze()."""

    @property
    def reads_input(self) -> bool:
        """Whether the program can read the value it is applied to.

        True not only for `.` and paths such as `.a`, but for anything
        applied to the input implicitly: builtins (`length`, `not`),
        formats (`@base64`), object shorthand (`{a}`), an `if` without
        `else`, and any call the analysis does not know to be input-free.
        """
    @property
    def vars(self) -> dict[str, frozenset[str] | None]:
        """Every free variable, without the leading '$'.

        Free means used but not bound by the program itself: names bound by
        `as`, `reduce`/`foreach` or `def f($x):` are excluded, so the keys
        are exactly what compile(vars=...) needs. Each name maps to the
        literal keys accessed on it (`$x.a` and `$x["b"]` give {"a", "b"}),
        or None if the variable is used in any other way (`$x`,
        `$x | keys`, `$x[$k]`). A `$name` in a string literal or comment is
        not a use.
        """
    def __repr__(self) -> str: ...

def analyze(code: str) -> Analysis:
    """Analyze a jq filter program without compiling it.

    Works on the parse tree only, so a program calling a filter jaq does
    not define can still be analyzed. The analysis is conservative: what
    it cannot prove input-free counts as reading the input.

    Args:
        code: The jq filter program.

    Returns:
        The analysis.

    Raises:
        CompileError: If code cannot be lexed or parsed.
    """

def compile(code: str, vars: Sequence[str] | None = None) -> Filter: ...
