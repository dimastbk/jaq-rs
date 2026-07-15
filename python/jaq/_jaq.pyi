from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Literal, TypeAlias, final, overload

__all__ = [
    "CompileError",
    "ExecutionError",
    "Filter",
    "JaqError",
    "ParseError",
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

def compile(code: str, vars: Sequence[str] | None = None) -> Filter: ...
