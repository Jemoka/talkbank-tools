//! Rust-owned request validation and op dispatch for the Python worker stdio loop.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) section "1. Worker Protocol Dispatch" for:
//! - Python implementation: `python/batchalign/worker/_protocol.py` + `python/batchalign/worker/_handlers.py`
//! - Shared schema: `schemas/ipc/worker_v2/`
//! - Full Rust/Python responsibility split.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

fn repr_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.repr()?.to_str()?.to_string())
}

fn error_payload<'py>(py: Python<'py>, message: &str) -> PyResult<Bound<'py, PyAny>> {
    let payload = PyDict::new(py);
    payload.set_item("op", "error")?;
    payload.set_item("error", message)?;
    Ok(payload.into_any())
}

fn shutdown_payload<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    let payload = PyDict::new(py);
    payload.set_item("op", "shutdown")?;
    Ok(payload.into_any())
}

fn response_payload<'py>(
    py: Python<'py>,
    op: &str,
    response_model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("mode", "json")?;
    let payload = PyDict::new(py);
    payload.set_item("op", op)?;
    payload.set_item(
        "response",
        response_model.call_method("model_dump", (), Some(&kwargs))?,
    )?;
    Ok(payload.into_any())
}

fn validate_request_model<'py>(
    py: Python<'py>,
    op: &str,
    request_model: &Bound<'py, PyAny>,
    req_payload: &Bound<'py, PyAny>,
    validation_error_type: &Bound<'py, PyAny>,
) -> PyResult<Result<Bound<'py, PyAny>, Bound<'py, PyAny>>> {
    match request_model.call_method1("model_validate", (req_payload,)) {
        Ok(request) => Ok(Ok(request)),
        Err(error) => {
            if error.matches(py, validation_error_type)? {
                let payload = error_payload(py, &format!("invalid {op} request: {error}"))?;
                Ok(Err(payload))
            } else {
                Err(error)
            }
        }
    }
}

/// Dispatch one worker IPC message using Rust-owned op routing.
#[pyfunction]
#[pyo3(signature = (
    message,
    *,
    health_fn,
    capabilities_fn,
    infer_fn,
    batch_infer_fn,
    execute_v2_fn,
    ensure_task_fn,
    infer_request_model,
    batch_infer_request_model,
    execute_v2_request_model,
    validation_error_type,
))]
pub(crate) fn dispatch_protocol_message(
    py: Python<'_>,
    message: &Bound<'_, PyAny>,
    health_fn: &Bound<'_, PyAny>,
    capabilities_fn: &Bound<'_, PyAny>,
    infer_fn: &Bound<'_, PyAny>,
    batch_infer_fn: &Bound<'_, PyAny>,
    execute_v2_fn: &Bound<'_, PyAny>,
    ensure_task_fn: &Bound<'_, PyAny>,
    infer_request_model: &Bound<'_, PyAny>,
    batch_infer_request_model: &Bound<'_, PyAny>,
    execute_v2_request_model: &Bound<'_, PyAny>,
    validation_error_type: &Bound<'_, PyAny>,
) -> PyResult<(Py<PyAny>, bool)> {
    let message = match message.cast::<PyDict>() {
        Ok(message) => message,
        Err(_) => {
            return Ok((
                error_payload(py, "request must be a JSON object")?.unbind(),
                false,
            ));
        }
    };

    let (op, op_repr) = match message.get_item("op")? {
        Some(value) => match value.cast::<PyString>() {
            Ok(value) => (value.to_str()?.to_string(), repr_text(value.as_any())?),
            Err(_) => {
                return Ok((
                    error_payload(py, &format!("unknown op: {}", repr_text(&value)?))?.unbind(),
                    false,
                ));
            }
        },
        None => return Ok((error_payload(py, "unknown op: None")?.unbind(), false)),
    };

    if op == "shutdown" {
        return Ok((shutdown_payload(py)?.unbind(), true));
    }

    let payload = match op.as_str() {
        "health" => response_payload(py, &op, &health_fn.call0()?)?,
        "capabilities" => response_payload(py, &op, &capabilities_fn.call0()?)?,
        "infer" => {
            let Some(req_payload) = message.get_item("request")? else {
                return Ok((
                    error_payload(py, "infer request must include mapping field 'request'")?
                        .unbind(),
                    false,
                ));
            };
            let req_payload = match req_payload.cast::<PyDict>() {
                Ok(payload) => payload.as_any(),
                Err(_) => {
                    return Ok((
                        error_payload(py, "infer request must include mapping field 'request'")?
                            .unbind(),
                        false,
                    ));
                }
            };
            let request_model = match validate_request_model(
                py,
                &op,
                infer_request_model,
                req_payload,
                validation_error_type,
            )? {
                Ok(request_model) => request_model,
                Err(payload) => return Ok((payload.unbind(), false)),
            };
            response_payload(py, &op, &infer_fn.call1((request_model,))?)?
        }
        "batch_infer" => {
            let Some(req_payload) = message.get_item("request")? else {
                return Ok((
                    error_payload(
                        py,
                        "batch_infer request must include mapping field 'request'",
                    )?
                    .unbind(),
                    false,
                ));
            };
            let req_payload = match req_payload.cast::<PyDict>() {
                Ok(payload) => payload.as_any(),
                Err(_) => {
                    return Ok((
                        error_payload(
                            py,
                            "batch_infer request must include mapping field 'request'",
                        )?
                        .unbind(),
                        false,
                    ));
                }
            };
            let request_model = match validate_request_model(
                py,
                &op,
                batch_infer_request_model,
                req_payload,
                validation_error_type,
            )? {
                Ok(request_model) => request_model,
                Err(payload) => return Ok((payload.unbind(), false)),
            };
            response_payload(py, &op, &batch_infer_fn.call1((request_model,))?)?
        }
        "execute_v2" => {
            let Some(req_payload) = message.get_item("request")? else {
                return Ok((
                    error_payload(
                        py,
                        "execute_v2 request must include mapping field 'request'",
                    )?
                    .unbind(),
                    false,
                ));
            };
            let req_payload = match req_payload.cast::<PyDict>() {
                Ok(payload) => payload.as_any(),
                Err(_) => {
                    return Ok((
                        error_payload(
                            py,
                            "execute_v2 request must include mapping field 'request'",
                        )?
                        .unbind(),
                        false,
                    ));
                }
            };
            let request_model = match validate_request_model(
                py,
                &op,
                execute_v2_request_model,
                req_payload,
                validation_error_type,
            )? {
                Ok(request_model) => request_model,
                Err(payload) => return Ok((payload.unbind(), false)),
            };
            response_payload(py, &op, &execute_v2_fn.call1((request_model,))?)?
        }
        "ensure_task" => {
            // ensure_task is a lightweight op: extract task + engine_overrides
            // from the request dict and call the Python handler directly.
            let Some(req_payload) = message.get_item("request")? else {
                return Ok((
                    error_payload(
                        py,
                        "ensure_task request must include mapping field 'request'",
                    )?
                    .unbind(),
                    false,
                ));
            };
            let req_dict = match req_payload.cast::<PyDict>() {
                Ok(d) => d,
                Err(_) => {
                    return Ok((
                        error_payload(py, "ensure_task request must be a mapping")?.unbind(),
                        false,
                    ));
                }
            };
            let task = match req_dict.get_item("task")? {
                Some(v) => v,
                None => {
                    return Ok((
                        error_payload(py, "ensure_task request must include 'task'")?.unbind(),
                        false,
                    ));
                }
            };
            let engine_overrides = req_dict.get_item("engine_overrides")?;
            let result = match engine_overrides {
                Some(eo) => ensure_task_fn.call1((task, eo))?,
                None => ensure_task_fn.call1((task, py.None()))?,
            };
            response_payload(py, &op, &result)?
        }
        _ => error_payload(py, &format!("unknown op: {op_repr}"))?,
    };

    Ok((payload.unbind(), false))
}
