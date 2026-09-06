"""Steward acceptance for the disposable open-sheet export seam; stdlib only.

Modes are distinct evidence classes. Self-check is NOT runtime acceptance.
No missing tool, missing adapter, or missing office engine is skipped or RED.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import shutil
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parent
FIXTURE = json.loads((ROOT / "canary.json").read_text(encoding="utf-8"))
NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
UPSTREAM = "6ab72ddb50bb87dad11cc321d87b5e9bcc3c5689"


def q(name: str) -> str:
    return f"{{{NS}}}{name}"


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run(command: list[str], *, success: bool = True) -> subprocess.CompletedProcess:
    result = subprocess.run(command, text=True, capture_output=True, timeout=180, check=False)
    if success and result.returncode != 0:
        raise AssertionError(f"Command failed: {command!r}\n{result.stderr}")
    return result


def executable(variable: str) -> str:
    value = os.environ.get(variable)
    found = shutil.which(value) if value else None
    if found is None:
        raise RuntimeError(f"ENVIRONMENT UNVERIFIED: set {variable} to an executable")
    return found


def driver_command() -> list[str]:
    value = json.loads(os.environ.get("OPEN_SHEET_POC_COMMAND", "null"))
    if not isinstance(value, list) or not value or not all(isinstance(x, str) for x in value):
        raise RuntimeError("MISSING SEAM: OPEN_SHEET_POC_COMMAND must be a JSON argv array")
    return value


def tree_digest(path: Path) -> str:
    digest = hashlib.sha256()
    for item in sorted(path.rglob("*")):
        if item.is_file():
            digest.update(item.relative_to(path).as_posix().encode() + b"\0")
            digest.update(hashlib.sha256(item.read_bytes()).digest())
    return digest.hexdigest()


def a1(row: int, column: int) -> str:
    letters = ""
    column += 1
    while column:
        column, digit = divmod(column - 1, 26)
        letters = chr(65 + digit) + letters
    return f"{letters}{row + 1}"


def locations(layout: dict) -> dict[tuple[str, str], tuple[str, str]]:
    result = {}
    for sheet in layout["sheets"]:
        for row, entity in enumerate(sheet["rows"]):
            for column, field in enumerate(sheet["columns"]):
                target = (entity, field)
                if target in result:
                    raise ValueError("ambiguous projection")
                result[target] = (
                    sheet["name"],
                    a1(sheet["header_row"] + 1 + row, sheet["left_column"] + column),
                )
    return result


def semantic_tree(expression: dict) -> tuple:
    op, args = expression["op"], expression["args"]
    if op == "number":
        return ("number", args)
    if op == "reference":
        return ("reference", args["entity"], args["field"])
    return (op, semantic_tree(args["left"]), semantic_tree(args["right"]))


# Test-only bounded parser: compare expression meaning, not serializer whitespace.
# It deliberately has no evaluator and cannot be used by the production adapter.
TOKEN = re.compile(
    r"\s*(?:(?P<ref>(?:'(?:[^']|'')*'!|[A-Za-z_][A-Za-z0-9_.]*!)?"
    r"\$?[A-Z]+\$?[1-9][0-9]*)|(?P<num>(?:\d+(?:\.\d*)?|\.\d+)"
    r"(?:[eE][+-]?\d+)?)|(?P<fn>MIN|MAX)|(?P<op>[+*/(),-]))"
)


def formula_tree(text: str, sheet: str, inverse: dict) -> tuple:
    tokens, position = [], 0
    text = text.strip().removeprefix("=")
    while position < len(text):
        match = TOKEN.match(text, position)
        if match is None:
            raise AssertionError(f"Out-of-profile formula at {text[position:]!r}")
        tokens.append((match.lastgroup, match.group(match.lastgroup)))
        position = match.end()
    index = 0

    def take(value: str) -> None:
        nonlocal index
        if index >= len(tokens) or tokens[index][1] != value:
            raise AssertionError(f"Expected {value!r} in {text!r}")
        index += 1

    def expression(minimum: int = 0) -> tuple:
        nonlocal index
        if index >= len(tokens):
            raise AssertionError("Incomplete formula")
        kind, value = tokens[index]
        index += 1
        if kind == "num":
            left = ("number", float(value))
        elif kind == "ref":
            owner, address = (value.rsplit("!", 1) if "!" in value else (sheet, value))
            if owner.startswith("'"):
                owner = owner[1:-1].replace("''", "'")
            target = inverse.get((owner, address.replace("$", "")))
            if target is None:
                raise AssertionError(f"Formula targets unprojected cell: {value}")
            left = ("reference", *target)
        elif value == "(":
            left = expression()
            take(")")
        elif value == "-":
            operand = expression(30)
            left = (("number", -operand[1]) if operand[0] == "number"
                    else ("subtract", ("number", 0), operand))
        elif kind == "fn":
            take("(")
            first = expression()
            take(",")
            second = expression()
            take(")")
            left = ("minimum" if value == "MIN" else "maximum", first, second)
        else:
            raise AssertionError(f"Unexpected token {value!r}")
        operators = {"+": (10, "add"), "-": (10, "subtract"),
                     "*": (20, "multiply"), "/": (20, "divide")}
        while index < len(tokens) and tokens[index][1] in operators:
            priority, name = operators[tokens[index][1]]
            if priority < minimum:
                break
            index += 1
            left = (name, left, expression(priority + 1))
        return left

    result = expression()
    if index != len(tokens):
        raise AssertionError(f"Trailing formula tokens: {tokens[index:]}")
    return result


class WorkbookXml:
    """Independent OOXML observation; does not use open-sheet/ExcelJS to read back."""

    def __init__(self, path: Path):
        with zipfile.ZipFile(path) as archive:
            workbook = ET.fromstring(archive.read("xl/workbook.xml"))
            rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
            targets = {r.attrib["Id"]: r.attrib["Target"] for r in rels}
            self.sheets = {}
            for sheet in workbook.find(q("sheets")):
                target = targets[sheet.attrib[f"{{{REL}}}id"]]
                member = target.lstrip("/") if target.startswith("/") else posixpath.normpath(
                    posixpath.join("xl", target)
                )
                self.sheets[sheet.attrib["name"]] = ET.fromstring(archive.read(member))
            self.styles = ET.fromstring(archive.read("xl/styles.xml"))
            self.strings = []
            if "xl/sharedStrings.xml" in archive.namelist():
                strings = ET.fromstring(archive.read("xl/sharedStrings.xml"))
                self.strings = ["".join(s.itertext()) for s in strings]

    def cell(self, sheet: str, address: str) -> ET.Element:
        cell = self.sheets[sheet].find(f".//{q('c')}[@r='{address}']")
        if cell is None:
            raise AssertionError(f"Missing cell {sheet}!{address}")
        return cell

    def value(self, cell: ET.Element):
        value = cell.findtext(q("v"))
        kind = cell.attrib.get("t", "n")
        if kind == "s":
            return self.strings[int(value)]
        if kind == "inlineStr":
            return "".join(cell.find(q("is")).itertext())
        if kind == "b":
            return value == "1"
        if kind == "e":
            raise AssertionError(f"Office calculation error: {value}")
        if kind == "str":
            return value
        return float(value) if value not in (None, "") else None

    def style(self, cell: ET.Element) -> ET.Element:
        return self.styles.find(q("cellXfs"))[int(cell.attrib.get("s", "0"))]

    def number_format(self, cell: ET.Element) -> str:
        code = int(self.style(cell).attrib.get("numFmtId", "0"))
        builtin = {0: "General", 4: "#,##0.00", 10: "0.00%"}
        for entry in self.styles.findall(f"{q('numFmts')}/{q('numFmt')}"):
            if int(entry.attrib["numFmtId"]) == code:
                return entry.attrib["formatCode"]
        return builtin.get(code, f"builtin:{code}")


def verify_book(book: WorkbookXml, document: dict, layout: dict, *, cached: bool = False) -> None:
    where = locations(layout)
    inverse = {cell: target for target, cell in where.items()}
    if list(book.sheets) != [s["name"] for s in layout["sheets"]]:
        raise AssertionError("Sheet membership/order drift")
    for target, (sheet, address) in where.items():
        original = document["entities"][target[0]]["fields"][target[1]]
        cell = book.cell(sheet, address)
        formula = cell.find(q("f"))
        if original["kind"] == "formula":
            if formula is None or not formula.text:
                raise AssertionError(f"Baked value instead of live formula: {target}")
            if formula_tree(formula.text, sheet, inverse) != semantic_tree(original["value"]):
                raise AssertionError(f"Changed formula meaning: {target}: {formula.text}")
            if not cached and cell.findtext(q("v")) not in (None, ""):
                raise AssertionError("Cached formula results invalidate independent recalculation")
        else:
            if formula is not None or book.value(cell) != original["value"]:
                raise AssertionError(f"Changed scalar: {target}")
    actual_formulas = sum(len(s.findall(f".//{q('f')}")) for s in book.sheets.values())
    expected_formulas = sum(
        document["entities"][e]["fields"][f]["kind"] == "formula" for e, f in where
    )
    if actual_formulas != expected_formulas:
        raise AssertionError("Unexpected extra or missing formulas")
    for sheet in layout["sheets"]:
        for index, field in enumerate(sheet["columns"]):
            header = book.cell(sheet["name"], a1(sheet["header_row"], sheet["left_column"] + index))
            expected = document["schemas"][sheet["schema"]]["fields"][field]["key"]
            if book.value(header) != expected:
                raise AssertionError("Header drift")
            font = book.styles.find(q("fonts"))[int(book.style(header).attrib.get("fontId", "0"))]
            bold = font.find(q("b"))
            if bold is None or bold.attrib.get("val", "1") in ("0", "false"):
                raise AssertionError("Header is not visibly distinguished")
            for entity in sheet["rows"]:
                requested = sheet["number_formats"].get(field)
                if requested and book.number_format(book.cell(*where[(entity, field)])) != requested:
                    raise AssertionError(f"Number-format loss at {(entity, field)}")


class OracleSelfChecks(unittest.TestCase):
    def test_known_arithmetic_is_exact(self):
        self.assertEqual(FIXTURE["expected"]["base"], {
            "e-jan.f-gross": 800, "e-jan.f-net": 600,
            "e-feb.f-gross": 800, "e-feb.f-net": 600,
            "e-mar.f-gross": 1000, "e-mar.f-net": 750,
        })
        self.assertEqual((1200 - 400) * (1 - 0.25), 600)
        self.assertEqual((2000 - 1000) * (1 - 0.5), 500)

    def test_projection_moves_without_new_identity(self):
        before = locations(FIXTURE["layouts"]["base"])
        after = locations(FIXTURE["layouts"]["moved"])
        self.assertEqual(set(before), set(after))
        self.assertEqual(len(before), 16)
        self.assertTrue(all(before[key] != after[key] for key in before))

    def test_expression_oracle_rejects_wrong_operator(self):
        inverse = {("P&L", "B2"): ("e-jan", "f-revenue"),
                   ("P&L", "C2"): ("e-jan", "f-cost")}
        expected = semantic_tree(FIXTURE["document"]["entities"]["e-jan"]["fields"]["f-gross"]["value"])
        self.assertEqual(formula_tree("(B2-C2)", "P&L", inverse), expected)
        self.assertNotEqual(formula_tree("(B2+C2)", "P&L", inverse), expected)

    def test_expression_oracle_preserves_parentheses(self):
        self.assertNotEqual(formula_tree("1-(2*3)", "S", {}), formula_tree("(1-2)*3", "S", {}))

    def test_quoted_sheet_and_absolute_reference(self):
        inverse = {("假設 '2026", "B4"): ("e-tax", "f-rate")}
        self.assertEqual(formula_tree("'假設 ''2026'!$B$4", "計畫", inverse),
                         ("reference", "e-tax", "f-rate"))

    def test_unprojected_reference_is_not_zero(self):
        with self.assertRaises(AssertionError):
            formula_tree("A99", "P&L", {})

    def test_unknown_function_is_not_accepted(self):
        with self.assertRaises(AssertionError):
            formula_tree("NOW()", "P&L", {})

    def test_ambiguous_projection_is_detected(self):
        layout = copy.deepcopy(FIXTURE["layouts"]["base"])
        layout["sheets"][1]["rows"].append("e-jan")
        with self.assertRaises(ValueError):
            locations(layout)


class RuntimePreflight(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cli = executable("TACHIKO_BIN")
        cls.temp = tempfile.TemporaryDirectory(prefix="tachiko-open-sheet-")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.directory = Path(cls.temp.name)
        cls.model = cls.directory / "model.ro"
        write_json(cls.model, FIXTURE["document"])
        cls.project = cls.directory / "source.roproj"
        run([cls.cli, "validate", str(cls.model)])
        run([cls.cli, "roproj", "materialize", str(cls.model), str(cls.project)])
        run([cls.cli, "roproj", "validate", str(cls.project)])

    def facts(self, source: Path, target: str) -> dict:
        return json.loads(run([self.cli, "formula", "inspect", str(source), target]).stdout)

    def revision(self, source: Path) -> str:
        return self.facts(source, "e-jan.f-net")["source_revision"]

    def test_fixture_reaches_real_rust_formula_boundary(self):
        revisions = set()
        before = tree_digest(self.project)
        for target, expected in FIXTURE["expected"]["base"].items():
            result = self.facts(self.project, target)
            revisions.add(result["source_revision"])
            outcome = result["outcome"]
            self.assertEqual(outcome["kind"], "formula")
            self.assertEqual(outcome["calculation"], {"kind": "value", "value": expected})
            entity, field = target.split(".")
            original = FIXTURE["document"]["entities"][entity]["fields"][field]["value"]
            self.assertEqual(outcome["expression"], original)
        self.assertEqual(len(revisions), 1)
        self.assertEqual(tree_digest(self.project), before)

    def test_existing_rust_edit_and_rename_preserve_bound_targets(self):
        changed = self.directory / "changed.ro"
        run([self.cli, "set", str(self.project), "tax.rate", "0.5", "--output", str(changed)])
        for target, expected in FIXTURE["expected"]["tax_half"].items():
            self.assertEqual(self.facts(changed, target)["outcome"]["calculation"],
                             {"kind": "value", "value": expected})
        renamed = self.directory / "renamed.ro"
        run([self.cli, "entity", "rename", str(changed), "jan", "renamed_jan", "--output", str(renamed)])
        self.assertNotEqual(self.revision(self.project), self.revision(changed))
        self.assertEqual(self.facts(changed, "e-jan.f-net")["outcome"]["expression"],
                         self.facts(renamed, "e-jan.f-net")["outcome"]["expression"])


    def test_public_upstream_writer_without_evaluator(self):
        node = shutil.which("node")
        if node is None:
            raise RuntimeError("ENVIRONMENT UNVERIFIED: Node is missing")
        output = self.directory / "upstream-probe.xlsx"
        report = json.loads(run([node, str(ROOT / "public-seam-probe.mjs"),
                                 "--output", str(output)]).stdout)
        self.assertEqual(report["package"], "@open-sheet/core")
        self.assertEqual(report["version"], "0.2.0")
        workbook = WorkbookXml(output)
        self.assertEqual(workbook.value(workbook.cell("Probe", "A1")), 1)
        formula = workbook.cell("Probe", "A2")
        self.assertEqual(formula_tree(formula.findtext(q("f")), "Probe", {}),
                         ("add", ("number", 1), ("number", 2)))
        self.assertIn(formula.findtext(q("v")), (None, ""))


class ExportAcceptance(RuntimePreflight):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.driver = driver_command()

    def export(self, *, layout=None, source=None, expected_revision=None, output=None, success=True):
        source = source or self.project
        layout = layout or FIXTURE["layouts"]["base"]
        with tempfile.TemporaryDirectory(dir=self.directory) as request_dir:
            directory = Path(request_dir)
            output = output or directory / "export.xlsx"
            request = {"source": str(source), "expected_source_revision": (
                expected_revision or self.revision(source)), "projection": layout, "output": str(output)}
            path = directory / "request.json"
            write_json(path, request)
            existed = output.exists()
            before_output = output.read_bytes() if existed else None
            before = tree_digest(source)
            process = run([*self.driver, "--request", str(path)], success=success)
            self.assertEqual(tree_digest(source), before, "Export mutated canonical source")
            evidence = json.loads(process.stdout)
            if success:
                self.assertEqual(evidence["status"], "exported")
                self.assertEqual(evidence["source_revision"], request["expected_source_revision"])
                self.assertEqual(evidence["writer"], {"name": "open-sheet", "source_commit": UPSTREAM})
                self.assertIn("ledger", evidence)
                book = WorkbookXml(output)
                retained = self.directory / (hashlib.sha256(str(output).encode()).hexdigest() + ".xlsx")
                shutil.copyfile(output, retained)
                return book, retained, evidence
            self.assertNotEqual(process.returncode, 0)
            self.assertEqual(evidence["status"], "rejected")
            self.assertIn("ledger", evidence)
            if existed:
                self.assertEqual(output.read_bytes(), before_output)
            else:
                self.assertFalse(output.exists(), "Rejected export left an artifact")
            return evidence

    def test_live_formulas_and_formats_in_base_and_moved_layout(self):
        for layout in FIXTURE["layouts"].values():
            with self.subTest(layout=layout["sheets"][0]["name"]):
                book, _, evidence = self.export(layout=layout)
                verify_book(book, FIXTURE["document"], layout)
                self.assertTrue(any(x.get("code") == "semantic_metadata_not_exported"
                                    for x in evidence["ledger"]))

    def test_changed_input_and_rename_reexport_from_current_rust_source(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as work:
            direct, renamed, project = [Path(work) / name for name in ("edit.ro", "rename.ro", "new.roproj")]
            run([self.cli, "set", str(self.project), "tax.rate", "0.5", "--output", str(direct)])
            run([self.cli, "entity", "rename", str(direct), "jan", "renamed_jan", "--output", str(renamed)])
            run([self.cli, "roproj", "materialize", str(renamed), str(project)])
            book, _, _ = self.export(source=project, layout=FIXTURE["layouts"]["moved"])
            model = copy.deepcopy(FIXTURE["document"])
            model["entities"]["e-tax"]["fields"]["f-rate"]["value"] = 0.5
            model["entities"]["e-jan"]["key"] = "renamed_jan"
            verify_book(book, model, FIXTURE["layouts"]["moved"])

    def test_stale_snapshot_rejected_without_artifact(self):
        evidence = self.export(expected_revision="deliberately-stale", success=False)
        self.assertEqual(evidence["code"], "stale_source_revision")

    def test_missing_formula_dependency_rejected_without_artifact(self):
        layout = copy.deepcopy(FIXTURE["layouts"]["base"])
        layout["sheets"] = layout["sheets"][1:]
        evidence = self.export(layout=layout, success=False)
        self.assertEqual(evidence["code"], "unprojected_reference")

    def test_duplicate_semantic_target_rejected_without_artifact(self):
        layout = copy.deepcopy(FIXTURE["layouts"]["base"])
        layout["sheets"][1]["rows"].append("e-jan")
        evidence = self.export(layout=layout, success=False)
        self.assertEqual(evidence["code"], "ambiguous_projection")

    def test_existing_destination_is_never_overwritten(self):
        output = self.directory / "already-exists.xlsx"
        output.write_bytes(b"existing-output-canary")
        evidence = self.export(output=output, success=False)
        self.assertEqual(evidence["code"], "output_exists")
        self.assertEqual(output.read_bytes(), b"existing-output-canary")

    def test_same_snapshot_repeats_the_same_observable_workbook(self):
        first, _, _ = self.export()
        second, _, _ = self.export()
        for where in locations(FIXTURE["layouts"]["base"]).values():
            left, right = first.cell(*where), second.cell(*where)
            self.assertEqual(first.value(left), second.value(right))
            self.assertEqual(left.findtext(q("f")), right.findtext(q("f")))
            self.assertEqual(first.number_format(left), second.number_format(right))


    def materialize_variant(self, model: dict, directory: Path) -> Path:
        direct, project = directory / "variant.ro", directory / "variant.roproj"
        write_json(direct, model)
        run([self.cli, "validate", str(direct)])
        run([self.cli, "roproj", "materialize", str(direct), str(project)])
        return project

    def test_all_current_formula_operators_preserve_expression_shape(self):
        for operator in ("add", "subtract", "multiply", "divide", "minimum", "maximum"):
            with self.subTest(operator=operator), tempfile.TemporaryDirectory(dir=self.directory) as work:
                model = copy.deepcopy(FIXTURE["document"])
                model["entities"]["e-jan"]["fields"]["f-net"]["value"] = {
                    "op": operator, "args": {"left": {"op": "number", "args": 4},
                                              "right": {"op": "number", "args": 2}}}
                source = self.materialize_variant(model, Path(work))
                book, _, _ = self.export(source=source)
                verify_book(book, model, FIXTURE["layouts"]["base"])

    def test_formula_looking_text_stays_literal(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as work:
            model = copy.deepcopy(FIXTURE["document"])
            model["entities"]["e-jan"]["fields"]["f-label"]["value"] = "=1+1"
            source = self.materialize_variant(model, Path(work))
            book, _, _ = self.export(source=source)
            verify_book(book, model, FIXTURE["layouts"]["base"])

    def test_selected_reference_value_is_explicitly_unsupported(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as work:
            model = copy.deepcopy(FIXTURE["document"])
            definition = model["schemas"]["s-plan"]["fields"]["f-label"]
            definition["field_type"] = {"type": "reference", "schema": "s-assumptions"}
            for entity in ("e-jan", "e-feb", "e-mar"):
                model["entities"][entity]["fields"]["f-label"] = {"kind": "reference", "value": "e-tax"}
            source = self.materialize_variant(model, Path(work))
            evidence = self.export(source=source, success=False)
            self.assertEqual(evidence["code"], "unsupported_value")
            self.assertTrue(any(x.get("code") == "unsupported_value" for x in evidence["ledger"]))


    def test_export_destination_inside_source_is_rejected(self):
        evidence = self.export(output=self.project / "forbidden.xlsx", success=False)
        self.assertEqual(evidence["code"], "invalid_output")

    def test_boolean_values_do_not_turn_into_numbers(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as work:
            model = copy.deepcopy(FIXTURE["document"])
            model["schemas"]["s-plan"]["fields"]["f-label"]["field_type"] = {"type": "boolean"}
            for entity, value in (("e-jan", True), ("e-feb", False), ("e-mar", True)):
                model["entities"][entity]["fields"]["f-label"] = {"kind": "boolean", "value": value}
            source = self.materialize_variant(model, Path(work))
            book, _, _ = self.export(source=source)
            verify_book(book, model, FIXTURE["layouts"]["base"])
            for entity in ("e-jan", "e-feb", "e-mar"):
                cell = book.cell(*locations(FIXTURE["layouts"]["base"])[(entity, "f-label")])
                self.assertEqual(cell.attrib.get("t"), "b")

    def test_grown_source_is_not_truncated_to_old_row_count(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as work:
            model = copy.deepcopy(FIXTURE["document"])
            row = copy.deepcopy(model["entities"]["e-jan"])
            row["id"], row["key"] = "e-apr", "apr"
            row["fields"]["f-label"]["value"] = "April"
            row["fields"]["f-revenue"]["value"] = 800
            def retarget(expression):
                if expression["op"] == "reference":
                    if expression["args"]["entity"] == "e-jan":
                        expression["args"]["entity"] = "e-apr"
                elif expression["op"] != "number":
                    retarget(expression["args"]["left"])
                    retarget(expression["args"]["right"])
            for field in ("f-gross", "f-net"):
                retarget(row["fields"][field]["value"])
            model["entities"]["e-apr"] = row
            source = self.materialize_variant(model, Path(work))
            self.assertEqual(self.facts(source, "e-apr.f-net")["outcome"]["calculation"],
                             {"kind": "value", "value": 300})
            layout = copy.deepcopy(FIXTURE["layouts"]["base"])
            layout["sheets"][1]["rows"].insert(0, "e-apr")
            book, _, _ = self.export(source=source, layout=layout)
            verify_book(book, model, layout)


class OfficeAcceptance(ExportAcceptance):
    def test_uncached_exports_recalculate_in_real_office_engine(self):
        office = executable("REFERENCE_OFFICE_BIN")
        print("Reference engine:", run([office, "--version"]).stdout.strip())
        for name, rate in (("base", 0.25), ("tax_half", 0.5)):
            with tempfile.TemporaryDirectory(dir=self.directory) as work:
                work = Path(work)
                source = self.project
                model = copy.deepcopy(FIXTURE["document"])
                if name != "base":
                    direct, source = work / "edit.ro", work / "edit.roproj"
                    run([self.cli, "set", str(self.project), "tax.rate", str(rate), "--output", str(direct)])
                    run([self.cli, "roproj", "materialize", str(direct), str(source)])
                    model["entities"]["e-tax"]["fields"]["f-rate"]["value"] = rate
                layout = FIXTURE["layouts"]["moved"]
                book, original, _ = self.export(source=source, layout=layout)
                verify_book(book, model, layout)  # Prove no cached answers BEFORE office opens it.
                output = work / "office-output"
                output.mkdir()
                profile = (work / "office-profile").as_uri()
                run([office, f"-env:UserInstallation={profile}", "--headless", "--convert-to",
                     "xlsx:Calc MS Excel 2007 XML", "--outdir", str(output), str(original)])
                recalculated = WorkbookXml(output / original.name)
                where = locations(layout)
                for target, expected in FIXTURE["expected"][name].items():
                    cell = recalculated.cell(*where[tuple(target.split("."))])
                    self.assertIsNotNone(cell.find(q("f")), "Office discarded formula")
                    self.assertEqual(recalculated.value(cell), expected)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("self-check", "preflight", "acceptance", "office"), required=True)
    arguments = parser.parse_args()
    selected = {"self-check": OracleSelfChecks, "preflight": RuntimePreflight,
                "acceptance": ExportAcceptance, "office": OfficeAcceptance}[arguments.mode]
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(selected)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)
