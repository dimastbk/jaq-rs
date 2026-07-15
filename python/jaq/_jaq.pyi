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
    ) -> list[Any]: ...
    @overload
    def run(
        self,
        input: str | bytes,
        *,
        text: Literal[True],
        vars: dict[str, _JsonValue] | None = None,
    ) -> list[Any]: ...
    def __repr__(self) -> str: ...

def compile(code: str, vars: Sequence[str] | None = None) -> Filter: ...
