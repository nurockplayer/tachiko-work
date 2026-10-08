"""UNFROZEN supplementary fixture/export oracle proposal. Standard library only.
XLSX writer/reader derived from exact public248 acceptance oracle.py. The new
compact66 recipe matches owning5999450017, not old f33 candidate/artifacts.
Generate/readback is fixture evidence only. verify requires actual Worker results.
"""
import csv
import hashlib
import io
import json
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

ROWS = 66
HEADERS = ["a", "b", "c"]
MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/package/2006/relationships"
def values():
    return [[f"key{v:03}", f"value{v:03}", f"00{v:03}"] for i in range(66) for v in [0 if i < 2 else i]]



def escaped(value):
    return (
        value.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\r", "&#13;")
    )


def xlsx(rows):
    sheet = f'<worksheet xmlns="{MAIN}"><sheetData>'
    for r, row in enumerate([HEADERS, *rows], 1):
        cells = "".join(
            f'<c r="{chr(65 + c)}{r}" t="str"><v>{escaped(value)}</v></c>'
            for c, value in enumerate(row)
        )
        sheet += f'<row r="{r}">{cells}</row>'
    sheet += "</sheetData></worksheet>"
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>',
        "_rels/.rels": f'<Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        "xl/workbook.xml": f'<workbook xmlns="{MAIN}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Synthetic" sheetId="1" r:id="rId1"/></sheets></workbook>',
        "xl/_rels/workbook.xml.rels": f'<Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>',
        "xl/worksheets/sheet1.xml": sheet,
    }
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, text in sorted(parts.items()):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, text.encode())
    return buffer.getvalue()


def csv_bytes(rows):
    output = io.StringIO(newline="")
    csv.writer(output, lineterminator="\r\n", quoting=csv.QUOTE_MINIMAL).writerows(
        [HEADERS, *rows]
    )
    return output.getvalue().encode()


def parse_xlsx(path):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate archive member"
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheets = workbook.findall(f"{{{MAIN}}}sheets/{{{MAIN}}}sheet")
        assert len(sheets) == 1, "unexpected worksheet count"
        relationships = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
        relationship_id = sheets[0].attrib[
            "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"
        ]
        target = next(
            r.attrib["Target"]
            for r in relationships
            if r.attrib["Id"] == relationship_id
        )
        sheet = ET.fromstring(archive.read("xl/" + target))
        shared = []
        if "xl/sharedStrings.xml" in names:
            shared = [
                "".join(si.itertext())
                for si in ET.fromstring(archive.read("xl/sharedStrings.xml"))
            ]
        rows = []
        for r, row in enumerate(sheet.findall(f"{{{MAIN}}}sheetData/{{{MAIN}}}row"), 1):
            assert row.attrib["r"] == str(r), "row order"
            cells = list(row)
            assert len(cells) == 3, "column count"
            values_out = []
            for c, cell in enumerate(cells):
                assert cell.attrib["r"] == f"{chr(65 + c)}{r}", "cell order"
                assert cell.find(f"{{{MAIN}}}f") is None, "unexpected formula"
                kind = cell.attrib.get("t")
                assert kind in ("str", "inlineStr", "s"), "Text type lost"
                value = cell.findtext(f"{{{MAIN}}}v", "")
                if kind == "s":
                    value = shared[int(value)]
                elif kind == "inlineStr":
                    value = "".join(cell.find(f"{{{MAIN}}}is").itertext())
                values_out.append(value)
            rows.append(values_out)
        return rows


def generate(directory):
    directory.mkdir(parents=True, exist_ok=False)
    rows = values()
    manifest = {"recipe": "compact66-owner-counterexample5999450017", "headers": HEADERS, "rows": rows, "provenance": "New independently generated CSV/XLSX; not unavailable f33 bytes or runtime evidence"}
    (directory / "saved-import-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False) + "\n")
    for extension, encode in [("csv", csv_bytes), ("xlsx", xlsx)]:
        path = directory / f"saved-import-66.{extension}"
        path.write_bytes(encode(rows))
        actual = list(csv.reader(io.StringIO(path.read_bytes().decode(), newline=""))) if extension == "csv" else parse_xlsx(path)
        assert actual == [HEADERS, *rows], f"independent fixture readback {extension}"
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(directory.iterdir())}
    (directory / "SHA256.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(json.dumps({"fixture_readback_only": True, "cells": 198, "sha256": hashes}))


def verify(directory, fixture_path=None):
    fixture = json.loads(Path(fixture_path).read_text()) if fixture_path else {"headers": HEADERS, "rows": values()}
    assert len(fixture["rows"]) in (66, 8406) and len(fixture["headers"]) == 3
    count = len(fixture["rows"])
    snapshots = 6 if count == 66 else 5
    receipts = json.loads((directory / "worker-result.json").read_text())
    expected_receipts = {(f, c, p) for f in ["csv", "xlsx"] for c in ["canonical", "opaque"] for p in ["save", "restart-reopen"]}
    assert len(receipts) == 8
    assert {(r["format"], r["carrier"], r["phase"]) for r in receipts} == expected_receipts
    assert all(r["outcome"] == "PASS" and len(r["workerUrls"]) == 1 and r["fixtureRows"] == count for r in receipts)
    outputs = sorted(directory.glob("*.csv")) + sorted(directory.glob("*.xlsx"))
    expected_names = {f"{f}-{c}-{phase}-{index}-snapshot.{output}" for f, c, phase in expected_receipts for index in range(snapshots) for output in ["csv", "xlsx"]}
    assert {p.name for p in outputs} == expected_names, "complete process/carrier/snapshot/export matrix"
    assert {p.name + ".expected.json" for p in outputs} == {p.name for p in directory.glob("*.expected.json")}
    for path in outputs:
        expected = json.loads(Path(str(path) + ".expected.json").read_text())
        assert len(expected) == count and all(len(row) == 3 for row in expected)
        fields = path.stem.split("-")
        # Filename syntax is checked through expected_names above; the snapshot
        # index is directly before the final snapshot suffix.
        index = int(fields[-2])
        independent = [list(row) for row in fixture["rows"]]
        if index == 0:
            if "restart-reopen" in path.name:
                independent[-1][0] = "history-two雪&<>"
        elif index == snapshots - 1:
            independent[-1][0] = "history-two雪&<>"
        else:
            independent[-1][0] = "history-one雪&<>"
        assert expected == independent, f"held expectation discrepancy {path}"
        # Output comparison below uses independent source plus explicit edits,
        # never accepting candidate-generated sidecars as the oracle itself.
        actual = list(csv.reader(io.StringIO(path.read_bytes().decode(), newline=""))) if path.suffix == ".csv" else parse_xlsx(path)
        assert actual == [fixture["headers"], *independent], f"independent all-cell export discrepancy {path}"
    print(json.dumps({"verified_export_files": len(outputs), "cells_each": count * 3, "performance_qualification": False}))

if __name__ == "__main__":
    action, directory, *rest = sys.argv[1:]
    if action == "generate":
        assert not rest
        generate(Path(directory))
    elif action == "verify":
        assert len(rest) <= 1
        verify(Path(directory), rest[0] if rest else None)
    else:
        raise ValueError(action)
