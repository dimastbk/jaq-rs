use jaq_all::json::{Map, Num, Val};
use num_bigint::BigInt;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};

/// Guards against recursive Python structures, which would otherwise overflow the stack.
const MAX_DEPTH: usize = 1024;

pub fn py_to_val(obj: &Bound<'_, PyAny>, depth: usize) -> PyResult<Val> {
    if depth > MAX_DEPTH {
        return Err(PyTypeError::new_err(
            "input is nested too deeply (or is recursive)",
        ));
    }
    if obj.is_none() {
        Ok(Val::Null)
    } else if let Ok(b) = obj.cast::<PyBool>() {
        // must precede PyInt: bool is a subclass of int
        Ok(Val::from(b.is_true()))
    } else if let Ok(i) = obj.cast::<PyInt>() {
        match i.extract::<isize>() {
            Ok(n) => Ok(Val::from(n)),
            Err(_) => Ok(Val::Num(Num::big_int(i.extract::<BigInt>()?))),
        }
    } else if let Ok(f) = obj.cast::<PyFloat>() {
        Ok(Val::from(f.value()))
    } else if let Ok(s) = obj.cast::<PyString>() {
        Ok(Val::utf8_str(s.to_str()?.to_owned()))
    } else if let Ok(b) = obj.cast::<PyBytes>() {
        Ok(Val::byte_str(b.as_bytes().to_vec()))
    } else if let Ok(l) = obj.cast::<PyList>() {
        l.iter().map(|x| py_to_val(&x, depth + 1)).collect()
    } else if let Ok(t) = obj.cast::<PyTuple>() {
        t.iter().map(|x| py_to_val(&x, depth + 1)).collect()
    } else if let Ok(d) = obj.cast::<PyDict>() {
        let mut map = Map::default();
        for (k, v) in d.iter() {
            map.insert(py_to_val(&k, depth + 1)?, py_to_val(&v, depth + 1)?);
        }
        Ok(Val::obj(map))
    } else {
        Err(PyTypeError::new_err(format!(
            "cannot convert {} to a JSON value",
            obj.get_type().name()?
        )))
    }
}

pub fn val_to_py<'py>(py: Python<'py>, v: &Val) -> PyResult<Bound<'py, PyAny>> {
    Ok(match v {
        Val::Null => py.None().into_bound(py),
        Val::Bool(b) => PyBool::new(py, *b).to_owned().into_any(),
        Val::Num(n) => match n {
            Num::Int(i) => i.into_pyobject(py)?.into_any(),
            Num::BigInt(b) => b.as_ref().into_pyobject(py)?.into_any(),
            Num::Float(f) => f.into_pyobject(py)?.into_any(),
            // decimals are kept as strings by jaq; expose them as floats
            Num::Dec(s) => s
                .parse::<f64>()
                .unwrap_or(f64::NAN)
                .into_pyobject(py)?
                .into_any(),
        },
        Val::TStr(b) => PyString::new(py, &String::from_utf8_lossy(b)).into_any(),
        Val::BStr(b) => PyBytes::new(py, b).into_any(),
        Val::Arr(a) => {
            let list = PyList::empty(py);
            for x in a.iter() {
                list.append(val_to_py(py, x)?)?;
            }
            list.into_any()
        }
        Val::Obj(o) => {
            let dict = PyDict::new(py);
            for (k, x) in o.iter() {
                dict.set_item(val_to_py(py, k)?, val_to_py(py, x)?)?;
            }
            dict.into_any()
        }
    })
}
