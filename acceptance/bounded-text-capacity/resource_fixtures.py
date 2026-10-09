"""Independent hostile/resource fixtures for parser boundary qualification.

Run after oracle.py generate, using uv run --no-project. Boundary padding is
XML whitespace or an explicit unsupported extension, never real source data.
"""

import csv
import io
import json
import re
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from oracle import xlsx


def pack(parts, compression=zipfile.ZIP_DEFLATED):
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w") as archive:
        for name, data in sorted(parts.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = compression
            archive.writestr(info, data)
    return out.getvalue()


def string_escape_variants(folder):
    worksheet = "xl/worksheets/sheet1.xml"
    variants = [
        "_x0041_",
        "_x00aF_",
        "_x005F_x0041_",
        "_x00&#52;1_",
        "_X0041_",
        "_x041_",
        "_x00G1_",
    ]
    records = []
    for count in [8, 8406]:
        rows = [["v" * (4096 if count == 8 else 1)] * 3 for _ in range(count)]
        rows[0][0] = "ESCAPE_SENTINEL"
        with zipfile.ZipFile(io.BytesIO(xlsx(rows))) as archive:
            original = {name: archive.read(name) for name in archive.namelist()}
        for old, new in [
            (b"account_id", b"a"),
            (b"profile_url", b"b"),
            (b"unix_timestamp", b"c"),
        ]:
            original[worksheet] = original[worksheet].replace(old, new, 1)
        for location in ["str", "inline", "shared", "header", "sheet"]:
            for index, spelling in enumerate(variants):
                parts = dict(original)
                value = f"left{spelling}right".encode()
                cell = b'<c r="A2" t="str"><v>ESCAPE_SENTINEL</v></c>'
                if location == "inline":
                    parts[worksheet] = parts[worksheet].replace(
                        cell,
                        b'<c r="A2" t="inlineStr"><is><t>' + value + b"</t></is></c>",
                    )
                elif location == "shared":
                    parts[worksheet] = parts[worksheet].replace(
                        cell, b'<c r="A2" t="s"><v>0</v></c>'
                    )
                    parts["xl/sharedStrings.xml"] = (
                        b'<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1" uniqueCount="1"><si><t>'
                        + value
                        + b"</t></si></sst>"
                    )
                    parts["xl/_rels/workbook.xml.rels"] = parts[
                        "xl/_rels/workbook.xml.rels"
                    ].replace(
                        b"</Relationships>",
                        b'<Relationship Id="strings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>',
                    )
                    parts["[Content_Types].xml"] = parts["[Content_Types].xml"].replace(
                        b"</Types>",
                        b'<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>',
                    )
                elif location == "sheet":
                    parts["xl/workbook.xml"] = parts["xl/workbook.xml"].replace(
                        b"Synthetic", value
                    )
                else:
                    old = b"<v>a</v>" if location == "header" else b"ESCAPE_SENTINEL"
                    new = b"<v>" + value + b"</v>" if location == "header" else value
                    parts[worksheet] = parts[worksheet].replace(old, new, 1)
                roots = [ET.fromstring(part) for part in parts.values()]
                decoded = "".join(
                    text
                    for root in roots
                    for node in root.iter()
                    for text in [node.text or "", *node.attrib.values()]
                )
                assert bool(re.search(r"_x[0-9A-Fa-f]{4}_", decoded)) == (index < 4)
                name = f"string-{count}-{location}-{index}.xlsx"
                (folder / name).write_bytes(pack(parts))
                records.append(
                    {
                        "file": name,
                        "refuse": index < 4,
                        "rows": count,
                        "xml_valid": True,
                        "location": location,
                        "value": ET.fromstring(b"<t>" + value + b"</t>").text,
                    }
                )
        malformed = [
            (worksheet, b"ESCAPE_SENTINEL", spelling)
            for spelling in [b"&undefined;", b"&#0;", b"&#x110000;", b"]]>"]
        ]
        malformed += [
            (worksheet, b'<c r="A2" t=', b'<c r="A2"t='),
            (
                worksheet,
                b"<worksheet",
                b'<?xml version="1.0"encoding="UTF-8"?><worksheet',
            ),
            ("xl/workbook.xml", b'name="Synthetic"', b'name="Bad<name"'),
            (worksheet, b'<c r="A2"', b'<bad::c r="A2"'),
            (worksheet, b"</worksheet>", b"</bad::worksheet>"),
        ]
        for index, (path, before, after) in enumerate(malformed):
            parts = dict(original)
            assert before in parts[path]
            parts[path] = parts[path].replace(before, after, 1)
            try:
                ET.fromstring(parts[path])
            except ET.ParseError:
                pass
            else:
                raise AssertionError("malformed XML negative unexpectedly parses")
            name = f"string-{count}-malformed-{index}.xlsx"
            (folder / name).write_bytes(pack(parts))
            records.append(
                {"file": name, "refuse": True, "rows": count, "xml_valid": False}
            )
    (folder / "string-escape-manifest.json").write_text(
        json.dumps(records, indent=2) + "\n"
    )


def generate(folder):
    with zipfile.ZipFile(folder / "source-8.xlsx") as archive:
        original = {name: archive.read(name) for name in archive.namelist()}
    worksheet = "xl/worksheets/sheet1.xml"
    records = []

    def save(name, parts, limit, actual, accepted, compression=zipfile.ZIP_DEFLATED):
        payload = pack(parts, compression)
        (folder / name).write_bytes(payload)
        records.append(
            {
                "file": name,
                "boundary": limit,
                "actual": actual,
                "source_bytes": len(payload),
                "expanded_bytes": sum(map(len, parts.values())),
                "entries": len(parts),
                "parser_accept": accepted,
            }
        )

    for delta in [0, 1]:
        # ZIP_STORED makes source byte count affine in this XML-whitespace pad.
        parts = dict(original)
        target = 2 * 1024 * 1024 + delta
        pad = target - len(pack(parts, zipfile.ZIP_STORED))
        parts[worksheet] = parts[worksheet].replace(
            b"</worksheet>", b" " * pad + b"</worksheet>"
        )
        assert len(pack(parts, zipfile.ZIP_STORED)) == target
        save(
            f"source-byte-{delta}.xlsx",
            parts,
            "source",
            target,
            delta == 0,
            zipfile.ZIP_STORED,
        )

        parts = dict(original)
        target = 8 * 1024 * 1024 + delta
        pad = target - sum(map(len, parts.values()))
        parts[worksheet] = parts[worksheet].replace(
            b"</worksheet>", b" " * pad + b"</worksheet>"
        )
        assert sum(map(len, parts.values())) == target
        save(f"expanded-byte-{delta}.xlsx", parts, "expanded", target, delta == 0)

        parts = dict(original)
        target = 100_000 + delta
        base_nodes = sum(1 for _ in ET.fromstring(parts[worksheet]).iter())
        extension = (
            b"<extLst>"
            + b'<ext uri="urn:invented-capacity"/>' * (target - base_nodes - 1)
            + b"</extLst>"
        )
        parts[worksheet] = parts[worksheet].replace(
            b"</worksheet>", extension + b"</worksheet>"
        )
        assert sum(1 for _ in ET.fromstring(parts[worksheet]).iter()) == target
        # At-limit parser success is inspection only; unsupported extensions
        # cannot be silently admitted as a lossless editable workbook.
        save(f"xml-node-{delta}.xlsx", parts, "xml_nodes", target, delta == 0)

        parts = dict(original)
        target = 256 + delta
        for index in range(target - len(parts)):
            parts[f"unused/part-{index:03}.xml"] = b"<invented/>"
        save(f"zip-entry-{delta}.xlsx", parts, "zip_entries", target, delta == 0)

        parts = dict(original)
        target = 64 + delta
        extension = (
            b"<extLst>"
            + b"<ext>" * (target - 2)
            + b"</ext>" * (target - 2)
            + b"</extLst>"
        )
        parts[worksheet] = parts[worksheet].replace(
            b"</worksheet>", extension + b"</worksheet>"
        )
        save(f"xml-depth-{delta}.xlsx", parts, "xml_depth", target, delta == 0)

    parts = dict(original)
    parts[worksheet] = parts[worksheet].replace(b'<c r="A2"', b'<c s="999" r="A2"', 1)
    save("malformed-style.xlsx", parts, "invalid_style", 999, False)
    parts = dict(original)
    parts[worksheet] = parts[worksheet].replace(b'<row r="2">', b'<row r="1">', 1)
    save("duplicate-coordinate.xlsx", parts, "duplicate_coordinate", 1, False)
    rows = [["v", "v", "v"] for _ in range(8406)]
    remaining = 1024 * 1024 - 8406 * 3
    for row in rows:
        for column in range(3):
            added = min(remaining, 4095)
            row[column] = '"' * (added + 1)
            remaining -= added
    assert remaining == 0
    payload = xlsx(rows)
    assert len(payload) < 2 * 1024 * 1024
    # Literal quote Text is legal XML, but its CSV escaping exceeds 2 MiB.
    (folder / "candidate-output-closure.xlsx").write_bytes(payload)
    # Independently solve an exact encoded CSV boundary while all stored Text
    # remains at the admitted 1 MiB and each field remains <=4096 UTF-8 bytes.
    # Every field starts with a quote, so replacing a letter by another quote
    # increases RFC CSV escaping by exactly one byte without changing Text size.
    rows = [['"', '"', '"'] for _ in range(8406)]
    remaining = 1024 * 1024 - 8406 * 3
    for row in rows:
        for column in range(3):
            added = min(remaining, 4095)
            row[column] += "a" * added
            remaining -= added

    def csv_output(data):
        output = io.StringIO(newline="")
        csv.writer(output, lineterminator="\r\n").writerows(
            [["account_id", "profile_url", "unix_timestamp"], *data]
        )
        return output.getvalue().encode()

    needed = 2 * 1024 * 1024 - len(csv_output(rows))
    for row in rows:
        for column in range(3):
            add_quotes = min(needed, row[column].count("a"))
            row[column] = row[column].replace("a", '"', add_quotes)
            needed -= add_quotes
    assert needed == 0
    assert len(csv_output(rows)) == 2 * 1024 * 1024
    (folder / "export-byte-0.xlsx").write_bytes(xlsx(rows))
    for row in rows:
        if any("a" in value for value in row):
            column = next(c for c, value in enumerate(row) if "a" in value)
            row[column] = row[column].replace("a", '"', 1)
            break
    assert len(csv_output(rows)) == 2 * 1024 * 1024 + 1
    (folder / "export-byte-1.xlsx").write_bytes(xlsx(rows))
    with zipfile.ZipFile(io.BytesIO(xlsx(rows))) as archive:
        short_parts = {name: archive.read(name) for name in archive.namelist()}
    for before, after in [
        (b"account_id", b"a"),
        (b"profile_url", b"b"),
        (b"unix_timestamp", b"c"),
    ]:
        short_parts[worksheet] = short_parts[worksheet].replace(before, after, 1)
    # Actual CSV would be 2 MiB -31 bytes, but body+39-byte reserve is +1.
    assert len(csv_output(rows)) - 39 + len(b"a,b,c\r\n") < 2 * 1024 * 1024
    (folder / "export-short-header-reserve.xlsx").write_bytes(pack(short_parts))
    (folder / "resource-manifest.json").write_text(json.dumps(records, indent=2) + "\n")
    string_escape_variants(folder)
    print(json.dumps(records))


if __name__ == "__main__":
    generate(Path(sys.argv[1]))
