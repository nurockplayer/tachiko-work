"""Steward-owned synthetic source and independent expected candidate; never an importer."""
from __future__ import annotations

import copy
import hashlib
from pathlib import Path

BASE_MAPPING = {
    "profile": "libreoffice-literal-table-experiment/1",
    "document": {"id": "lo355-document", "title": "Inventory snapshot"},
    "schema": {"id": "lo355-inventory", "key": "inventory"},
    "key_header": "code",
    "columns": [
        {"header": "code", "id": "lo355-code", "key": "code", "type": "text"},
        {"header": "quantity", "id": "lo355-quantity", "key": "quantity", "type": "number"},
        {"header": "unit_price", "id": "lo355-price", "key": "unit_price", "type": "number"},
    ],
    "rows": [
        {"source_key": "00123", "id": "lo355-a", "key": "a"},
        {"source_key": "=1+1", "id": "lo355-b", "key": "b"},
        {"source_key": "台灣／日本", "id": "lo355-c", "key": "c"},
    ],
}
RECORDS = [["00123", 42.5, 0.25], ["=1+1", 0, 120], ["台灣／日本", -3, 100]]
SELECTION = {"sheet": "Inventory", "row": 0, "column": 0, "height": 4, "width": 3}


def expected_candidate(records=None, mapping=None):
    """Golden DTO built from declared fixture rows, NOT observed Office cells."""
    mapping = copy.deepcopy(mapping or BASE_MAPPING)
    records = records if records is not None else RECORDS
    columns = mapping["columns"]
    schema = {**mapping["schema"], "fields": {
        field["id"]: {"id": field["id"], "key": field["key"],
                       "field_type": {"type": field["type"]}, "required": True}
        for field in columns
    }}
    entities = {}
    for target, values in zip(mapping["rows"], records):
        entities[target["id"]] = {
            "id": target["id"], "key": target["key"], "schema": schema["id"],
            "fields": {field["id"]: {"kind": field["type"], "value": value}
                       for field, value in zip(columns, values)},
        }
    return {"format_version": 2, **mapping["document"],
            "schemas": {schema["id"]: schema}, "entities": entities}


def synthetic_observation():
    """Native seam fixture; real UNO equivalence is tested separately, never implied."""
    rows = [[field["header"] for field in BASE_MAPPING["columns"]], *RECORDS]
    return {
        "source": {"kind": "saved-file", "sha256": hashlib.sha256(b"native-canary").hexdigest(),
                   "selection": copy.deepcopy(SELECTION)},
        "rows": [[{"kind": "TEXT", "text": value, "merged": False}
                  if isinstance(value, str) else
                  {"kind": "VALUE", "value": value, "number_format": 16, "merged": False}
                  for value in row] for row in rows],
    }


def create_inventory(office, path: Path, *, records=None, moved=False):
    """Create only the trusted fixture, using actual UNO (not spreadsheet XML)."""
    doc = office.load()
    selection = copy.deepcopy(SELECTION)
    if moved:
        selection.update(sheet="在庫 '2026", row=2, column=1)
    doc.Sheets.getByIndex(0).Name = selection["sheet"]
    values = [[field["header"] for field in BASE_MAPPING["columns"]], *(records or RECORDS)]
    for r, row in enumerate(values):
        for c, value in enumerate(row):
            item = doc.Sheets.getByIndex(0).getCellByPosition(c + selection["column"], r + selection["row"])
            if isinstance(value, str):
                item.String = value
            else:
                item.Value = value
    office.save_copy(doc, path)
    office.dispose(doc)
    return selection


def observe_selection(doc, selection, sha256):
    """Independent test observer. Production must not import this test helper."""
    sheet = doc.Sheets.getByName(selection["sheet"])
    rows = []
    for r in range(selection["row"], selection["row"] + selection["height"]):
        row = []
        for c in range(selection["column"], selection["column"] + selection["width"]):
            item = sheet.getCellByPosition(c, r)
            result = {"kind": item.Type.value, "merged": bool(item.IsMerged)}
            if result["kind"] == "TEXT":
                result["text"] = item.String
            elif result["kind"] == "VALUE":
                result.update(value=item.Value,
                              number_format=doc.NumberFormats.getByKey(item.NumberFormat).Type)
            elif result["kind"] == "FORMULA":
                result.update(formula=item.Formula, value=item.Value, error=item.Error)
            row.append(result)
        rows.append(row)
    return {"source": {"kind": "saved-file", "sha256": sha256, "selection": selection}, "rows": rows}
