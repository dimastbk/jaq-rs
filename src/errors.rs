use pyo3::create_exception;
use pyo3::exceptions::PyException;

create_exception!(
    jaq,
    JaqError,
    PyException,
    "Base exception for all jaq errors."
);
create_exception!(
    jaq,
    CompileError,
    JaqError,
    "The filter program failed to compile."
);
create_exception!(
    jaq,
    ParseError,
    JaqError,
    "The JSON text input could not be parsed."
);
create_exception!(
    jaq,
    ExecutionError,
    JaqError,
    "The filter raised an error at runtime."
);
