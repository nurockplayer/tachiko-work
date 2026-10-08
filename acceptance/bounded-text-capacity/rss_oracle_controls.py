"""Small deterministic cross-language receipt controls; no runtime/browser."""
import json
import sys
from pathlib import Path

from rss_oracle import MeasurementIncomplete, check_rss_budget


def run(path):
    capture = json.loads(Path(path).read_text())
    assert capture["sensor_controls_only"] is True and capture["product_execution"] is False
    expected = {
        "complete-measurement-positive": "complete",
        "complete-over-budget-negative": "budget_exceeded",
        "missing-live-renderer": "measurement_unverified",
        "unparseable-live-renderer": "measurement_unverified",
        "missing-vmrss-live-renderer": "measurement_unverified",
        "unproved-exit-permission-error": "measurement_unverified",
        "proven-exit-positive": "complete",
        "positive-aggregate-omitted-renderer": "measurement_unverified",
        "exit-without-proof": "measurement_unverified",
        "legacy-aggregate-only": "measurement_unverified",
    }
    assert len(capture["records"]) == len(expected)
    assert {r["name"]: r["outcome"] for r in capture["records"]} == expected
    results = []
    for record in capture["records"]:
        try:
            increase = check_rss_budget(record["samples"])
            actual = "complete"
        except MeasurementIncomplete:
            actual = "measurement_unverified"
        except AssertionError:
            actual = "budget_exceeded"
        assert actual == expected[record["name"]], record["name"]
        if record["name"] == "complete-measurement-positive":
            assert increase == 368 * 1024 * 1024
        elif record["name"] == "proven-exit-positive":
            assert increase == 0
        results.append({"name": record["name"], "expected": actual, "outcome": "PASS"})
    print(json.dumps({"sensor_controls_only": True, "product_execution": False, "performance_qualification": False, "results": results}))


if __name__ == "__main__":
    run(sys.argv[1])
