"""Issue #355: synthetic LibreOffice-only qualification, NOT Tachiko acceptance.

Requires an installed LibreOffice and its ABI-compatible Python/pyuno. Never
opens caller-provided documents or attaches to an existing Office instance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import threading
import unittest
import uuid
from datetime import date


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Office:
    def __init__(self, binary: str, artifacts: Path):
        import uno

        self.uno = uno
        self.artifacts = artifacts
        self.temp = tempfile.TemporaryDirectory(prefix="tachiko-lo355-")
        self.process = None
        self.documents = []
        self.watchdog = None
        self.timed_out = False
        self.log = (artifacts / "office.log").open("wb")
        try:
            endpoint = f"pipe,name=tachiko355_{uuid.uuid4().hex};urp;StarOffice.ComponentContext"
            profile = (Path(self.temp.name) / "profile").as_uri()
            self.process = subprocess.Popen(
                [binary, f"-env:UserInstallation={profile}", "--headless", "--nologo",
                 "--nodefault", "--norestore", "--nofirststartwizard", f"--accept={endpoint}"],
                stdin=subprocess.DEVNULL, stdout=self.log, stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            self.watchdog = threading.Timer(90, self.expire)
            self.watchdog.daemon = True
            self.watchdog.start()
            local = uno.getComponentContext()
            resolver = local.ServiceManager.createInstanceWithContext(
                "com.sun.star.bridge.UnoUrlResolver", local
            )
            deadline = time.monotonic() + 20
            while True:
                try:
                    self.context = resolver.resolve(f"uno:{endpoint}")
                    break
                except uno.getClass("com.sun.star.connection.NoConnectException"):
                    if self.process.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("ENVIRONMENT_UNVERIFIED: private UNO connection failed")
                    time.sleep(0.2)
            self.desktop = self.context.ServiceManager.createInstanceWithContext(
                "com.sun.star.frame.Desktop", self.context
            )
        except BaseException:
            self.close()
            raise

    def props(self, **values):
        result = []
        for name, value in values.items():
            item = self.uno.createUnoStruct("com.sun.star.beans.PropertyValue")
            item.Name, item.Value = name, value
            result.append(item)
        return tuple(result)

    def load(self, path: Path | None = None, *, readonly: bool = False):
        url = path.as_uri() if path else "private:factory/scalc"
        doc = self.desktop.loadComponentFromURL(
            url, "_blank", 0, self.props(
                Hidden=True, ReadOnly=readonly, PickListEntry=False,
                MacroExecutionMode=self.uno.getConstantByName(
                    "com.sun.star.document.MacroExecMode.NEVER_EXECUTE"),
                UpdateDocMode=self.uno.getConstantByName(
                    "com.sun.star.document.UpdateDocMode.NO_UPDATE"),
            )
        )
        require(doc is not None, "Office did not return a document")
        self.documents.append(doc)
        return doc

    def save_copy(self, doc, path: Path) -> None:
        if path.exists() or path.is_symlink():
            raise FileExistsError(path)
        filter_name = "calc8" if path.suffix == ".ods" else "Calc MS Excel 2007 XML"
        doc.storeToURL(path.as_uri(), self.props(FilterName=filter_name, Overwrite=False))
        require(path.is_file() and path.stat().st_size > 0, "Missing saved artifact")

    def dispose(self, doc) -> None:
        doc.dispose()
        self.documents.remove(doc)

    def expire(self) -> None:
        self.timed_out = True
        if self.process is not None:
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def close(self) -> None:
        if self.watchdog is not None:
            self.watchdog.cancel()
        # Kill only our process group; no global pkill, user profile or active session.
        if self.process is not None:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait(timeout=5)
        self.log.close()
        self.temp.cleanup()


def cell(doc, address: str, sheet: int = 0):
    return doc.Sheets.getByIndex(sheet).getCellRangeByName(address)


def observe(doc, address: str, sheet: int = 0) -> dict:
    item = cell(doc, address, sheet)
    return {"kind": item.Type.value, "formula": item.Formula, "value": item.Value,
            "display": item.String, "error": item.Error}


def format_cell(office: Office, doc, address: str, code: str) -> None:
    locale = office.uno.createUnoStruct("com.sun.star.lang.Locale")
    locale.Language, locale.Country = "en", "US"
    key = doc.NumberFormats.queryKey(code, locale, True)
    if key == -1:
        key = doc.NumberFormats.addNew(code, locale)
    cell(doc, address).NumberFormat = key


def create_fixture(office: Office, name: str):
    doc = office.load()
    if name == "literals":
        for address, text in {"A1": "00123", "A2": "=1+1", "A3": "台灣／日本"}.items():
            cell(doc, address).String = text
        cell(doc, "A4").Value = 42.5
        cell(doc, "A5").Formula = "=TRUE()"
        cell(doc, "A6").Formula = "=FALSE()"
        cell(doc, "A8").Formula = '=""'
        cell(doc, "A9").Value = 0
    elif name == "plan":
        doc.Sheets.getByIndex(0).Name = "Assumptions"
        doc.Sheets.insertNewByName("P&L", 1)
        cell(doc, "A1").String = "Tax"
        cell(doc, "A2").Value = 0.25
        for row, (label, revenue, cost) in enumerate(
            [("January", 1200, 400), ("February", 1600, 800), ("March", 2000, 1000)], 2
        ):
            cell(doc, f"A{row}", 1).String = label
            cell(doc, f"B{row}", 1).Value = revenue
            cell(doc, f"C{row}", 1).Value = cost
            cell(doc, f"D{row}", 1).Formula = f"=B{row}-C{row}"
            cell(doc, f"E{row}", 1).Formula = f"=D{row}*(1-$Assumptions.$A$2)"
    elif name == "display":
        settings = doc.NumberFormatSettings
        origin = office.uno.createUnoStruct("com.sun.star.util.Date")
        origin.Year, origin.Month, origin.Day = 1899, 12, 30
        settings.NullDate = origin
        cell(doc, "A1").Value = (date(2026, 9, 9) - date(1899, 12, 30)).days
        format_cell(office, doc, "A1", "YYYY-MM-DD")
        cell(doc, "A2").Value = 0.25
        format_cell(office, doc, "A2", "0.00%")
        cell(doc, "A3").Formula = "=1/0"
    else:
        raise ValueError(name)
    doc.calculateAll()
    return doc


def check_fixture(doc, name: str, *, half: bool = False) -> dict:
    doc.calculateAll()
    if name == "literals":
        observed = {a: observe(doc, a) for a in [f"A{i}" for i in range(1, 10)]}
        for address, text in {"A1": "00123", "A2": "=1+1", "A3": "台灣／日本"}.items():
            require(observed[address]["kind"] == "TEXT", f"Coerced text at {address}")
            require(observed[address]["display"] == text, f"Changed text at {address}")
        require(observed["A4"]["value"] == 42.5, "Numeric value changed")
        for address, value in {"A5": 1, "A6": 0}.items():
            require(observed[address]["kind"] == "FORMULA", "Lost Boolean formula")
            require(observed[address]["value"] == value, "Wrong Boolean result")
        require(observed["A7"]["kind"] == "EMPTY", "Blank became a value")
        require(observed["A8"]["kind"] == "FORMULA" and observed["A8"]["display"] == "",
                "Empty formula result is not a blank cell")
        require(observed["A9"]["kind"] == "VALUE" and observed["A9"]["value"] == 0,
                "Zero became a blank")
    elif name == "plan":
        observed = {f"{col}{row}": observe(doc, f"{col}{row}", 1)
                    for row in range(2, 5) for col in "DE"}
        for row, gross, net in zip(range(2, 5), [800, 800, 1000],
                                   [400, 400, 500] if half else [600, 600, 750]):
            for col, expected in [("D", gross), ("E", net)]:
                current = observed[f"{col}{row}"]
                require(current["kind"] == "FORMULA", "Formula flattened to cached number")
                require(current["error"] == 0 and current["value"] == expected,
                        f"Wrong independent arithmetic at {col}{row}: {current}")
    else:
        observed = {a: observe(doc, a) for a in ["A1", "A2", "A3", "A4"]}
        require(observed["A1"]["display"] == "2026-09-09", "Date display changed")
        require(observed["A1"]["value"] == 46274, "Date serial/origin changed")
        require(observed["A2"]["value"] == 0.25 and observed["A2"]["display"] == "25.00%",
                "Percent display was confused with its stored value")
        require(observed["A3"]["kind"] == "FORMULA" and observed["A3"]["error"] != 0,
                "Formula error silently became a value")
        require(observed["A4"]["kind"] == "EMPTY", "Error and blank conflated")
    return observed


class Qualification(unittest.TestCase):
    office: Office
    artifacts: Path
    evidence: list

    def roundtrip(self, name: str) -> None:
        doc = create_fixture(self.office, name)
        record = {"fixture": name, "stages": [{"stage": "original", "cells": check_fixture(doc, name)}]}
        prior = {}
        for index, extension in enumerate(["ods", "xlsx", "ods"]):
            path = self.artifacts / f"{name}-{index}.{extension}"
            self.office.save_copy(doc, path)
            self.office.dispose(doc)
            doc = self.office.load(path, readonly=True)
            stage = {"stage": f"reopen-{extension}", "path": path.name,
                     "sha256": digest(path), "cells": check_fixture(doc, name)}
            for previous, expected in prior.items():
                self.assertEqual(digest(previous), expected, "Earlier source artifact changed")
            prior[path] = digest(path)
            record["stages"].append(stage)
        if name == "plan":
            cell(doc, "A2").Value = 0.5
            record["changed_input"] = check_fixture(doc, name, half=True)
            changed = self.artifacts / "plan-changed.xlsx"
            self.office.save_copy(doc, changed)
            self.office.dispose(doc)
            doc = self.office.load(changed, readonly=True)
            record["changed_reopened"] = check_fixture(doc, name, half=True)
            record["changed_sha256"] = digest(changed)
            for previous, expected in prior.items():
                self.assertEqual(digest(previous), expected)
        self.office.dispose(doc)
        self.evidence.append(record)

    def test_01_literal_types_roundtrip(self):
        self.roundtrip("literals")

    def test_02_live_formula_roundtrip_and_recalculation(self):
        self.roundtrip("plan")

    def test_03_date_percent_error_roundtrip(self):
        self.roundtrip("display")

    def test_04_readonly_flag_is_not_api_security(self):
        doc = create_fixture(self.office, "literals")
        path = self.artifacts / "readonly.ods"
        self.office.save_copy(doc, path)
        self.office.dispose(doc)
        before = digest(path)
        doc = self.office.load(path, readonly=True)
        self.assertTrue(doc.isReadonly())
        cell(doc, "A1").String = "changed-in-memory"
        self.assertEqual(cell(doc, "A1").String, "changed-in-memory")
        self.assertEqual(digest(path), before)
        self.office.dispose(doc)
        self.evidence.append({"check": "ReadOnly allows API mutation; disk unchanged without save",
                              "source_sha256": before})

    def test_05_existing_output_is_preserved(self):
        doc = create_fixture(self.office, "literals")
        path = self.artifacts / "existing.ods"
        path.write_bytes(b"existing-output-sentinel")
        before = digest(path)
        with self.assertRaises(FileExistsError):
            self.office.save_copy(doc, path)
        self.assertEqual(digest(path), before)
        self.office.dispose(doc)
        self.evidence.append({"check": "test harness refuses existing output", "sha256": before})

    def test_06_observation_preserves_saved_source_and_dirty_state(self):
        doc = create_fixture(self.office, "literals")
        path = self.artifacts / "observation.ods"
        self.office.save_copy(doc, path)
        self.office.dispose(doc)
        doc = self.office.load(path, readonly=True)
        before = digest(path)
        dirty = doc.isModified()
        first = {f"A{i}": observe(doc, f"A{i}") for i in range(1, 10)}
        second = {f"A{i}": observe(doc, f"A{i}") for i in range(1, 10)}
        self.assertEqual(first, second)
        self.assertEqual(doc.isModified(), dirty)
        self.assertEqual(digest(path), before)
        self.office.dispose(doc)
        self.evidence.append({"check": "read observations preserve bytes and dirty state",
                              "source_sha256": before})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="new absent scratch directory")
    parser.add_argument("--office", default=shutil.which("soffice"))
    args = parser.parse_args()
    if not args.office:
        parser.error("ENVIRONMENT_UNVERIFIED: installed soffice required")
    output = args.output.absolute()
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    version = subprocess.run([args.office, "--version"], check=True, capture_output=True,
                             text=True, timeout=15).stdout.strip()
    report = {"issue": 355, "scope": "LibreOffice-only synthetic preflight",
              "office": version, "platform": platform.platform(),
              "python": sys.version, "script_sha256": digest(Path(__file__)),
              "native_tachiko": "NOT_RUN", "hostile_input_sandbox": "NOT_TESTED",
              "gui_macos": "NOT_TESTED", "observations": []}
    office = None
    try:
        office = Office(args.office, output)
        Qualification.office, Qualification.artifacts = office, output
        Qualification.evidence = report["observations"]
        result = unittest.TextTestRunner(verbosity=2).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(Qualification))
        passed = result.wasSuccessful() and not office.timed_out
        report.update(tests_run=result.testsRun, failures=len(result.failures),
                      errors=len(result.errors), skips=len(result.skipped),
                      passed=passed)
        return 0 if passed else 1
    except BaseException as exc:
        report.update(passed=False, environment_error=f"{type(exc).__name__}: {exc}")
        raise
    finally:
        if office:
            office.close()
            report["watchdog_expired"] = office.timed_out
        (output / "evidence.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
