use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFrozenSet};

mod analysis;
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
        jaq_all::compile_with(code, jaq_all::defs(), jaq_all::data::funs(), &vars)
            .map_err(errors::render)
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

/// What a jq filter program reads, found from its parse tree.
#[pyclass(frozen, module = "jaq", name = "Analysis")]
struct PyAnalysis {
    reads_input: bool,
    vars: Vec<(String, analysis::Members)>,
}

#[pymethods]
impl PyAnalysis {
    /// Whether the program can read the value it is applied to.
    #[getter]
    fn reads_input(&self) -> bool {
        self.reads_input
    }

    /// Every free variable, without the leading `$`, mapped to the literal
    /// keys the program accesses on it, or `None` if it uses the variable in
    /// any other way. The keys are exactly what `compile(vars=...)` needs.
    #[getter]
    fn vars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (name, members) in &self.vars {
            match members {
                analysis::Members::Keys(keys) => out.set_item(name, PyFrozenSet::new(py, keys)?)?,
                analysis::Members::Whole => out.set_item(name, py.None())?,
            }
        }
        Ok(out)
    }

    fn __repr__(&self, py: Python) -> PyResult<String> {
        Ok(format!(
            "jaq.Analysis(reads_input={}, vars={})",
            if self.reads_input { "True" } else { "False" },
            self.vars(py)?.repr()?
        ))
    }
}

/// Analyze a jq filter program without compiling it.
///
/// Reports whether the program reads its input, and which variables it
/// uses without binding them itself, with the members it reaches on each.
/// Conservative: anything the analysis cannot prove input-free counts as
/// reading the input, and any variable use other than a literal key access
/// counts as using the whole variable.
#[pyfunction]
fn analyze(py: Python, code: &str) -> PyResult<PyAnalysis> {
    match py.detach(|| analysis::analyze(code)) {
        Ok(a) => Ok(PyAnalysis {
            reads_input: a.reads_input,
            vars: a.vars,
        }),
        Err(msg) => Err(CompileError::new_err(msg)),
    }
}

#[pymodule(gil_used = false)]
fn _jaq(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_function(wrap_pyfunction!(analyze, m)?)?;
    m.add_class::<PyAnalysis>()?;
    m.add_class::<Filter>()?;
    m.add("JaqError", py.get_type::<JaqError>())?;
    m.add("CompileError", py.get_type::<CompileError>())?;
    m.add("ParseError", py.get_type::<ParseError>())?;
    m.add("ExecutionError", py.get_type::<ExecutionError>())?;
    Ok(())
}
