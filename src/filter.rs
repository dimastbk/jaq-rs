use jaq_all::data;
use jaq_all::jaq_core::{unwrap_valr, Vars};
use jaq_all::json::read::parse_many;
use jaq_all::json::{Val, ValX};
use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
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

    /// Run the filter over `input` (object or, with `text`, JSON text),
    /// calling `f` for every output value. `E: From<PyErr>` lets callers
    /// smuggle their own control flow (e.g. early stop) through `data::run`.
    fn execute<E: From<PyErr>>(
        &self,
        input: &Bound<'_, PyAny>,
        text: bool,
        var_values: Vec<Val>,
        f: impl FnMut(ValX) -> Result<(), E>,
    ) -> Result<(), E> {
        let runner = data::Runner::default();
        let vars = Vars::new(var_values);
        let on_input_err = |e: String| E::from(ParseError::new_err(e));

        if text {
            let buf: Vec<u8> = if let Ok(s) = input.cast::<PyString>() {
                s.to_str().map_err(E::from)?.as_bytes().to_vec()
            } else if let Ok(b) = input.cast::<PyBytes>() {
                b.as_bytes().to_vec()
            } else {
                return Err(E::from(PyTypeError::new_err(
                    "text=True requires str or bytes input",
                )));
            };
            let inputs = parse_many(&buf).map(|r| r.map_err(|e| e.to_string()));
            data::run(&runner, &self.inner, vars, inputs, on_input_err, f)
        } else {
            let val = py_to_val(input, 0).map_err(E::from)?;
            let inputs = std::iter::once(Ok::<_, String>(val));
            data::run(&runner, &self.inner, vars, inputs, on_input_err, f)
        }
    }
}

fn to_execution_err(e: jaq_all::json::Error) -> PyErr {
    ExecutionError::new_err(e.to_string())
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
        self.execute(&input, text, var_values, |vx: ValX| -> Result<(), PyErr> {
            let val = unwrap_valr(vx).map_err(to_execution_err)?;
            outputs.append(val_to_py(py, &val)?)?;
            Ok(())
        })?;
        Ok(outputs)
    }

    /// Run the filter and return only its first output.
    ///
    /// Evaluation is lazy: the filter stops as soon as one output is
    /// produced. Raises `IndexError` if the filter yields no output.
    #[pyo3(signature = (input, *, text = false, vars = None))]
    fn first<'py>(
        &self,
        py: Python<'py>,
        input: Bound<'py, PyAny>,
        text: bool,
        vars: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let var_values = self.var_values(vars)?;
        let mut found: Option<Bound<'py, PyAny>> = None;
        // E = Option<PyErr>: Some(e) is a real error, None means "stop, got one".
        let outcome = self.execute(
            &input,
            text,
            var_values,
            |vx: ValX| -> Result<(), Option<PyErr>> {
                let val = unwrap_valr(vx).map_err(|e| Some(to_execution_err(e)))?;
                found = Some(val_to_py(py, &val).map_err(Some)?);
                Err(None)
            },
        );
        match outcome {
            Err(Some(e)) => Err(e),
            _ => found.ok_or_else(|| PyIndexError::new_err("filter produced no output")),
        }
    }

    /// Run the filter and return all outputs serialized as JSON,
    /// joined by newlines (like the jq CLI).
    #[pyo3(signature = (input, *, text = false, vars = None))]
    fn text<'py>(
        &self,
        input: Bound<'py, PyAny>,
        text: bool,
        vars: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<String> {
        let var_values = self.var_values(vars)?;
        let mut lines = Vec::new();
        self.execute(&input, text, var_values, |vx: ValX| -> Result<(), PyErr> {
            let val = unwrap_valr(vx).map_err(to_execution_err)?;
            lines.push(val.to_string());
            Ok(())
        })?;
        Ok(lines.join("\n"))
    }

    fn __repr__(&self) -> String {
        format!("jaq.Filter({:?})", self.code)
    }
}
