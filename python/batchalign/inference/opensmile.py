"""OpenSMILE audio feature extraction: audio -> raw tabular features."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import TYPE_CHECKING, NotRequired, TypedDict

import numpy as np
from pydantic import BaseModel, Field

from batchalign.inference._domain_types import AudioPath

if TYPE_CHECKING:
    import opensmile
    import pandas as pd

L = logging.getLogger("batchalign.worker")


class OpenSmileBatchItem(BaseModel):
    """A single openSMILE inference request."""

    audio_path: AudioPath
    feature_set: str = "eGeMAPSv02"
    feature_level: str = "functionals"


class OpenSmileResponse(BaseModel):
    """Raw tabular openSMILE output returned by the Python model host."""

    feature_set: str
    feature_level: str
    num_features: int = Field(ge=0)
    duration_segments: int = Field(ge=0)
    audio_file: str
    rows: list[dict[str, float]] = Field(default_factory=list)
    success: bool
    error: str | None = None


class OpenSmileResult(TypedDict, total=False):
    """Structured return payload for openSMILE extraction."""

    feature_set: str
    feature_level: str
    num_features: int
    duration_segments: int
    audio_file: str
    rows: list[dict[str, float]]
    features_sample: NotRequired[dict[str, float]]
    error: NotRequired[str]
    success: bool


_FEATURE_SET_MAP: dict[str, opensmile.FeatureSet] | None = None


def _get_feature_set_map() -> dict[str, opensmile.FeatureSet]:
    global _FEATURE_SET_MAP
    if _FEATURE_SET_MAP is None:
        import opensmile

        _FEATURE_SET_MAP = {
            "eGeMAPSv02": opensmile.FeatureSet.eGeMAPSv02,
            "eGeMAPSv01b": opensmile.FeatureSet.eGeMAPSv01b,
            "GeMAPSv01b": opensmile.FeatureSet.GeMAPSv01b,
            "ComParE_2016": opensmile.FeatureSet.ComParE_2016,
        }
    return _FEATURE_SET_MAP


def _build_smile_engine(feature_set: str, feature_level: str) -> tuple[object, str]:
    import opensmile

    feature_set_map = _get_feature_set_map()
    feature_set_enum = feature_set_map.get(feature_set, opensmile.FeatureSet.eGeMAPSv02)
    normalized_level = (
        "functionals"
        if feature_level == "functionals"
        else "low_level_descriptors"
    )
    feature_level_enum = (
        opensmile.FeatureLevel.Functionals
        if normalized_level == "functionals"
        else opensmile.FeatureLevel.LowLevelDescriptors
    )
    return (
        opensmile.Smile(
            feature_set=feature_set_enum,
            feature_level=feature_level_enum,
        ),
        normalized_level,
    )


def _rows_from_dataframe(features_df: pd.DataFrame) -> list[dict[str, float]]:
    rows: list[dict[str, float]] = []
    for record in features_df.to_dict(orient="records"):
        rows.append({str(key): float(value) for key, value in record.items()})
    return rows


def _build_response(
    *,
    audio_label: str,
    feature_set: str,
    feature_level: str,
    features_df: pd.DataFrame,
) -> OpenSmileResponse:
    if features_df is None or features_df.empty:
        raise ValueError("Feature extraction returned empty results")

    rows = _rows_from_dataframe(features_df)
    return OpenSmileResponse(
        feature_set=feature_set,
        feature_level=feature_level,
        num_features=len(features_df.columns),
        duration_segments=len(features_df),
        audio_file=audio_label,
        rows=rows,
        success=True,
    )


def infer_opensmile_item(item: OpenSmileBatchItem) -> OpenSmileResponse:
    """Run openSMILE on one audio file path."""
    try:
        smile, normalized_level = _build_smile_engine(item.feature_set, item.feature_level)
        L.info("Extracting features from: %s", Path(item.audio_path).name)
        features_df = smile.process_file(item.audio_path)
        return _build_response(
            audio_label=str(item.audio_path),
            feature_set=item.feature_set,
            feature_level=normalized_level,
            features_df=features_df,
        )
    except Exception as error:
        L.error("Error extracting openSMILE features from %s: %s", item.audio_path, error)
        return OpenSmileResponse(
            feature_set=item.feature_set,
            feature_level=item.feature_level,
            num_features=0,
            duration_segments=0,
            audio_file=str(item.audio_path),
            rows=[],
            success=False,
            error=str(error),
        )


def infer_opensmile_prepared_audio(
    audio: np.ndarray,
    sample_rate_hz: int,
    *,
    feature_set: str = "eGeMAPSv02",
    feature_level: str = "functionals",
    audio_label: str = "<prepared_audio>",
) -> OpenSmileResponse:
    """Run openSMILE on Rust-prepared mono PCM audio."""
    try:
        smile, normalized_level = _build_smile_engine(feature_set, feature_level)
        features_df = smile.process_signal(
            np.asarray(audio, dtype=np.float32),
            sampling_rate=sample_rate_hz,
        )
        return _build_response(
            audio_label=audio_label,
            feature_set=feature_set,
            feature_level=normalized_level,
            features_df=features_df,
        )
    except Exception as error:
        L.error("Error extracting openSMILE features from prepared audio: %s", error)
        return OpenSmileResponse(
            feature_set=feature_set,
            feature_level=feature_level,
            num_features=0,
            duration_segments=0,
            audio_file=audio_label,
            rows=[],
            success=False,
            error=str(error),
        )


def extract_features(
    audio_path: AudioPath,
    feature_set: str = "eGeMAPSv02",
    feature_level: str = "functionals",
) -> OpenSmileResult:
    """Backward-compatible dict wrapper around the single-item adapter."""
    response = infer_opensmile_item(
        OpenSmileBatchItem(
            audio_path=audio_path,
            feature_set=feature_set,
            feature_level=feature_level,
        )
    )
    payload = response.model_dump(mode="json")
    if response.rows:
        payload["features_sample"] = response.rows[0]
    return payload  # type: ignore[return-value]
