use jaq_all::data;
use jaq_all::jaq_core::{unwrap_valr, Vars};
use jaq_all::json::read::parse_many;
use jaq_all::json::{Val, ValX};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString};

use crate::conv::{py_to_val, val_to_py};
use crate::errors::{ExecutionError, ParseError};

/// A compiled jq filter, created by `jaq.compile()`.
#[pyclass(frozen, module = "jaq")]
pub struct Filter {
    pub(crate) inner: data::Filter,
    pub(crate) code: String,
    pub(crate) vars: Vec<String>,
}

impl Filter {
    /// Collect values for the variables declared at compile time,
    /// in declaration order (which `Vars::new` relies on).
    fn var_values(&self, vars: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<Val>> {
        let mut by_name = Vec::new();
        if let Some(d) = vars {
            for (key, value) in d.iter() {
                let key: String = key
                    .extract()
                    .map_err(|_| PyTypeError::new_err("variable names must be str"))?;
                let key = key.trim_start_matches('$').to_owned();
                if !self.vars.contains(&key) {
                    return Err(PyValueError::new_err(format!(
                        "variable ${key} was not declared in compile()"
                    )));
                }
                by_name.push((key, value));
            }
        }
        self.vars
            .iter()
            .map(|name| match by_name.iter().find(|(key, _)| key == name) {
                Some((_, v)) => py_to_val(v, 0),
                None => Err(PyValueError::new_err(format!(
                    "missing value for variable ${name}"
                ))),
            })
            .collect()
    }
}

#[pymethods]
impl Filter {
    /// Run the filter on one input and return a list of all outputs.
    ///
    /// With `text=True`, `input` must be a JSON string (or bytes); it may
    /// contain multiple whitespace-separated JSON documents, each of which
    /// is fed to the filter in turn.
    ///
    /// `vars` provides a value for every variable declared in `compile()`.
    #[pyo3(signature = (input, *, text = false, vars = None))]
    fn run<'py>(
        &self,
        py: Python<'py>,
        input: Bound<'py, PyAny>,
        text: bool,
        vars: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let var_values = self.var_values(vars)?;
        let outputs = PyList::empty(py);
        let runner = data::Runner::default();
        let collect = &mut |vx: ValX| -> Result<(), PyErr> {
            let val = unwrap_valr(vx).map_err(|e| ExecutionError::new_err(e.to_string()))?;
            outputs.append(val_to_py(py, &val)?)?;
            Ok(())
        };
        let on_input_err = |e: String| ParseError::new_err(e);

        if text {
            let buf: Vec<u8> = if let Ok(s) = input.cast::<PyString>() {
                s.to_str()?.as_bytes().to_vec()
            } else if let Ok(b) = input.cast::<PyBytes>() {
                b.as_bytes().to_vec()
            } else {
                return Err(PyTypeError::new_err(
                    "text=True requires str or bytes input",
                ));
            };
            let inputs = parse_many(&buf).map(|r| r.map_err(|e| e.to_string()));
            data::run(
                &runner,
                &self.inner,
                Vars::new(var_values),
                inputs,
                on_input_err,
                collect,
            )?;
        } else {
            let val = py_to_val(&input, 0)?;
            let inputs = std::iter::once(Ok::<_, String>(val));
            data::run(
                &runner,
                &self.inner,
                Vars::new(var_values),
                inputs,
                on_input_err,
                collect,
            )?;
        }
        Ok(outputs)
    }

    fn __repr__(&self) -> String {
        format!("jaq.Filter({:?})", self.code)
    }
}
