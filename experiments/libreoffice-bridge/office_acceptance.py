"""Actual saved Calc file → proposed bridge → real Rust. No simulated Office reads.

--mode preflight validates the synthetic Office ingress fixture without requiring
Rust. --mode acceptance requires both engines and the implemented bridge.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

from acceptance import call, executable, native_reports, shim, write_json, SUCCESS_LEDGER
from canary import (BASE_MAPPING, RECORDS, SELECTION, create_inventory, expected_candidate,
                    observe_selection, synthetic_observation)
from office_preflight import Office, digest, format_cell

ROOT = Path(__file__).resolve().parent
EVIDENCE = []


class OfficePreflight(unittest.TestCase):
    office_bin: str

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lo355-office-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.office = Office(self.office_bin, self.directory)
        self.addCleanup(self.office.close)

    def test_01_real_ods_and_xlsx_match_native_observation_fixture(self):
        for suffix in ("ods", "xlsx"):
            path = self.directory / f"inventory.{suffix}"
            selection = create_inventory(self.office, path)
            before = digest(path)
            doc = self.office.load(path, readonly=True)
            dirty = doc.isModified()
            observed = observe_selection(doc, selection, before)
            self.assertEqual(observed["rows"], synthetic_observation()["rows"])
            self.assertEqual(doc.isModified(), dirty)
            self.assertEqual(digest(path), before)
            EVIDENCE.append({"case": "real_" + suffix, "observation": observed})
            self.office.dispose(doc)

    def test_02_unsaved_memory_and_saved_source_are_distinct(self):
        path = self.directory / "inventory.ods"
        selection = create_inventory(self.office, path)
        before = digest(path)
        doc = self.office.load(path, readonly=True)
        doc.Sheets.getByIndex(0).getCellByPosition(1, 1).Value = 999
        self.assertTrue(doc.isModified())
        self.assertEqual(observe_selection(doc, selection, before)["rows"][1][1]["value"], 999)
        self.assertEqual(digest(path), before)
        self.office.dispose(doc)
        doc = self.office.load(path, readonly=True)
        observed = observe_selection(doc, selection, before)
        self.assertEqual(observed["rows"][1][1]["value"], 42.5)
        EVIDENCE.append({"case": "dirty_memory_is_not_saved_bytes", "observation": observed})
        self.office.dispose(doc)

    def test_03_moved_selection_retains_literals_not_coordinate_identity(self):
        path = self.directory / "moved.ods"
        selection = create_inventory(self.office, path, moved=True)
        doc = self.office.load(path, readonly=True)
        observed = observe_selection(doc, selection, digest(path))
        self.assertEqual(observed["rows"], synthetic_observation()["rows"])
        self.assertEqual(observed["source"]["selection"]["sheet"], "在庫 '2026")
        EVIDENCE.append({"case": "moved_selection", "observation": observed})
        self.office.dispose(doc)


class OfficeAcceptance(unittest.TestCase):
    office_bin: str
    binary: str

    @classmethod
    def setUpClass(cls):
        # Do not call a missing engine an expected bridge failure.
        cls.binary = executable("TACHIKO_BIN")
        native_reports(cls.binary, expected_candidate(), "office-prerequisite")

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lo355-full-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.office = Office(self.office_bin, self.directory)
        self.addCleanup(self.office.close)
        self.source = self.directory / "inventory.ods"
        self.selection = create_inventory(self.office, self.source)
        self.output = self.directory / "report.json"
        self.sentinel = self.directory / "existing-project.ro"
        self.sentinel.write_bytes(b"unrelated existing work must not change")
        self.request = {
            "source": {"kind": "saved-file", "path": str(self.source),
                       "expected_sha256": digest(self.source), "selection": self.selection},
            "mapping": copy.deepcopy(BASE_MAPPING), "output": str(self.output),
        }

    def invoke(self, *, mode="normal", mutation=None):
        request_file = self.directory / "request.json"
        write_json(request_file, self.request)
        protected = self.sentinel.read_bytes()
        before = self.source.read_bytes()
        proxy, log = shim(self.directory, self.binary, mode=mode, mutation=mutation)
        environment = os.environ | {"TACHIKO_BIN": proxy, "REFERENCE_OFFICE_BIN": self.office_bin}
        result = call([sys.executable, str(ROOT / "bridge.py"), "--request", str(request_file)],
                      success=False, env=environment)
        self.assertEqual(self.sentinel.read_bytes(), protected)
        if not mutation or mutation[0] != str(self.source):
            self.assertEqual(self.source.read_bytes(), before, "Office source changed")
        response = json.loads(result.stdout)
        EVIDENCE.append({"case": self.id(), "exit": result.returncode, "response": response})
        return result, response, log

    def reject(self, code, **options):
        result, response, _ = self.invoke(**options)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(response.get("code"), code, response)
        self.assertEqual(response.get("status"), "rejected", response)
        self.assertNotIn("candidate", response)
        self.assertNotIn("native", response)
        self.assertTrue(any(e.get("code") == code for e in response.get("ledger", [])))
        if code != "invalid_output":
            self.assertFalse(self.output.exists(), "failure left a user-visible artifact")

    def test_01_actual_saved_ods_and_xlsx_reach_real_rust(self):
        for suffix in ("ods", "xlsx"):
            with self.subTest(format=suffix):
                if suffix == "xlsx":
                    self.source = self.directory / "inventory.xlsx"
                    self.selection = create_inventory(self.office, self.source)
                    self.request["source"].update(path=str(self.source), expected_sha256=digest(self.source))
                    self.output = self.directory / "xlsx-report.json"
                    self.request["output"] = str(self.output)
                result, response, log = self.invoke()
                self.assertEqual(result.returncode, 0, response)
                self.assertEqual(response.get("status"), "analyzed", response)
                self.assertEqual(response["candidate"], expected_candidate())
                self.assertEqual(response["source"], {"kind": "saved-file", "sha256": digest(self.source),
                                                       "selection": self.selection})
                self.assertEqual(response["native"], native_reports(self.binary, expected_candidate(), digest(self.source)))
                self.assertEqual(json.loads(self.output.read_text()), response)
                self.assertTrue(SUCCESS_LEDGER <= {(e.get("classification"), e.get("code")) for e in response["ledger"]})
                trace = [json.loads(line) for line in log.read_text().splitlines()]
                self.assertTrue(any(e["args"][0] == "validate" for e in trace))
                for entry in trace:
                    if "candidate" in entry:
                        self.assertEqual(entry["candidate"], expected_candidate())
                        self.assertFalse(Path(entry["input_path"]).exists())

    def test_02_changed_saved_values_and_shifted_selection(self):
        source = self.directory / "changed.ods"
        records = copy.deepcopy(RECORDS)
        records[0][1] = 7
        selection = create_inventory(self.office, source, records=records, moved=True)
        self.source = source
        self.request["source"].update(path=str(source), expected_sha256=digest(source), selection=selection)
        result, response, _ = self.invoke()
        self.assertEqual(result.returncode, 0, response)
        self.assertEqual(response["candidate"], expected_candidate(records))
        self.assertEqual(response["source"]["selection"], selection)

    def test_03_active_document_is_not_silently_replaced_by_disk(self):
        self.request["source"]["kind"] = "active-document"
        self.reject("saved_file_required")

    def test_04_stale_source_digest_rejects(self):
        self.request["source"]["expected_sha256"] = "0" * 64
        self.reject("source_changed")

    def test_05_native_failure_leaves_no_report(self):
        self.reject("native_failure", mode="fail")

    def test_06_existing_report_is_preserved(self):
        self.output.write_bytes(b"existing-output")
        self.reject("invalid_output")
        self.assertEqual(self.output.read_bytes(), b"existing-output")

    def test_07_symlink_source_is_rejected_without_touching_target(self):
        link = self.directory / "link.ods"
        link.symlink_to(self.source)
        self.request["source"]["path"] = str(link)
        self.reject("invalid_input")
        self.assertTrue(link.is_symlink())

    def test_08_real_formula_percent_blank_and_merged_cell_reject(self):
        original = self.source
        original_digest = digest(original)
        for index, kind in enumerate(("formula", "percent", "blank", "merged")):
            with self.subTest(kind=kind):
                # This fixture must persist the unsupported construct into new saved
                # bytes. A ReadOnly-loaded document may accept in-memory API edits
                # while storeToURL writes the unchanged representation.
                doc = self.office.load(original, readonly=False)
                item = doc.Sheets.getByIndex(0).getCellByPosition(1, 1)
                if kind == "formula":
                    item.Formula = "=42.5"
                elif kind == "percent":
                    format_cell(self.office, doc, "B2", "0.00%")
                elif kind == "blank":
                    item.String = ""
                else:
                    doc.Sheets.getByIndex(0).getCellRangeByName("B2:C2").merge(True)
                self.source = self.directory / f"unsupported-{index}.ods"
                self.office.save_copy(doc, self.source)
                self.office.dispose(doc)
                self.assertEqual(digest(original), original_digest, "fixture construction changed the source")

                # Qualify the saved fixture independently before exercising the
                # bridge so an Office-version-specific save behavior cannot turn
                # the intended rejection into a false production failure.
                qualified = self.office.load(self.source, readonly=True)
                saved_item = qualified.Sheets.getByIndex(0).getCellByPosition(1, 1)
                if kind == "formula":
                    self.assertEqual(saved_item.Type.value, "FORMULA")
                    self.assertTrue(saved_item.Formula.startswith("="))
                elif kind == "percent":
                    self.assertIn("%", saved_item.String)
                elif kind == "blank":
                    self.assertEqual(saved_item.Type.value, "EMPTY")
                else:
                    saved_range = qualified.Sheets.getByIndex(0).getCellRangeByName("B2:C2")
                    self.assertTrue(bool(saved_item.IsMerged) or bool(saved_range.IsMerged))
                self.office.dispose(qualified)

                self.output = self.directory / f"unsupported-{index}-report.json"
                self.request["output"] = str(self.output)
                self.request["source"].update(path=str(self.source), expected_sha256=digest(self.source))
                self.reject("type_mismatch" if kind == "blank" else "unsupported_cell")

    def test_09_concurrent_source_change_does_not_publish_stale_report(self):
        self.reject("source_changed", mutation=(str(self.source), "external-change"))
        self.assertEqual(self.source.read_bytes(), b"external-change", "do not fake rollback of an external write")

    def test_10_destination_race_preserves_external_winner(self):
        self.reject("invalid_output", mutation=(str(self.output), "external-output-winner"))
        self.assertEqual(self.output.read_bytes(), b"external-output-winner")

    def test_11_oversized_input_is_rejected_before_office_load(self):
        self.source.write_bytes(b"x" * (2 * 1024 * 1024 + 1))
        self.request["source"]["expected_sha256"] = digest(self.source)
        self.reject("invalid_input")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("preflight", "acceptance"), required=True)
    parser.add_argument("--evidence", required=True, type=Path, help="new absent JSON file")
    args = parser.parse_args()
    if args.evidence.exists() or args.evidence.is_symlink():
        parser.error("evidence path already exists")
    office_bin = executable("REFERENCE_OFFICE_BIN")
    # Missing pyuno is environment failure, never a skip/RED.
    import uno  # noqa: F401

    OfficePreflight.office_bin = office_bin
    OfficeAcceptance.office_bin = office_bin
    classes = [OfficePreflight] if args.mode == "preflight" else [OfficePreflight, OfficeAcceptance]
    suite = unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromTestCase(c) for c in classes)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {"mode": args.mode, "tests": result.testsRun, "failures": len(result.failures),
              "errors": len(result.errors), "skips": len(result.skipped), "passed": result.wasSuccessful(),
              "office": call([office_bin, "--version"]).stdout.strip(),
              "python": sys.version, "script_sha256": digest(Path(__file__)),
              "repository_head": os.environ.get("LIBREOFFICE_BRIDGE_HEAD", "uncommitted preparation"),
              "native": "NOT_RUN" if args.mode == "preflight" else os.environ.get("TACHIKO_BIN"),
              "observations": EVIDENCE}
    with args.evidence.open("x", encoding="utf-8") as output:
        json.dump(report, output, ensure_ascii=False, indent=2)
        output.write("\n")
    raise SystemExit(0 if result.wasSuccessful() else 1)