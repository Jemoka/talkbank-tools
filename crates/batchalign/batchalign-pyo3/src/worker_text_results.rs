//! Rust-owned normalization for worker-protocol V2 text-task batch results.
//!
//! **See also:** [INTERFACE_MAP.md](../INTERFACE_MAP.md) section "7. Text Task Result Normalization" for:
//! - Python caller: `python/batchalign/worker/_text_v2.py`
//! - Full Rust/Python responsibility split and input/output contracts.

use batchalign_types::worker::{BatchInferResponse, InferResponse};
use batchalign_types::worker_v2::{
    CorefAnnotationV2, CorefChainRefV2, CorefItemResultV2, CorefResultV2, MorphosyntaxItemResultV2,
    MorphosyntaxResultV2, TranslationItemResultV2, TranslationResultV2, UtsegItemResultV2,
    UtsegResultV2,
};
use pyo3::prelude::*;
use talkbank_transform::coref::CorefRawResponse;

use crate::error::BatchalignBoundaryError;
use crate::py_json_bridge::py_to_json_value;

fn normalize_result_count<'a>(
    response: &'a BatchInferResponse,
    expected_count: usize,
    task: &str,
) -> PyResult<&'a [InferResponse]> {
    let actual_count = response.results.len();
    if actual_count != expected_count {
        return Err(BatchalignBoundaryError::internal(format!(
            "worker protocol V2 {task} host returned {actual_count} items, expected {expected_count}"
        )).into_py_err());
    }
    Ok(response.results.as_slice())
}

fn response_object<'a>(
    result: Option<&'a serde_json::Value>,
    task: &str,
) -> PyResult<Option<&'a serde_json::Map<String, serde_json::Value>>> {
    match result {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Object(obj)) => Ok(Some(obj)),
        Some(_) => Err(BatchalignBoundaryError::internal(format!(
            "{task} V2 expected a JSON-object result"
        ))
        .into_py_err()),
    }
}

fn normalize_morphosyntax_raw_sentences(
    result: Option<&serde_json::Value>,
) -> PyResult<Option<Vec<serde_json::Value>>> {
    let Some(obj) = response_object(result, "morphosyntax")? else {
        return Ok(None);
    };

    if let Some(raw_sentences) = obj.get("raw_sentences") {
        return match raw_sentences {
            serde_json::Value::Array(items) => Ok(Some(items.clone())),
            _ => Err(BatchalignBoundaryError::internal(
                "morphosyntax V2 raw_sentences must be a list",
            )
            .into_py_err()),
        };
    }

    match obj.get("sentences") {
        Some(serde_json::Value::Array(sentences)) if sentences.is_empty() => Ok(Some(Vec::new())),
        _ => Err(BatchalignBoundaryError::internal(
            "morphosyntax V2 expected raw_sentences in worker result",
        )
        .into_py_err()),
    }
}

fn normalize_string_list(
    result: Option<&serde_json::Value>,
    field_name: &str,
    task: &str,
) -> PyResult<Option<Vec<String>>> {
    let Some(obj) = response_object(result, task)? else {
        return Ok(None);
    };

    let Some(value) = obj.get(field_name) else {
        return Ok(None);
    };

    match value {
        serde_json::Value::Array(items) if items.iter().all(serde_json::Value::is_string) => {
            Ok(Some(
                items
                    .iter()
                    .map(|item| item.as_str().unwrap_or_default().to_owned())
                    .collect(),
            ))
        }
        _ => Err(BatchalignBoundaryError::internal(format!(
            "{task} V2 field {field_name:?} must be a list[str]"
        ))
        .into_py_err()),
    }
}

fn normalize_usize_list(
    result: Option<&serde_json::Value>,
    field_name: &str,
    task: &str,
) -> PyResult<Option<Vec<usize>>> {
    let Some(obj) = response_object(result, task)? else {
        return Ok(None);
    };

    let Some(value) = obj.get(field_name) else {
        return Ok(None);
    };

    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| match item {
                serde_json::Value::Number(value) => value
                    .as_u64()
                    .and_then(|number| usize::try_from(number).ok())
                    .ok_or_else(|| {
                        BatchalignBoundaryError::internal(format!(
                            "{task} V2 field {field_name:?} values must be non-negative integers"
                        ))
                        .into_py_err()
                    }),
                _ => Err(BatchalignBoundaryError::internal(format!(
                    "{task} V2 field {field_name:?} must be a list[usize]"
                ))
                .into_py_err()),
            })
            .collect::<PyResult<Vec<_>>>()
            .map(Some),
        _ => Err(BatchalignBoundaryError::internal(format!(
            "{task} V2 field {field_name:?} must be a list[usize]"
        ))
        .into_py_err()),
    }
}

fn normalize_string_field(
    result: Option<&serde_json::Value>,
    field_name: &str,
    task: &str,
) -> PyResult<Option<String>> {
    let Some(obj) = response_object(result, task)? else {
        return Ok(None);
    };

    let Some(value) = obj.get(field_name) else {
        return Ok(None);
    };

    match value {
        serde_json::Value::String(text) => Ok(Some(text.clone())),
        _ => Err(BatchalignBoundaryError::internal(format!(
            "{task} V2 field {field_name:?} must be a string"
        ))
        .into_py_err()),
    }
}

fn normalize_coref_annotations(
    result: Option<&serde_json::Value>,
) -> PyResult<Option<Vec<CorefAnnotationV2>>> {
    let Some(obj) = response_object(result, "coref")? else {
        return Ok(None);
    };

    let raw: CorefRawResponse = serde_json::from_value(serde_json::Value::Object(obj.clone()))
        .map_err(|error| {
            BatchalignBoundaryError::internal(format!(
                "coref V2 annotations must match CorefRawResponse: {error}"
            ))
            .into_py_err()
        })?;

    Ok(Some(
        raw.annotations
            .into_iter()
            .map(|annotation| CorefAnnotationV2 {
                sentence_idx: annotation.sentence_idx,
                words: annotation
                    .words
                    .into_iter()
                    .map(|word_refs| {
                        word_refs
                            .into_iter()
                            .map(|chain_ref| CorefChainRefV2 {
                                chain_id: chain_ref.chain_id,
                                is_start: chain_ref.is_start,
                                is_end: chain_ref.is_end,
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect(),
    ))
}

fn normalize_morphosyntax_result(
    response: &BatchInferResponse,
    expected_count: usize,
) -> PyResult<String> {
    let payload = MorphosyntaxResultV2 {
        items: normalize_result_count(response, expected_count, "morphosyntax")?
            .iter()
            .map(|infer_result| {
                Ok(MorphosyntaxItemResultV2 {
                    raw_sentences: normalize_morphosyntax_raw_sentences(
                        infer_result.result.as_ref(),
                    )?,
                    error: infer_result.error.clone(),
                })
            })
            .collect::<PyResult<Vec<_>>>()?,
    };
    serde_json::to_string(&payload)
        .map_err(|error| BatchalignBoundaryError::internal(error.to_string()).into_py_err())
}

fn normalize_utseg_result(
    response: &BatchInferResponse,
    expected_count: usize,
) -> PyResult<String> {
    let payload = UtsegResultV2 {
        items: normalize_result_count(response, expected_count, "utseg")?
            .iter()
            .map(|infer_result| {
                Ok(UtsegItemResultV2 {
                    assignments: normalize_usize_list(
                        infer_result.result.as_ref(),
                        "assignments",
                        "utseg",
                    )?,
                    trees: normalize_string_list(infer_result.result.as_ref(), "trees", "utseg")?,
                    error: infer_result.error.clone(),
                })
            })
            .collect::<PyResult<Vec<_>>>()?,
    };
    serde_json::to_string(&payload)
        .map_err(|error| BatchalignBoundaryError::internal(error.to_string()).into_py_err())
}

fn normalize_translation_result(
    response: &BatchInferResponse,
    expected_count: usize,
) -> PyResult<String> {
    let payload = TranslationResultV2 {
        items: normalize_result_count(response, expected_count, "translate")?
            .iter()
            .map(|infer_result| {
                Ok(TranslationItemResultV2 {
                    raw_translation: normalize_string_field(
                        infer_result.result.as_ref(),
                        "raw_translation",
                        "translate",
                    )?,
                    error: infer_result.error.clone(),
                })
            })
            .collect::<PyResult<Vec<_>>>()?,
    };
    serde_json::to_string(&payload)
        .map_err(|error| BatchalignBoundaryError::internal(error.to_string()).into_py_err())
}

fn normalize_coref_result(
    response: &BatchInferResponse,
    expected_count: usize,
) -> PyResult<String> {
    let payload = CorefResultV2 {
        items: normalize_result_count(response, expected_count, "coref")?
            .iter()
            .map(|infer_result| {
                Ok(CorefItemResultV2 {
                    annotations: normalize_coref_annotations(infer_result.result.as_ref())?,
                    error: infer_result.error.clone(),
                })
            })
            .collect::<PyResult<Vec<_>>>()?,
    };
    serde_json::to_string(&payload)
        .map_err(|error| BatchalignBoundaryError::internal(error.to_string()).into_py_err())
}

#[pyfunction]
#[pyo3(signature = (task, response, expected_count))]
pub(crate) fn normalize_text_task_result(
    _py: Python<'_>,
    task: &str,
    response: &Bound<'_, PyAny>,
    expected_count: usize,
) -> PyResult<String> {
    let response: BatchInferResponse = serde_json::from_value(py_to_json_value(response)?)
        .map_err(|error| BatchalignBoundaryError::internal(error.to_string()).into_py_err())?;

    match task {
        "morphosyntax" => normalize_morphosyntax_result(&response, expected_count),
        "utseg" => normalize_utseg_result(&response, expected_count),
        "translate" => normalize_translation_result(&response, expected_count),
        "coref" => normalize_coref_result(&response, expected_count),
        _ => Err(BatchalignBoundaryError::internal(format!(
            "unsupported text task result normalization: {task}"
        ))
        .into_py_err()),
    }
}

// ---------------------------------------------------------------------------
// Token alignment (used by Python morphosyntax tokenizer realignment)
// ---------------------------------------------------------------------------

/// Align Stanza tokenizer output back to original CHAT words.
///
/// Returns a Python list: plain `str` for unchanged tokens,
/// `(str, bool)` tuples for MWT expansion hints.
#[pyfunction]
pub(crate) fn align_tokens(
    py: Python<'_>,
    original_words: Vec<String>,
    stanza_tokens: Vec<String>,
    alpha2: String,
) -> PyResult<Py<pyo3::types::PyList>> {
    use pyo3::types::{PyBool, PyList, PyString, PyTuple};
    use talkbank_transform::tokenizer_realign::{self, PatchedToken};

    let patched =
        py.detach(|| tokenizer_realign::align_tokens(&original_words, &stanza_tokens, &alpha2));

    let result = PyList::empty(py);
    for tok in &patched {
        match tok {
            PatchedToken::Plain(s) => {
                result.append(PyString::new(py, s))?;
            }
            PatchedToken::Hint(s, expand) => {
                let s_any: Py<PyAny> = PyString::new(py, s).unbind().into_any();
                let b_any: Py<PyAny> = PyBool::new(py, *expand).to_owned().unbind().into_any();
                let tup = PyTuple::new(py, [s_any.bind(py), b_any.bind(py)])?;
                result.append(tup)?;
            }
        }
    }

    Ok(result.unbind())
}
