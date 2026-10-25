#!/usr/bin/env python3
"""Size percentiles from the FUSE spreadsheet corpus metadata (Barik et al., MSR 2015).

Input: ``fuse-bin.analysis.dedup.poi-dec2014.json.gz``, one JSON record per line.
Each record is one deduplicated spreadsheet binary with WARC origin metadata,
Tika file facts and an Apache POI workbook analysis.

The POI analysis is workbook-level only. It has no per-worksheet rows, columns
or cell counts, so those metrics are reported as unavailable rather than
estimated; see docs/research/spreadsheet-size-percentiles-2026-10.md.

Usage:
    python3 tools/corpus/fuse_metadata.py <analysis.json.gz> \
        [--sha1 EXPECTED] [--json-out PATH]
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import math
import os
import sys
from collections import Counter
from datetime import datetime, timedelta, timezone

SCRIPT_VERSION = "1.0.0"
JST = timezone(timedelta(hours=9))

# Stage profiles from .claude/briefs/capacity-plan-2026-10-26.md. Only the
# workbook populated-cell ceiling can be checked with FUSE metadata; the row
# ceilings have no counterpart in the corpus fields.
CAPACITY_STAGES = (
    ("Stage A", 100_000),
    ("Stage B", 500_000),
    ("Stage C", 2_000_000),
    ("Stage D", 20_000_000),
)

# POI fields that partition formula cells and input (non-formula) cells.
FORMULA_SUBTYPES = (
    "integerFormulas",
    "nonIntegerFormulas",
    "stringFormulas",
    "booleanFormulas",
    "errorFormulas",
    "dateTimeFormulas",
    "blankFormulas",
)
INPUT_SUBTYPES = (
    "integerInputCells",
    "nonIntegerInputCells",
    "stringInputCells",
    "booleanInputCells",
    "errorInputCells",
    "dateTimeInputCells",
)


def percentile(sorted_values: list[int], p: float) -> int:
    """Nearest-rank percentile: value at ceil(p/100 * n), 1-indexed."""
    n = len(sorted_values)
    rank = max(1, math.ceil(p / 100.0 * n))
    return sorted_values[rank - 1]


def summarize(values: list[int]) -> dict:
    ordered = sorted(values)
    n = len(ordered)
    return {
        "n": n,
        "p50": percentile(ordered, 50),
        "p75": percentile(ordered, 75),
        "p90": percentile(ordered, 90),
        "p95": percentile(ordered, 95),
        "p99": percentile(ordered, 99),
        "max": ordered[-1],
        "mean": round(sum(ordered) / n, 2),
    }


def file_sha1(path: str) -> str:
    digest = hashlib.sha1()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def as_int(record: dict, key: str) -> int | None:
    value = record.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return int(value)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("analysis", help="deduplicated POI analysis JSON, gzip-compressed")
    parser.add_argument("--sha1", help="expected SHA-1 of the analysis file")
    parser.add_argument("--json-out", help="write the full result as JSON here")
    args = parser.parse_args()

    actual_sha1 = file_sha1(args.analysis)
    if args.sha1 and actual_sha1.lower() != args.sha1.lower():
        print(f"error: SHA-1 mismatch: {actual_sha1} != {args.sha1}", file=sys.stderr)
        return 2

    lines = json_lines = parse_errors = 0
    missing_poi = 0
    error_notifications: Counter[str] = Counter()
    content_types: Counter[str] = Counter()
    content_type_unavailable = 0
    formula_subtype_mismatch = 0
    input_subtype_mismatch = 0
    missing_fields: Counter[str] = Counter()
    duplicate_digests = 0
    seen_digests: set[str] = set()
    excluded_non_positive = 0

    file_bytes: list[int] = []
    worksheets: list[int] = []
    input_cells: list[int] = []
    formula_cells: list[int] = []
    populated_cells: list[int] = []
    populated_formula_free: list[int] = []
    populated_formula_bearing: list[int] = []
    formula_cells_bearing: list[int] = []

    with gzip.open(args.analysis, "rt", encoding="utf-8", errors="replace") as stream:
        for line in stream:
            lines += 1
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                parse_errors += 1
                continue
            json_lines += 1

            digest = (record.get("Tika") or {}).get("Digest")
            if digest:
                if digest in seen_digests:
                    duplicate_digests += 1
                    continue
                seen_digests.add(digest)

            content_type = (record.get("Tika") or {}).get("Tika-Content-Type")
            if content_type:
                content_types[content_type] += 1
            else:
                content_type_unavailable += 1

            poi = record.get("POI")
            if not isinstance(poi, dict):
                missing_poi += 1
                continue
            notification = str(poi.get("errorNotification"))
            error_notifications[notification] += 1
            if notification != "OK":
                continue

            size = as_int((record.get("Tika") or {}), "Length")
            sheets = as_int(poi, "numSheets")
            total_input = as_int(poi, "totalInputCells")
            total_formula = as_int(poi, "totalFormulas")
            for name, value in (
                ("Tika.Length", size),
                ("POI.numSheets", sheets),
                ("POI.totalInputCells", total_input),
                ("POI.totalFormulas", total_formula),
            ):
                if value is None:
                    missing_fields[name] += 1
            if None in (size, sheets, total_input, total_formula):
                continue
            if sheets <= 0 or total_input < 0 or total_formula < 0:
                excluded_non_positive += 1
                continue

            formula_sum = sum(as_int(poi, key) or 0 for key in FORMULA_SUBTYPES)
            if formula_sum != total_formula:
                formula_subtype_mismatch += 1
            input_sum = sum(as_int(poi, key) or 0 for key in INPUT_SUBTYPES)
            if input_sum != total_input:
                input_subtype_mismatch += 1

            populated = total_input + total_formula
            file_bytes.append(size)
            worksheets.append(sheets)
            input_cells.append(total_input)
            formula_cells.append(total_formula)
            populated_cells.append(populated)
            if total_formula == 0:
                populated_formula_free.append(populated)
            else:
                populated_formula_bearing.append(populated)
                formula_cells_bearing.append(total_formula)

    if not populated_cells:
        print("error: no analyzable workbooks", file=sys.stderr)
        return 1

    def coverage(values: list[int]) -> dict:
        ordered = sorted(values)
        n = len(ordered)
        result = {}
        for stage, cap in CAPACITY_STAGES:
            count = sum(1 for value in ordered if value <= cap)
            result[stage] = {
                "cell_limit": cap,
                "workbooks_fitting": count,
                "share_pct": round(100.0 * count / n, 4) if n else None,
            }
        return result

    metrics = {
        "file_bytes": summarize(file_bytes),
        "worksheets_per_workbook": summarize(worksheets),
        "input_cells_per_workbook": summarize(input_cells),
        "formula_cells_per_workbook": summarize(formula_cells),
        "populated_cells_per_workbook": summarize(populated_cells),
    }
    if formula_cells_bearing:
        metrics["formula_cells_per_formula_bearing_workbook"] = summarize(formula_cells_bearing)
    if populated_formula_free:
        metrics["populated_cells_per_formula_free_workbook"] = summarize(populated_formula_free)
    if populated_formula_bearing:
        metrics["populated_cells_per_formula_bearing_workbook"] = summarize(
            populated_formula_bearing
        )

    result = {
        "generated_at_jst": datetime.now(JST).isoformat(timespec="seconds"),
        "script": "tools/corpus/fuse_metadata.py",
        "script_version": SCRIPT_VERSION,
        "python_version": sys.version.split()[0],
        "input": {
            "path": args.analysis,
            "sha1": actual_sha1,
            "compressed_bytes": os.path.getsize(args.analysis),
            "lines": lines,
            "json_records": json_lines,
        },
        "percentile_method": "nearest-rank on sorted values (ceil(p/100*n), 1-indexed)",
        "counts": {
            "analyzable_workbooks": len(populated_cells),
            "formula_free_workbooks": len(populated_formula_free),
            "formula_bearing_workbooks": len(populated_formula_bearing),
            "formula_bearing_share_pct": round(
                100.0 * len(populated_formula_bearing) / len(populated_cells), 4
            ),
            "multi_sheet_workbooks": sum(1 for sheets in worksheets if sheets > 1),
            "multi_sheet_share_pct": round(
                100.0 * sum(1 for sheets in worksheets if sheets > 1) / len(worksheets), 4
            ),
        },
        "metrics": metrics,
        "coverage_cells_only": {
            "all_workbooks": coverage(populated_cells),
            "formula_free_workbooks": coverage(populated_formula_free),
            "formula_bearing_workbooks": coverage(populated_formula_bearing),
        },
        "exclusions": {
            "parse_errors": parse_errors,
            "missing_POI": missing_poi,
            "missing_fields": dict(missing_fields),
            "duplicate_tika_digests_dropped": duplicate_digests,
            "non_positive_dimensions": excluded_non_positive,
            "poi_error_notification": dict(error_notifications.most_common()),
            "formula_subtype_sum_mismatch": formula_subtype_mismatch,
            "input_subtype_sum_mismatch": input_subtype_mismatch,
        },
        "content_types": dict(content_types.most_common()),
        "content_type_unavailable": content_type_unavailable,
        "unavailable_metrics": {
            "highest_used_row_per_worksheet": "FUSE POI analysis carries no sheet dimensions",
            "populated_columns_per_worksheet": "FUSE POI analysis carries no sheet dimensions",
            "non_empty_cells_per_worksheet": "FUSE POI analysis carries no sheet dimensions",
            "highest_used_row_per_workbook": "not derivable from workbook aggregates",
        },
    }

    if args.json_out:
        with open(args.json_out, "w", encoding="utf-8") as handle:
            json.dump(result, handle, indent=2, sort_keys=False)
            handle.write("\n")

    print(f"FUSE metadata analysis ({result['generated_at_jst']})")
    print(f"  input: {args.analysis} sha1={actual_sha1} lines={lines} records={json_lines}")
    print(
        "  analyzable workbooks: {analyzable_workbooks} "
        "(formula-free {formula_free_workbooks}, formula-bearing {formula_bearing_workbooks})".format(
            **result["counts"]
        )
    )
    print(f"  Parser: {result['script']} {SCRIPT_VERSION} on Python {result['python_version']}")
    print()
    header = f"  {'metric':<46}{'n':>8}{'p50':>12}{'p75':>12}{'p90':>12}{'p95':>12}{'p99':>14}{'max':>14}{'mean':>14}"
    print(header)
    for name, stats in metrics.items():
        print(
            f"  {name:<46}{stats['n']:>8}{stats['p50']:>12,}{stats['p75']:>12,}"
            f"{stats['p90']:>12,}{stats['p95']:>12,}{stats['p99']:>14,}{stats['max']:>14,}{stats['mean']:>14,.2f}"
        )
    print()
    for group, stages in result["coverage_cells_only"].items():
        print(f"  coverage (populated cells per workbook, {group}):")
        for stage, row in stages.items():
            print(
                f"    {stage} <= {row['cell_limit']:>11,} cells: "
                f"{row['workbooks_fitting']:>7} workbooks ({row['share_pct']}%)"
            )
    print()
    print(f"  exclusions: {json.dumps(result['exclusions'])}")
    print(f"  content types: {json.dumps(result['content_types'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
