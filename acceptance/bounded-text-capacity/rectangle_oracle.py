"""Work #507 independent fixtures and complete CSV/typed XLSX export oracle.

Standard library only. Fixture readback is not runtime qualification. Expected
matrices are frozen before execution; exports never supply their own oracle.
"""
import csv
import hashlib
import io
import json
import math
import sys
import xml.etree.ElementTree as ET
import zipfile
from decimal import Decimal
from pathlib import Path

MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/package/2006/relationships"
DOCREL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
EDIT_TEXT = '000-edited-雪,"first"\r\nkept'


def recipe(rows, columns, mixed=True):
    headers = ["date", "amount"] if columns == 2 else [f"f{c:02}" for c in range(columns)]
    if columns == 3 and not mixed:
        headers = ["account_id", "profile_url", "unix_timestamp"]
    kinds = ["number" if mixed and c % 2 else "text" for c in range(columns)]
    values = []
    for r in range(rows):
        row = []
        for c, kind in enumerate(kinds):
            value = None
            if not (r > 0 and (r + c) % 113 == 0):
                if kind == "number":
                    number = [0, -13.25, 42.5, 1_000_000, 1e-6, -1e-7, r + 0.25][r % 7]
                    value = {"kind": kind, "value": number}
                else:
                    text = f'000{r:05}_{c},雪"quote"\nLF\r\nCRLF' if r % 97 == 0 else f"000{r:05}_{c}"
                    value = {"kind": kind, "value": text}
            row.append(value)
        values.append(row)
    return {"name": f"{rows}x{columns}-{'mixed' if mixed else 'text'}", "headers": headers, "kinds": kinds, "values": values}


def lexical(value):
    return "" if value is None else str(value["value"])


def canonical_number(value):
    text = format(Decimal(str(value)), "f")
    return text.rstrip("0").rstrip(".") if "." in text else text


def csv_bytes(case):
    output = io.StringIO(newline="")
    csv.writer(output, lineterminator="\r\n").writerows([case["headers"], *[[lexical(v) for v in row] for row in case["values"]]])
    return output.getvalue().encode()


def escaped(value):
    return value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\r", "&#13;")


def xlsx_bytes(case):
    rows = [[{"kind": "text", "value": h} for h in case["headers"]], *case["values"]]
    sheet = f'<worksheet xmlns="{MAIN}"><sheetData>'
    for r, row in enumerate(rows, 1):
        sheet += f'<row r="{r}">'
        for c, value in enumerate(row):
            address = f"{chr(65 + c)}{r}"
            if value is None:
                sheet += f'<c r="{address}"/>'
            elif value["kind"] == "number":
                sheet += f'<c r="{address}" t="n"><v>{value["value"]}</v></c>'
            else:
                sheet += f'<c r="{address}" t="inlineStr"><is><t xml:space="preserve">{escaped(value["value"])}</t></is></c>'
        sheet += "</row>"
    sheet += "</sheetData></worksheet>"
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>',
        "_rels/.rels": f'<Relationships xmlns="{REL}"><Relationship Id="rId1" Type="{DOCREL}/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        "xl/workbook.xml": f'<workbook xmlns="{MAIN}" xmlns:r="{DOCREL}"><sheets><sheet name="Synthetic" sheetId="1" r:id="rId1"/></sheets></workbook>',
        "xl/_rels/workbook.xml.rels": f'<Relationships xmlns="{REL}"><Relationship Id="rId1" Type="{DOCREL}/worksheet" Target="worksheets/sheet1.xml"/></Relationships>',
        "xl/worksheets/sheet1.xml": sheet,
    }
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, text in sorted(parts.items()):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, text.encode())
    return output.getvalue()


def read_xlsx(path, columns):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate archive member"
        book = ET.fromstring(archive.read("xl/workbook.xml"))
        sheets = book.findall(f"{{{MAIN}}}sheets/{{{MAIN}}}sheet")
        assert len(sheets) == 1
        rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
        target = next(r.attrib["Target"] for r in rels if r.attrib["Id"] == sheets[0].attrib[f"{{{DOCREL}}}id"])
        assert target == "worksheets/sheet1.xml", "unexpected fixture/export layout"
        shared = []
        if "xl/sharedStrings.xml" in names:
            strings = ET.fromstring(archive.read("xl/sharedStrings.xml"))
            assert strings.tag == f"{{{MAIN}}}sst"
            for item in strings:
                assert item.tag == f"{{{MAIN}}}si" and len(item) == 1
                assert item[0].tag == f"{{{MAIN}}}t", "unexpected shared-string structure"
                shared.append(item[0].text or "")
        sheet = ET.fromstring(archive.read("xl/" + target))
        result = []
        for r, row in enumerate(sheet.findall(f"{{{MAIN}}}sheetData/{{{MAIN}}}row"), 1):
            assert row.attrib["r"] == str(r)
            assert len(row) == columns
            values = []
            for c, cell in enumerate(row):
                assert cell.attrib["r"] == f"{chr(65 + c)}{r}"
                assert cell.find(f"{{{MAIN}}}f") is None
                kind = cell.attrib.get("t", "n")
                if kind == "inlineStr":
                    value = {"kind": "text", "value": "".join(cell.find(f"{{{MAIN}}}is").itertext())}
                elif kind == "s":
                    index = cell.findtext(f"{{{MAIN}}}v", "")
                    assert index and all("0" <= digit <= "9" for digit in index), "invalid shared-string index"
                    assert int(index) < len(shared), "shared-string index out of bounds"
                    value = {"kind": "text", "value": shared[int(index)]}
                elif kind == "str":
                    value = {"kind": "text", "value": cell.findtext(f"{{{MAIN}}}v", "")}
                else:
                    assert kind == "n", f"unexpected typed carrier {kind}"
                    text = cell.findtext(f"{{{MAIN}}}v", "")
                    number = float(text) if text else None
                    assert number is None or math.isfinite(number)
                    value = None if number is None else {"kind": "number", "value": number}
                values.append(value)
            result.append(values)
        return result


def generate(directory):
    directory.mkdir(parents=True, exist_ok=False)
    accepted = [recipe(8, 2), recipe(320, 2), recipe(1000, 16), recipe(1576, 16), recipe(8406, 1), recipe(8406, 3, False)]
    refused = [recipe(65, 17), recipe(8407, 1), recipe(1577, 16)]
    for rows, group in [(65, accepted), (64, refused)]:
        case = recipe(rows, 2)
        case["name"] += "-oversize"
        for row in case["values"]:
            row[0] = {"kind": "text", "value": "x" * 1200}
        group.append(case)
    for excess, group in [(0, accepted), (1, refused)]:
        case = recipe(65, 16)
        case["name"] += f"-header{208 + excess}"
        case["headers"][0] = "h" * (146 + excess)
        group.append(case)
    for label, value in [("expression", "-1+2"), ("nonfinite", "inf"), ("mixed-column", "not-a-number")]:
        case = recipe(320, 2)
        case["name"] += "-" + label
        case["values"][27][1] = {"kind": "text", "value": value}
        refused.append(case)
    for case in [*accepted, *refused]:
        for extension, encoder in [("csv", csv_bytes), ("xlsx", xlsx_bytes)]:
            path = directory / f'{case["name"]}.{extension}'
            path.write_bytes(encoder(case))
            if extension == "csv":
                actual = list(csv.reader(io.StringIO(path.read_bytes().decode(), newline="")))
                assert actual == [case["headers"], *[[lexical(v) for v in row] for row in case["values"]]]
            else:
                actual = read_xlsx(path, len(case["headers"]))
                assert actual == [[{"kind": "text", "value": h} for h in case["headers"]], *case["values"]]
    (directory / "cases.json").write_text(json.dumps({"accepted": accepted, "refused": refused}, ensure_ascii=False))
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(directory.iterdir())}
    (directory / "SHA256.json").write_text(json.dumps(hashes, indent=2))
    print(json.dumps({"fixture_readback_only": True, "files": len(hashes)}))


def verify(directory, fixtures):
    cases = {c["name"]: c for c in json.loads((fixtures / "cases.json").read_text())["accepted"]}
    captures = json.loads((directory / "captures.json").read_text())
    required = {(name, source, carrier, phase, format) for name in ["320x2-mixed", "1000x16-mixed", "8406x3-text"] for source in ["csv", "xlsx"] for carrier in ["opaque", "canonical"] for phase in ["saved", "reopened"] for format in ["csv", "xlsx"]}
    actual_cases = [(c["case"], c["sourceFormat"], c["carrier"], c["phase"], Path(c["file"]).suffix[1:]) for c in captures]
    assert len(actual_cases) == len(required) and set(actual_cases) == required, "missing/duplicate lifecycle exports"
    for capture in captures:
        case = cases[capture["case"]]
        expected = json.loads(json.dumps(case["values"]))
        assert capture["state"] == "edited"
        for r, c in [(0, 0), (len(expected) // 2, len(case["kinds"]) - 1), (len(expected) - 1, 0)]:
            expected[r][c] = {"kind": "number", "value": 700.25} if case["kinds"][c] == "number" else {"kind": "text", "value": EDIT_TEXT}
        path = directory / capture["file"]
        if path.suffix == ".xlsx":
            actual = read_xlsx(path, len(case["headers"]))
            assert actual == [[{"kind": "text", "value": h} for h in case["headers"]], *expected], str(path)
        else:
            actual = list(csv.reader(io.StringIO(path.read_bytes().decode(), newline="")))
            assert actual[0] == case["headers"] and len(actual) == len(expected) + 1
            for r, row in enumerate(expected):
                assert len(actual[r + 1]) == len(row)
                for c, value in enumerate(row):
                    text = actual[r + 1][c]
                    if value is not None and value["kind"] == "number":
                        assert text == canonical_number(value["value"]), (path, r, c, "canonical Number text")
                    else:
                        assert text == lexical(value), (path, r, c)
    print(json.dumps({"complete_export_oracle": "PASS", "captures": len(captures)}))


if __name__ == "__main__":
    if sys.argv[1] == "generate":
        generate(Path(sys.argv[2]))
    elif sys.argv[1] == "verify":
        verify(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        raise SystemExit("generate NEW_DIRECTORY | verify CAPTURE_DIRECTORY FIXTURES")
