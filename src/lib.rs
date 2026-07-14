use pyo3::prelude::*;

mod conv;
mod errors;
mod filter;

use errors::{CompileError, ExecutionError, JaqError, ParseError};
use filter::Filter;

/// Compile a jq filter program into a reusable `Filter`.
///
/// `vars` declares names usable as `$name` inside the program;
/// their values are provided per call via `Filter.run(vars=...)`.
#[pyfunction]
#[pyo3(signature = (code, vars = None))]
fn compile(py: Python, code: &str, vars: Option<Vec<String>>) -> PyResult<Filter> {
    let vars: Vec<String> = vars
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.trim_start_matches('$').to_owned())
        .collect();
    let compiled = py.detach(|| {
        jaq_all::compile_with(code, jaq_all::defs(), jaq_all::data::funs(), &vars).map_err(
            |reports| {
                reports
                    .iter()
                    .map(|fr| jaq_all::load::FileReportsDisp::new(fr).to_string())
                    .collect::<String>()
            },
        )
    });
    match compiled {
        Ok(inner) => Ok(Filter {
            inner,
            code: code.to_owned(),
            vars,
        }),
        Err(msg) => Err(CompileError::new_err(msg)),
    }
}

#[pymodule(gil_used = false)]
fn _jaq(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_class::<Filter>()?;
    m.add("JaqError", py.get_type::<JaqError>())?;
    m.add("CompileError", py.get_type::<CompileError>())?;
    m.add("ParseError", py.get_type::<ParseError>())?;
    m.add("ExecutionError", py.get_type::<ExecutionError>())?;
    Ok(())
}
