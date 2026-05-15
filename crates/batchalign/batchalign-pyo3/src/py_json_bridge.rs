//! Utility for converting Python values to `serde_json::Value`.

use crate::error::BatchalignBoundaryError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

/// Convert a generic Python value into a JSON value for Rust-side parsing.
///
/// This helper is shared across worker execution modules so the Rust boundary,
/// not Python, decides which primitive shapes are accepted from IPC payloads.
pub(crate) fn py_to_json_value(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(v) = value.extract::<bool>() {
        return Ok(serde_json::Value::Bool(v));
    }
    if let Ok(v) = value.extract::<i64>() {
        return Ok(serde_json::Value::Number(v.into()));
    }
    if let Ok(v) = value.extract::<u64>() {
        return Ok(serde_json::Value::Number(v.into()));
    }
    if let Ok(v) = value.extract::<f64>() {
        return serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                BatchalignBoundaryError::internal("invalid float in callback response")
                    .into_py_err()
            });
    }
    if let Ok(v) = value.extract::<String>() {
        return Ok(serde_json::Value::String(v));
    }
    if value.hasattr("model_dump")? {
        let kwargs = PyDict::new(value.py());
        kwargs.set_item("mode", "json")?;
        let dumped = value.call_method("model_dump", (), Some(&kwargs))?;
        return py_to_json_value(dumped.as_any());
    }
    if let Ok(list) = value.cast::<PyList>() {
        let mut items = Vec::with_capacity(list.len());
        for item in list.iter() {
            items.push(py_to_json_value(&item.into_any())?);
        }
        return Ok(serde_json::Value::Array(items));
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        let mut items = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            items.push(py_to_json_value(&item.into_any())?);
        }
        return Ok(serde_json::Value::Array(items));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut obj = serde_json::Map::with_capacity(dict.len());
        for (key, item) in dict.iter() {
            let key_str = key.extract::<String>()?;
            obj.insert(key_str, py_to_json_value(&item.into_any())?);
        }
        return Ok(serde_json::Value::Object(obj));
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "callback response contains unsupported Python type",
    ))
}
