from __future__ import annotations

import typing

__all__ = [
    "CompileError",
    "ExecutionError",
    "Filter",
    "JaqError",
    "ParseError",
    "compile",
]

_JsonValue: typing.TypeAlias = (
    None
    | bool
    | int
    | float
    | str
    | bytes
    | list["_JsonValue"]
    | tuple["_JsonValue", ...]
    | dict[typing.Any, "_JsonValue"]
)

class JaqError(Exception): ...
class CompileError(JaqError): ...
class ParseError(JaqError): ...
class ExecutionError(JaqError): ...

@typing.final
class Filter:
    def run(
        self,
        input: _JsonValue,
        *,
        text: bool = False,
        vars: dict[str, _JsonValue] | None = None,
    ) -> list[typing.Any]: ...
    def __repr__(self) -> str: ...

def compile(code: str, vars: typing.Sequence[str] | None = None) -> Filter: ...
