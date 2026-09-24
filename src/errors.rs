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

/// Render loader/compiler reports into a single human-readable message.
pub(crate) fn render(reports: Vec<jaq_all::load::FileReports>) -> String {
    reports
        .iter()
        .map(|fr| jaq_all::load::FileReportsDisp::new(fr).to_string())
        .collect()
}
