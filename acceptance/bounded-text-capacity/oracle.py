"""Independent standard-library oracle; never imports Tachiko parsers or writers.

Run with uv run --no-project oracle.py generate DIR or verify DIR.
New synthetic recipe, not the historical Sheet #150 artifact/hash.
"""

import csv
import hashlib
import io
import math
from rss_oracle import check_rss_budget
import json
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

ROWS = 8406
HEADERS = ["account_id", "profile_url", "unix_timestamp"]
MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/package/2006/relationships"
EDITS = {
    (0, 0): '000-edited-雪,"first"\r\nkept',
    (ROWS // 2, 1): "https://example.invalid/edited-middle?x=0007",
    (ROWS - 1, 2): "0001700008405999",
}


def values(count=ROWS):
    result = []
    for i in range(count):
        identifier = f"invented_{i:05}"
        if i % 1024 == 0:
            identifier += ',雪"quote"\nLF\r\nCRLF'
        result.append(
            [
                identifier,
                f"https://example.invalid/account/{i:05}",
                ("000" if i % 97 == 0 else "") + str(1700000000000 + i),
            ]
        )
    return result


def promoted_opened_values():
    return [[character * 4096 for character in "xyz"] for _ in range(8)]


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
    csv.writer(output, lineterminator="\r\n", quoting=csv.QUOTE_ALL).writerows(
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
    manifest = {
        "recipe": "work489-text-v1-new-synthetic-2026-10-04",
        "provenance": "Invented values; not Sheet150 historical fixture hashes",
        "headers": HEADERS,
        "types": ["text"] * 3,
        "rows": rows,
        "promoted_opened_rows": promoted_opened_values(),
        "edits": [[r, c, value] for (r, c), value in EDITS.items()],
    }
    (directory / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, separators=(",", ":")) + "\n"
    )
    for count in [8, 64, 65, 128, 129, 1024, 1025, ROWS, ROWS + 1]:
        for extension, encode in [("csv", csv_bytes), ("xlsx", xlsx)]:
            (directory / f"source-{count}.{extension}").write_bytes(
                encode(values(count))
            )
    for extension, encode in [("csv", csv_bytes), ("xlsx", xlsx)]:
        path = directory / f"promoted-opened.{extension}"
        path.write_bytes(encode(promoted_opened_values()))
        actual = (
            list(csv.reader(io.StringIO(path.read_bytes().decode(), newline="")))
            if extension == "csv"
            else parse_xlsx(path)
        )
        assert actual == [HEADERS, *promoted_opened_values()]
    (directory / "malformed.csv").write_bytes(
        b'account_id,profile_url,unix_timestamp\r\n"unclosed'
    )
    (directory / "malformed.xlsx").write_bytes(b"PK\x03\x04truncated")
    for extension in ["csv", "xlsx"]:
        source = directory / f"source-{ROWS}.{extension}"
        actual = (
            list(csv.reader(io.StringIO(source.read_bytes().decode(), newline="")))
            if extension == "csv"
            else parse_xlsx(source)
        )
        assert actual == [HEADERS, *rows], f"generator independent readback {extension}"
    hashes = {
        p.name: hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(directory.iterdir())
    }
    (directory / "SHA256.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(
        json.dumps({"generated": str(directory), "cells": ROWS * 3, "sha256": hashes})
    )


def verify(directory):
    boundary = json.loads((directory / "boundaries.json").read_text())
    assert boundary["mode"] == "all" and boundary["completed"] is True
    assert len(boundary["records"]) >= 13
    assert all(record["outcome"] == "PASS" for record in boundary["records"])
    names = {record["name"] for record in boundary["records"]}
    required = {
        "ordinary request 65536 and 65537",
        "small plain capacity preview uses complete profile",
        "metadata discriminator permits type-last objects",
        "metadata discriminator refuses positional array control",
        "metadata discriminator refuses duplicate export then import",
        "metadata discriminator refuses duplicate import then export",
        "metadata discriminator refuses duplicate export",
        "metadata discriminator refuses malformed allowed tag",
        "metadata discriminator refuses invalid nonmetadata payload",
        "metadata discriminator refuses deep ignored nonmetadata payload",
        "allowed discriminator retains typed payload validation",
        "metadata request exact 4MiB and limit plus one",
        "encoded CSV header row exact 39 bytes and plus one",
        "Unicode XML-sensitive metadata header39 and name31 roundtrips",
        "oversized non-metadata spreadsheet operations",
        "4096 UTF8 Text byte ceiling",
        "aggregate Text exact 1MiB and plus one",
        "CSV export exact 2MiB and candidate limit plus one",
        "empty Text retains existing missing-value representation",
        "history remains executable after all refusals",
        "thirteen-column profile remains unadmitted",
    }
    required.update(
        f"{boundary}-{delta}.xlsx"
        for boundary in [
            "source-byte",
            "expanded-byte",
            "xml-node",
            "xml-depth",
            "zip-entry",
        ]
        for delta in [0, 1]
    )
    string_cases = json.loads(
        (directory / "fixtures/string-escape-manifest.json").read_text()
    )
    assert len(string_cases) == 88
    required.update(
        f"ST {'refusal' if case['refuse'] else 'near-match'} {case['file']}"
        for case in string_cases
    )
    for pattern in ["_x0041_", "_x00aF_", "_x005F_x0041_"]:
        required.add(f"ST edit {pattern}")
        for header in ["false", "true"]:
            required.update(
                f"ST CSV {count} header={header} {pattern}" for count in [8, 8406]
            )
            required.update(
                f"ST metadata {kind} header={header} {pattern}"
                for kind in ["csv", "xlsx", "inspect"]
            )
    assert required <= names, f"missing mandatory boundary cases: {required - names}"
    assert boundary["wasm_linear_memory_high_water_bytes"] <= 256 * 1024 * 1024
    profile = (directory / "native-profile-boundaries.log").read_text()
    for marker in [
        "PASS identity256/257",
        "PASS metadata3145728/3145729",
        "PASS full Opened reply16777216/16777217",
        "PASS unchanged ordinary queryFields complete reply65536/65537",
        "PASS large constraint/multiple-schema/non-Text/formula/saved-definition disqualification",
        "PASS row-limit refusal preserves None, complete resident, exports and both history stacks",
        "PASS ST_Xstring capacity reopen refusal, near-matches and generic preservation",
    ]:
        assert marker in profile, f"missing profile group: {marker}"
    expected = values()
    for (r, c), value in EDITS.items():
        expected[r][c] = value
    expected_names = {
        f"native-{carrier}/{source}{suffix}-export.{output}"
        for carrier in ["opaque", "canonical"]
        for source in ["csv", "xlsx"]
        for suffix in ["", "-reopened"]
        for output in ["csv", "xlsx"]
    } | {
        f"worker/{source}-{carrier}-{mode}-export.{output}"
        for carrier in ["opaque", "canonical"]
        for source in ["csv", "xlsx"]
        for mode in ["import", "reopen"]
        for output in ["csv", "xlsx"]
    }
    files = sorted(directory.rglob("*-export.csv")) + sorted(
        directory.rglob("*-export.xlsx")
    )
    assert {str(p.relative_to(directory)) for p in files} == expected_names, (
        "incomplete/extra export matrix"
    )
    def check_timings(timings, counts):
        for name, count in counts.items():
            assert len(timings[name]) == count, (name, count)
        for name, samples in timings.items():
            assert samples and all(isinstance(x, (int, float)) and math.isfinite(x) and x >= 0 for x in samples)
            assert max(samples) <= (1000 if name in ("edit", "undo", "redo") else 10000)
            if name in ("edit", "undo", "redo"):
                assert sorted(samples)[math.ceil(len(samples) * .95) - 1] <= 500

    assert {p.name for p in (directory / "worker").glob("*-capacity-from-*.xlsx")} == {
        f"{recipe}-capacity-from-{source}-{carrier}.xlsx"
        for recipe in ["small", "promoted-opened"]
        for source in ["csv", "xlsx"]
        for carrier in ["opaque", "canonical"]
    }, "incomplete/extra small capacity artifact matrix"
    for carrier in ["opaque", "canonical"]:
        native = directory / f"native-{carrier}"
        imported = json.loads((native / "native-import-complete.json").read_text())
        assert imported["cells"] == ROWS * 3 and imported["history_edits_each"] == 128
        assert imported["carrier"] == carrier
        for source in ["csv", "xlsx"]:
            check_timings(json.loads((native / f"{source}-timings.json").read_text()), {"edit": 131, "undo": 3, "redo": 3})
            check_timings(json.loads((native / f"{source}-reopen-timings.json").read_text()), {"reopen": 10, "resave": 10})
            fresh = json.loads((native / f"{source}-fresh-process.json").read_text())
            assert fresh["pid"] != imported["pid"] and fresh["cycles"] == 10
            assert fresh["cells"] == ROWS * 3 and fresh["carrier"] == carrier
            assert fresh["saved_open_route"] == "metadata-aware"
            for recipe, rows in [("small", values(64)), ("promoted-opened", promoted_opened_values())]:
                small = directory / f"worker/{recipe}-capacity-from-{source}-{carrier}.xlsx"
                assert parse_xlsx(small) == [HEADERS, *rows]
                with zipfile.ZipFile(small) as archive:
                    workbook = ET.fromstring(archive.read("xl/workbook.xml"))
                    sheet = workbook.find(f"{{{MAIN}}}sheets/{{{MAIN}}}sheet")
                    assert sheet.attrib["name"] == "Capacity\tname\n雪&\r"
            for mode in ["import", "reopen"]:
                receipt = json.loads((directory / f"worker/{source}-{carrier}-{mode}-receipt.json").read_text())
                assert (receipt["format"], receipt["carrier"], receipt["mode"], receipt["savedOpenRoute"]) == (source, carrier, mode, "metadata-aware")
                check_rss_budget(receipt["memory"])
                assert receipt["cells"] == ROWS * 3
                assert len(receipt["workerUrls"]) == (2 if mode == "import" else 1)
                counts = {"edit": 131, "undo": 10, "redo": 10} if mode == "import" else {"reopen": 10, "resave": 10}
                check_timings(receipt["timings"], counts)
                heartbeat = receipt["heartbeat"]
                assert heartbeat and all(isinstance(x, (int, float)) and math.isfinite(x) and x >= 0 for x in heartbeat)
                assert sorted(heartbeat)[math.ceil(len(heartbeat) * .95) - 1] <= 100 and max(heartbeat) <= 250
                memory = receipt["wasmMemory"]
                stages = ["promoted-opened-before-termination", "imported", "history-64", "history-128", "exported"] if mode == "import" else ["reopen-1", "reopen-5", "reopen-10", "exported"]
                assert [x["stage"] for x in memory] == stages
                assert all(isinstance(x["bytes"], int) and 0 < x["bytes"] <= 256 * 1024 * 1024 for x in memory)
                assert all(x["worker"] == len(receipt["workerUrls"]) for x in memory[-4:])
                cases = receipt["smallProjections"]
                if mode == "import":
                    assert [x["recipe"] for x in cases] == ["promoted-opened", "small"]
                    assert [x["cells"] for x in cases] == [24, 192]
                    assert all(x["imported"] > 65536 for x in cases)
                    assert cases[0]["opened"] > 65536 and cases[0]["reopened"] > 65536
                    assert [x["workerRestarted"] for x in cases] == [True, False]
                else:
                    assert cases == []
    worker = json.loads((directory / "worker/worker-complete.json").read_text())
    assert worker["receipts"] == 8 and worker["cells"] == ROWS * 3
    assert worker["carriers"] == ["opaque", "canonical"]
    assert worker["capacity_performance_qualification"] is True and worker["producer_qualification"] is False
    metadata_headers = ["雪&", "<i>", "url\"'" + "x" * 20]
    header_output = io.StringIO(newline="")
    csv.writer(header_output, lineterminator="\r\n").writerow(metadata_headers)
    assert len(header_output.getvalue().encode()) == 39
    metadata_csv = directory / "capacity-metadata.csv"
    metadata_xlsx = directory / "capacity-metadata.xlsx"
    assert list(
        csv.reader(io.StringIO(metadata_csv.read_bytes().decode(), newline=""))
    ) == [
        metadata_headers,
        *values(),
    ]
    assert parse_xlsx(metadata_xlsx) == [metadata_headers, *values()]
    with zipfile.ZipFile(metadata_xlsx) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet = workbook.find(f"{{{MAIN}}}sheets/{{{MAIN}}}sheet")
        assert sheet.attrib["name"] == "雪&\"<'>\n\t\r" + "n" * 22
    for path in files:
        actual = (
            list(csv.reader(io.StringIO(path.read_bytes().decode(), newline="")))
            if path.suffix == ".csv"
            else parse_xlsx(path)
        )
        assert actual == [HEADERS, *expected], f"all-cell discrepancy: {path}"
    print(json.dumps({"verified": [str(p) for p in files], "cells_per_file": ROWS * 3, "capacity_performance_qualification": True, "producer_qualification": False}))


if __name__ == "__main__":
    action, folder = sys.argv[1:]
    {"generate": generate, "verify": verify}[action](Path(folder))
