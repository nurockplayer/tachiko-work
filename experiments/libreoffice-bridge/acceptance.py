"""Steward acceptance for #355; native evidence and actual Office evidence are distinct.

No missing prerequisite is skipped or classified as behavioral RED. The new
bridge deliberately fails until implemented. Never import this oracle in it.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import bridge
from canary import BASE_MAPPING, RECORDS, SELECTION, expected_candidate, synthetic_observation

ROOT = Path(__file__).resolve().parent
SUCCESS_LEDGER = {
    ("exact", "selected_scalars"),
    ("strengthening", "explicit_schema"),
    ("strengthening", "new_identity"),
    ("presentation", "presentation_not_imported"),
    ("unresolved", "outside_selection_unassessed"),
}


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, allow_nan=False) + "\n", encoding="utf-8")


def executable(name):
    found = shutil.which(os.environ.get(name, ""))
    if not found:
        raise RuntimeError(f"ENVIRONMENT_UNVERIFIED: set {name} to a real executable")
    return found


def call(argv, *, success=True, env=None):
    result = subprocess.run(argv, capture_output=True, text=True, timeout=45, check=False, env=env)
    if success and result.returncode:
        raise AssertionError(f"{argv!r}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
    return result


def native_reports(binary, document, state):
    with tempfile.TemporaryDirectory(prefix="lo355-oracle-") as folder:
        path = Path(folder) / "candidate.ro"
        write_json(path, document)
        call([binary, "validate", str(path)])
        return {name: json.loads(call([binary, "analyze", name, str(path),
                                       "--source-state", state]).stdout)
                for name in ("document", "validation")}


def shim(directory, binary, *, mode="normal", mutation=None):
    """Log and forward REAL native commands; fault modes test fail-closed behavior."""
    path, log = directory / "native-proxy", directory / "native.jsonl"
    body = f'''#!{sys.executable}
import json, pathlib, subprocess, sys
args = sys.argv[1:]
entry = {{"args": args}}
if args and args[0] == "validate" and len(args) > 1:
    p = pathlib.Path(args[1])
elif len(args) > 2 and args[0] == "analyze":
    p = pathlib.Path(args[2])
else:
    p = None
if p is not None and p.is_file():
    entry.update(input_path=str(p), candidate=json.loads(p.read_text()))
with open({str(log)!r}, "a", encoding="utf-8") as f:
    f.write(json.dumps(entry) + "\\n")
mode = {mode!r}
if mode == "fail":
    print("forced native rejection", file=sys.stderr)
    sys.exit(73)
if mode == "bad-json" and args and args[0] == "analyze":
    print("not native JSON")
    sys.exit(0)
mutation = {mutation!r}
if mutation and args and args[0] == "validate":
    destination, content = mutation
    pathlib.Path(destination).write_bytes(content.encode())
p = subprocess.run([{binary!r}, *args], check=False)
sys.exit(p.returncode)
'''
    path.write_text(body, encoding="utf-8")
    path.chmod(0o700)
    return str(path), log


class OracleChecks(unittest.TestCase):
    def test_closed_canary(self):
        document = expected_candidate()
        self.assertEqual(len(document["entities"]), 3)
        self.assertEqual(document["entities"]["lo355-a"]["fields"]["lo355-code"]["value"], "00123")
        self.assertEqual(document["entities"]["lo355-b"]["fields"]["lo355-code"]["value"], "=1+1")
        self.assertEqual(document["entities"]["lo355-c"]["fields"]["lo355-quantity"]["value"], -3)

    def test_observation_is_not_a_semantic_document(self):
        observed = synthetic_observation()
        self.assertNotIn("entities", observed)
        self.assertEqual([c["kind"] for c in observed["rows"][1]], ["TEXT", "VALUE", "VALUE"])
        self.assertEqual(observed["source"]["selection"], SELECTION)

    def test_golden_mapping_has_no_location_identity(self):
        serialized = json.dumps(expected_candidate())
        self.assertNotIn("Inventory!", serialized)
        self.assertNotIn('"A2"', serialized)
        self.assertEqual(len({r["id"] for r in BASE_MAPPING["rows"]}), 3)


class NativePreflight(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = executable("TACHIKO_BIN")
        print("NATIVE BINARY:", call([cls.binary, "--version"]).stdout.strip())

    def test_01_real_admission_and_provider_free_queries(self):
        reports = native_reports(self.binary, expected_candidate(), "native-preflight")
        self.assertTrue(reports["document"])
        self.assertTrue(reports["validation"])

    def test_02_native_rejects_a_wrong_typed_candidate(self):
        bad = expected_candidate()
        bad["entities"]["lo355-a"]["fields"]["lo355-quantity"] = {"kind": "text", "value": "42.5"}
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "bad.ro"
            write_json(path, bad)
            self.assertNotEqual(call([self.binary, "validate", str(path)], success=False).returncode, 0)

    def test_03_real_materialize_reopen_is_already_available(self):
        with tempfile.TemporaryDirectory() as folder:
            source, project = Path(folder) / "candidate.ro", Path(folder) / "candidate.roproj"
            write_json(source, expected_candidate())
            before = source.read_bytes()
            call([self.binary, "roproj", "materialize", str(source), str(project)])
            call([self.binary, "roproj", "validate", str(project)])
            direct = json.loads(call([self.binary, "analyze", "document", str(source), "--source-state", "same-snapshot"]).stdout)
            reopened = json.loads(call([self.binary, "analyze", "document", str(project), "--source-state", "same-snapshot"]).stdout)
            self.assertEqual(direct, reopened)
            self.assertEqual(source.read_bytes(), before)


class NativeAcceptance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = executable("TACHIKO_BIN")
        # Independent native prerequisite must run BEFORE testing the missing seam.
        native_reports(cls.binary, expected_candidate(), "acceptance-prerequisite")

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lo355-acceptance-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.observed = synthetic_observation()
        self.mapping = copy.deepcopy(BASE_MAPPING)

    def run_bridge(self, *, mode="normal"):
        before = copy.deepcopy((self.observed, self.mapping))
        proxy, log = shim(self.directory, self.binary, mode=mode)
        result = bridge.analyze_snapshot(self.observed, self.mapping, tachiko_bin=proxy)
        self.assertEqual((self.observed, self.mapping), before, "bridge mutated caller input")
        self.assertIsInstance(result, dict)
        return result, log

    def assert_rejected(self, code, *, mode="normal"):
        result, log = self.run_bridge(mode=mode)
        self.assertEqual(result.get("status"), "rejected")
        self.assertEqual(result.get("code"), code, result)
        self.assertNotIn("candidate", result, "rejection leaked a partial candidate")
        self.assertNotIn("native", result, "rejection claimed successful native analysis")
        if log.exists():
            for line in log.read_text().splitlines():
                entry = json.loads(line)
                if "input_path" in entry:
                    self.assertFalse(Path(entry["input_path"]).exists(), "rejection leaked scratch candidate")
        self.assertTrue(any(e.get("code") == code for e in result.get("ledger", [])), result)

    def assert_analyzed(self, expected=None):
        expected = expected if expected is not None else expected_candidate()
        result, log = self.run_bridge()
        self.assertEqual(result.get("status"), "analyzed", result)
        self.assertEqual(result["candidate"], expected)
        self.assertEqual(result["source"], self.observed["source"])
        expected_reports = native_reports(self.binary, expected, self.observed["source"]["sha256"])
        self.assertEqual(result["native"], expected_reports)
        codes = {(e.get("classification"), e.get("code")) for e in result["ledger"]}
        self.assertTrue(SUCCESS_LEDGER <= codes, codes)
        trace = [json.loads(line) for line in log.read_text().splitlines()]
        operations = [tuple(e["args"][:2]) for e in trace]
        self.assertTrue(any(e["args"][0] == "validate" for e in trace))
        for name in ("document", "validation"):
            self.assertIn(("analyze", name), operations)
        for entry in trace:
            args = entry["args"]
            self.assertTrue(args == ["--version"] or args[0] == "validate"
                            or args[:2] in (["analyze", "document"], ["analyze", "validation"]), args)
            if "candidate" in entry:
                self.assertEqual(entry["candidate"], expected)
                self.assertFalse(Path(entry["input_path"]).exists(), "temporary candidate leaked")
        return result

    def test_01_exact_literals_real_native_calls_and_truthful_ledger(self):
        self.assert_analyzed()

    def test_02_changed_numbers_are_not_replaced_by_demo_constants(self):
        self.observed["rows"][1][1]["value"] = 7
        records = copy.deepcopy(RECORDS)
        records[0][1] = 7
        self.assert_analyzed(expected_candidate(records))

    def test_03_layout_and_label_movement_is_not_semantic_identity(self):
        self.observed["source"]["selection"].update(sheet="在庫 '2026", row=8, column=4)
        self.observed["rows"][1:] = list(reversed(self.observed["rows"][1:]))
        self.observed["rows"] = [[row[i] for i in (2, 0, 1)] for row in self.observed["rows"]]
        self.assert_analyzed()

    def test_04_formula_results_are_never_substituted_for_formulas(self):
        for formula, cached in (("=1+1", 2), ("=TRUE()", 1), ('=""', 0), ("=1/0", 0)):
            with self.subTest(formula=formula):
                self.observed["rows"][1][1] = {"kind": "FORMULA", "formula": formula,
                                                 "value": cached, "merged": False}
                self.assert_rejected("unsupported_cell")

    def test_05_type_coercion_and_blank_defaults_are_forbidden(self):
        for bad in ({"kind": "TEXT", "text": "42.5", "merged": False},
                    {"kind": "EMPTY", "merged": False},
                    {"kind": "VALUE", "value": True, "number_format": 16, "merged": False}):
            with self.subTest(cell=bad):
                self.observed["rows"][1][1] = bad
                self.assert_rejected("type_mismatch")

    def test_06_date_percent_currency_boolean_format_is_not_plain_number(self):
        for flag in (2, 4, 8, 32, 64, 128, 256, 1024, 2048):
            with self.subTest(format=flag):
                self.observed["rows"][1][1]["number_format"] = flag
                self.assert_rejected("unsupported_cell")

    def test_07_nonfinite_and_merged_content_is_not_admitted(self):
        for value in (float("nan"), float("inf"), -float("inf")):
            with self.subTest(value=str(value)):
                self.observed["rows"][1][1]["value"] = value
                self.assert_rejected("type_mismatch")
        self.observed = synthetic_observation()
        self.observed["rows"][1][1]["merged"] = True
        self.assert_rejected("unsupported_cell")

    def test_08_duplicate_or_unknown_source_keys_and_headers_reject(self):
        for row, col, text in ((0, 1, "code"), (0, 1, "unknown"), (2, 0, "00123"), (2, 0, "missing")):
            with self.subTest(row=row, column=col, text=text):
                self.observed = synthetic_observation()
                self.observed["rows"][row][col] = {"kind": "TEXT", "text": text, "merged": False}
                self.assert_rejected("mapping_mismatch")

    def test_09_no_truncation_to_old_rows_or_columns(self):
        self.observed["rows"].append(copy.deepcopy(self.observed["rows"][1]))
        self.observed["source"]["selection"]["height"] = 5
        self.assert_rejected("mapping_mismatch")
        self.observed = synthetic_observation()
        for row in self.observed["rows"]:
            row.append({"kind": "TEXT", "text": "unmapped", "merged": False})
        self.observed["source"]["selection"]["width"] = 4
        self.assert_rejected("mapping_mismatch")

    def test_10_invalid_mapping_never_silently_repairs_identity_or_type(self):
        for category in ("duplicate-id", "unknown-type", "duplicate-row", "missing-row"):
            with self.subTest(category=category):
                self.mapping = copy.deepcopy(BASE_MAPPING)
                if category == "duplicate-id":
                    self.mapping["columns"][1]["id"] = self.mapping["columns"][0]["id"]
                elif category == "unknown-type":
                    self.mapping["columns"][1]["type"] = "date"
                elif category == "duplicate-row":
                    self.mapping["rows"][1]["id"] = self.mapping["rows"][0]["id"]
                else:
                    self.mapping["rows"].pop()
                self.assert_rejected("invalid_mapping" if category != "missing-row" else "mapping_mismatch")

    def test_11_bounds_malformed_snapshot_and_live_source_are_rejected(self):
        for key, value in (("height", 0), ("width", 17), ("height", 102), ("row", -1), ("column", True)):
            with self.subTest(key=key, value=value):
                self.observed = synthetic_observation()
                self.observed["source"]["selection"][key] = value
                self.assert_rejected("invalid_snapshot")
        self.observed = synthetic_observation()
        self.observed["source"]["kind"] = "active-document"
        self.assert_rejected("invalid_snapshot")

    def test_12_native_rejection_cannot_be_presented_as_success(self):
        self.assert_rejected("native_failure", mode="fail")

    def test_13_invalid_native_json_cannot_be_presented_as_success(self):
        self.assert_rejected("native_failure", mode="bad-json")

    def test_14_user_defined_plain_number_format_is_presentation_only(self):
        self.observed["rows"][1][1]["number_format"] = 17
        self.assert_analyzed()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("oracle", "preflight", "native"), required=True)
    args = parser.parse_args()
    classes = {"oracle": [OracleChecks], "preflight": [NativePreflight],
               "native": [NativePreflight, NativeAcceptance]}[args.mode]
    suite = unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromTestCase(c) for c in classes)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)
