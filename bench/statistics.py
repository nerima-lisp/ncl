"""Summarize performance samples and compare their observed noise ranges."""

from __future__ import annotations

import math
import statistics
from typing import Any, Mapping, Sequence


def sample_statistics(values: Sequence[int | float]) -> dict[str, Any]:
    samples: list[float] = []
    for value in values:
        if (
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not math.isfinite(value)
        ):
            raise ValueError("samples must be finite numbers")
        samples.append(float(value))
    if not samples:
        return {
            "sample_count": 0,
            "median_seconds": None,
            "minimum_seconds": None,
            "maximum_seconds": None,
            "range_seconds": None,
        }
    minimum = min(samples)
    maximum = max(samples)
    return {
        "sample_count": len(samples),
        "median_seconds": statistics.median(samples),
        "minimum_seconds": minimum,
        "maximum_seconds": maximum,
        "range_seconds": maximum - minimum,
    }


def compare_statistics(
    first: Mapping[str, Any],
    second: Mapping[str, Any],
    *,
    noise_basis: str = "difference",
) -> dict[str, Any]:
    if noise_basis not in {"difference", "ratio"}:
        raise ValueError("noise basis must be difference or ratio")
    first_median = first.get("median_seconds")
    second_median = second.get("median_seconds")
    first_range = first.get("range_seconds")
    second_range = second.get("range_seconds")
    if any(value is None for value in (first_median, second_median, first_range, second_range)):
        return {
            "noise_basis": noise_basis,
            "median_difference_seconds": None,
            "absolute_noise_width_seconds": None,
            "difference_within_noise": None,
            "ratio": None,
            "relative_noise_width_ratio": None,
            "ratio_within_noise": None,
            "noise_below": None,
        }
    difference = float(first_median) - float(second_median)
    absolute_noise = max(float(first_range), float(second_range))
    ratio = float(first_median) / float(second_median) if second_median else None
    relative_noise = max(
        float(first_range) / abs(float(first_median)) if first_median else 0.0,
        float(second_range) / abs(float(second_median)) if second_median else 0.0,
    )
    difference_within_noise = abs(difference) <= absolute_noise
    ratio_within_noise = (
        None if ratio is None else abs(ratio - 1.0) <= relative_noise
    )
    return {
        "median_difference_seconds": difference,
        "noise_basis": noise_basis,
        "absolute_noise_width_seconds": absolute_noise,
        "difference_within_noise": difference_within_noise,
        "ratio": ratio,
        "relative_noise_width_ratio": relative_noise,
        "ratio_within_noise": ratio_within_noise,
        "noise_below": (
            difference_within_noise if noise_basis == "difference" else ratio_within_noise
        ),
    }


def paired_statistics(
    first_samples: Sequence[Mapping[str, Any]],
    second_samples: Sequence[Mapping[str, Any]],
    *,
    expected_sample_count: int,
) -> dict[str, Any]:
    def by_pair(samples: Sequence[Mapping[str, Any]]) -> dict[int, float]:
        values: dict[int, float] = {}
        for sample in samples:
            if sample.get("status") != "passed":
                continue
            pair = sample.get("pair")
            seconds = sample.get("seconds")
            if not isinstance(pair, int) or isinstance(pair, bool) or pair < 1:
                raise ValueError("passed paired samples need a positive pair number")
            if (
                isinstance(seconds, bool)
                or not isinstance(seconds, (int, float))
                or not math.isfinite(seconds)
                or seconds <= 0
            ):
                raise ValueError("passed paired samples need a positive finite duration")
            if pair in values:
                raise ValueError("paired samples contain a duplicate pair number")
            values[pair] = float(seconds)
        return values

    first_by_pair = by_pair(first_samples)
    second_by_pair = by_pair(second_samples)
    deltas = [
        first_by_pair[pair] - second_by_pair[pair]
        for pair in sorted(first_by_pair.keys() & second_by_pair.keys())
    ]
    summary = sample_statistics(deltas)
    sample_count = summary["sample_count"]
    median = summary["median_seconds"]
    noise_width = summary["range_seconds"]
    return {
        "sample_count": sample_count,
        "expected_sample_count": expected_sample_count,
        "median_delta_seconds": median,
        "minimum_delta_seconds": summary["minimum_seconds"],
        "maximum_delta_seconds": summary["maximum_seconds"],
        "range_seconds": noise_width,
        "noise_width_seconds": noise_width,
        "ncl_wins": sum(delta < 0 for delta in deltas),
        "sbcl_wins": sum(delta > 0 for delta in deltas),
        "ties": sum(delta == 0 for delta in deltas),
        "noise_below": (
            None
            if median is None or sample_count != expected_sample_count
            else abs(median) <= noise_width
        ),
        "delta_definition": "NCL minus SBCL seconds; negative means NCL was faster",
    }
