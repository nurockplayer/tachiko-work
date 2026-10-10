"""Mutate only A2 in a real runtime-exported XLSX; Python standard library only."""
import hashlib
import io
import json
from pathlib import Path
import re
import sys
import zipfile


CASES = [
    ("number-inline", 't="n"', "<is><t>do-not-drop</t></is>", True, "text", None),
    ("default-inline", "", "<is><t>do-not-drop</t></is>", True, "text", None),
    ("inline-value", 't="inlineStr"', "<v>123</v>", True, "text", None),
    ("string-inline", 't="str"', "<is><t>do-not-drop</t></is>", True, "text", None),
    ("formula-number-inline", 't="n"', "<f>1+2</f><is><t>3</t></is>", True, "number", None),
    ("formula-default-inline", "", "<f>1+2</f><is><t>3</t></is>", True, "number", None),
    ("formula-inline-value", 't="inlineStr"', "<f>1+2</f><v>3</v>", True, "number", None),
    ("formula-error-inline", 't="e"', "<f>1+2</f><is><t>#VALUE!</t></is>", True, "number", None),
    ("duplicate-value", 't="n"', "<v>1</v><v>2</v>", True, "number", None),
    ("duplicate-inline", 't="inlineStr"', "<is><t>a</t></is><is><t>b</t></is>", True, "text", None),
    ("mixed-carrier", 't="inlineStr"', "<is><t>a</t></is><v>2</v>", True, "text", None),
    ("valid-inline", 't="inlineStr"', "<is><t>do-not-drop</t></is>", False, "text", {"kind": "text", "value": "do-not-drop"}),
    ("valid-number", 't="n"', "<v>123</v>", False, "number", {"kind": "number", "value": 123}),
    ("valid-default", "", "<v>123</v>", False, "number", {"kind": "number", "value": 123}),
    ("blank-number", 't="n"', "", False, "number", {"kind": "empty"}),
    ("blank-default", "", "", False, "number", {"kind": "empty"}),
    ("blank-value", 't="n"', "<v/>", False, "number", {"kind": "empty"}),
    ("formula-missing-cache", 't="n"', "<f>1+2</f>", False, "number", {"kind": "empty"}),
    ("formula-bad-cache", 't="n"', "<f>1+2</f><v>not-a-number</v>", False, "number", {"kind": "empty"}),
    ("formula-error-cache", 't="e"', "<f>1+2</f><v>#VALUE!</v>", False, "number", {"kind": "empty"}),
    ("formula-inline-cache", 't="inlineStr"', "<f>1+2</f><is><t>3</t></is>", False, "number", {"kind": "empty"}),
]


def replace_cell(xml, attributes, content):
    matches = list(re.finditer(r'<c\b[^>]*\br="A2"[^>]*>.*?</c>', xml, re.S))
    if len(matches) != 1:
        raise ValueError("seed must contain exactly one non-self-closing A2 cell")
    match = matches[0]
    return xml[:match.start()] + f'<c r="A2" {attributes}>{content}</c>' + xml[match.end():]


def generate(seed, destination):
    with zipfile.ZipFile(io.BytesIO(seed)) as archive:
        assert len(archive.namelist()) == len(set(archive.namelist()))
        parts = {name: archive.read(name) for name in archive.namelist()}
    worksheet = "xl/worksheets/sheet1.xml"
    original = parts[worksheet].decode("utf-8")
    destination.mkdir(exist_ok=False)
    manifest = []
    for name, attributes, content, refuse, field_type, expected in CASES:
        target = destination / f"{name}.xlsx"
        changed = replace_cell(original, attributes, content).encode("utf-8")
        with zipfile.ZipFile(target, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path, data in parts.items():
                info = zipfile.ZipInfo(path, (2020, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, changed if path == worksheet else data)
        manifest.append({"file": target.name, "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "refuse": refuse, "field_type": field_type, "expected": expected})
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    generate(Path(sys.argv[1]).read_bytes(), Path(sys.argv[2]))
