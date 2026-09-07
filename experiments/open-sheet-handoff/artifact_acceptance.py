"""Observe actual browser-saved bytes through Rust, #337 and optional real Office.

The private carrier reader only extracts bounded path/byte entries into fresh
scratch storage. Rust remains the only canonical format and semantic validator.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent
EXPORT = ROOT.parent / "open-sheet-export"
LIMIT = 4 * 1024 * 1024
MAGIC = b"TWDPROJ1"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def carrier_entries(data: bytes) -> list[tuple[str, bytes]]:
    """Test support only: no JSON parser, semantic writer, or public format API."""
    if len(data) > LIMIT or not data.startswith(MAGIC):
        raise ValueError("Outside the pinned no-Date TWDPROJ1 carrier profile")
    position = len(MAGIC)

    def take(count: int) -> bytes:
        nonlocal position
        end = position + count
        if end > len(data):
            raise ValueError("Truncated private carrier")
        result = data[position:end]
        position = end
        return result

    count = struct.unpack("<I", take(4))[0]
    if not 1 <= count <= 1024:
        raise ValueError("Out-of-profile entry count")
    entries, names = [], set()
    for _ in range(count):
        path_length, byte_length = struct.unpack("<HI", take(6))
        name = take(path_length).decode("utf-8")
        path = PurePosixPath(name)
        if (not name or name in names or path.is_absolute()
                or any(part in ("", ".", "..") for part in name.split("/"))
                or "\\" in name or ":" in name or "\0" in name):
            raise ValueError("Unsafe or duplicate carrier entry")
        names.add(name)
        entries.append((name, take(byte_length)))
    if position != len(data):
        raise ValueError("Trailing private carrier bytes")
    # Reject file/directory aliases before writing any entry.
    if any(str(parent) in names for name in names for parent in PurePosixPath(name).parents):
        raise ValueError("Conflicting carrier entry paths")
    return entries


def command(argv: list[str], *, cwd: Path | None = None) -> str:
    result = subprocess.run(argv, cwd=cwd, text=True, capture_output=True,
                            timeout=180, check=False)
    if result.returncode != 0:
        raise AssertionError(f"Command failed: {argv!r}\n{result.stdout}\n{result.stderr}")
    return result.stdout


def tool(variable: str) -> str:
    value = os.environ.get(variable)
    found = shutil.which(value) if value else None
    if found is None:
        raise RuntimeError(f"ENVIRONMENT UNVERIFIED: set {variable} to a real executable")
    return found


def write_json(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def check_native(cli: str, project: Path, model: dict, expected: dict) -> str:
    command([cli, "roproj", "validate", str(project)])
    facts = json.loads(command([cli, "analyze", "document", str(project)]))
    assert facts["source"]["document_id"] == model["id"]
    assert {entity["id"] for entity in facts["entities"]} == set(model["entities"])
    assert {schema["id"] for schema in facts["schemas"]} == set(model["schemas"])
    for entity in model["entities"].values():
        schema = model["schemas"][entity["schema"]]
        for field, value in entity["fields"].items():
            address = f"{entity['key']}.{schema['fields'][field]['key']}"
            fact = json.loads(command([cli, "analyze", "field", str(project), address]))
            assert fact["field"] == {"entity": entity["id"], "field": field}
            assert fact["stored_value"] == value, address
    revisions = set()
    for target, expected_value in expected.items():
        fact = json.loads(command([cli, "formula", "inspect", str(project), target]))
        revisions.add(fact["source_revision"])
        assert fact["outcome"]["calculation"] == {"kind": "value", "value": expected_value}
    assert len(revisions) == 1
    return revisions.pop()


def verify(artifacts: Path, output: Path, mode: str) -> None:
    cli = tool("TACHIKO_BIN")
    node = shutil.which("node")
    if node is None:
        raise RuntimeError("ENVIRONMENT UNVERIFIED: Node required")
    office = tool("REFERENCE_OFFICE_BIN") if mode == "office" else None
    engine = command([office, "--version"]).strip() if office else "NOT RUN"
    sys.path.insert(0, str(EXPORT))
    import acceptance as exporter_oracle  # Existing independent OOXML assertions.

    fixture = json.loads((EXPORT / "canary.json").read_text(encoding="utf-8"))
    evidence = json.loads((artifacts / "browser-evidence.json").read_text(encoding="utf-8"))
    head = command(["git", "rev-parse", "HEAD"], cwd=ROOT).strip()
    assert evidence["head"] == head, "Browser evidence belongs to another checkout HEAD"
    assert evidence["stale_rejection"] == "canonical-bytes-unchanged"
    assert evidence["invalid_rejection"] == "canonical-bytes-unchanged"
    assert evidence["reopened"] == "independently-admitted-and-byte-equal"
    output.mkdir()  # Never overwrite an earlier evidence directory.
    results = []
    for name, rate, expectation in (("before", 0.25, "base"), ("after", 0.5, "tax_half")):
        data = (artifacts / f"{name}.twdproj").read_bytes()
        assert digest(data) == evidence[f"{name}_sha256"], "Saved evidence bytes changed"
        entries = carrier_entries(data)
        model = copy.deepcopy(fixture["document"])
        model["entities"]["e-tax"]["fields"]["f-rate"]["value"] = rate
        with tempfile.TemporaryDirectory(prefix="tachiko-handoff-artifact-") as temporary:
            scratch = Path(temporary)
            project = scratch / "source.roproj"
            project.mkdir()
            for relative, raw in entries:
                destination = project / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                with destination.open("xb") as stream:
                    stream.write(raw)
            revision = check_native(cli, project, model, fixture["expected"][expectation])
            before = exporter_oracle.tree_digest(project)
            xlsx = output / f"{name}.xlsx"
            request = scratch / "request.json"
            layout = fixture["layouts"]["moved"]
            write_json(request, {"source": str(project), "expected_source_revision": revision,
                                 "projection": layout, "output": str(xlsx)})
            report = json.loads(command([node, str(EXPORT / "export.mjs"),
                                         "--request", str(request)]))
            assert report["status"] == "exported"
            assert report["source_revision"] == revision
            assert exporter_oracle.tree_digest(project) == before
            book = exporter_oracle.WorkbookXml(xlsx)
            exporter_oracle.verify_book(book, model, layout)  # Live formulas, NO caches.
            if office:
                recalculated = output / f"{name}-recalculated"
                recalculated.mkdir()
                profile = (scratch / "fresh-office-profile").as_uri()
                command([office, f"-env:UserInstallation={profile}", "--headless",
                         "--convert-to", "xlsx:Calc MS Excel 2007 XML", "--outdir",
                         str(recalculated), str(xlsx)])
                workbook = exporter_oracle.WorkbookXml(recalculated / xlsx.name)
                targets = exporter_oracle.locations(layout)
                for target, expected in fixture["expected"][expectation].items():
                    cell = workbook.cell(*targets[tuple(target.split("."))])
                    assert cell.find(exporter_oracle.q("f")) is not None
                    assert workbook.value(cell) == expected, target
            results.append({"source": name, "source_sha256": digest(data),
                            "xlsx_sha256": digest(xlsx.read_bytes()), "cli_revision": revision,
                            "reference_recalculation": "PASS" if office else "NOT RUN"})
    write_json(output / "artifact-evidence.json", {
        "head": head, "mode": mode, "office_binary": office, "office_version": engine,
        "artifacts": results,
        "claim": "Actual browser-saved source; closed profile only, not general XLSX adoption",
    })
    print(json.dumps({"head": head, "mode": mode, "artifacts": 2, "office": engine}))


class CarrierSelfChecks(unittest.TestCase):
    @staticmethod
    def pack(entries):
        result = bytearray(MAGIC + struct.pack("<I", len(entries)))
        for name, raw in entries:
            name = name.encode()
            result += struct.pack("<HI", len(name), len(raw)) + name + raw
        return bytes(result)

    def test_exact_raw_bytes_are_preserved(self):
        entries = [("one.json", b"not parsed"), ("nested/two.json", b"\0\xff")]
        self.assertEqual(carrier_entries(self.pack(entries)), entries)

    def test_all_truncation_points_are_rejected(self):
        data = self.pack([("a", b"abcdef")])
        for length in range(len(data)):
            with self.subTest(length=length), self.assertRaises(ValueError):
                carrier_entries(data[:length])

    def test_unsafe_paths_and_duplicates_fail(self):
        for name in ("", "../x", "/x", "a/../b", "a//b", "a/./b", "a\\b", "C:x", "a\0b"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                carrier_entries(self.pack([(name, b"x")]))
        with self.assertRaises(ValueError):
            carrier_entries(self.pack([("a", b"x"), ("a", b"y")]))

    def test_file_directory_collision_is_rejected(self):
        with self.assertRaises(ValueError):
            carrier_entries(self.pack([("a", b"x"), ("a/b", b"y")]))

    def test_wrong_profile_trailing_and_empty_carriers_fail(self):
        for data in (b"TWDPROJ2{}", self.pack([]), self.pack([("a", b"x")]) + b"x"):
            with self.subTest(data=data), self.assertRaises(ValueError):
                carrier_entries(data)

    def test_bounded_counts_and_bytes(self):
        for data in (MAGIC + struct.pack("<I", 1025), MAGIC + bytes(LIMIT)):
            with self.assertRaises(ValueError):
                carrier_entries(data)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-check", action="store_true")
    parser.add_argument("--artifacts", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mode", choices=("structural", "office"), default="structural")
    args = parser.parse_args()
    if args.self_check:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CarrierSelfChecks)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        raise SystemExit(0 if result.wasSuccessful() else 1)
    if args.artifacts is None or args.output is None:
        parser.error("--artifacts and --output are required outside --self-check")
    if sys.flags.optimize:
        parser.error("Assertions must not be disabled; do not use python -O")
    verify(args.artifacts.resolve(), args.output.resolve(), args.mode)
