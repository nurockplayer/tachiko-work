"""Issue #355 experimental LibreOffice -> Tachiko bridge.

Two bounded seams share one literal transcription core:

* :func:`analyze_snapshot` -- a disposable internal integration boundary that
  maps an explicit UNO observation rectangle through a caller-supplied mapping
  into a direct-ro/v2 candidate and forwards it to the real ``tachiko validate``
  / ``analyze document`` / ``analyze validation`` commands.  It owns no semantic
  authority and never mutates caller inputs.
* ``bridge.py --request`` -- the actual saved-file command.  It captures the
  named file's bytes once, opens a private copy in a fresh isolated LibreOffice
  process/profile/pipe and turns the explicit selection into that observation.

Stdlib only; pyuno is imported lazily from the host interpreter.  This module
never imports the Steward fixtures/oracles, never evaluates a formula, never
writes an Office document or an existing Tachiko project and never replaces an
existing output.  ``ReadOnly`` is supplementary, not a security guarantee.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import uuid

FORMAT_VERSION = 2
MAX_SOURCE_BYTES = 2 * 1024 * 1024
MAX_DATA_ROWS = 100
MAX_COLUMNS = 16
MIN_HEIGHT = 2
MAX_HEIGHT = MAX_DATA_ROWS + 1
ALLOWED_SUFFIXES = (".ods", ".xlsx")
SHA256_RE = re.compile(r"\A[0-9a-f]{64}\Z")
NATIVE_TIMEOUT_SECONDS = 45
OFFICE_TIMEOUT_SECONDS = 90
KIND_EMPTY, KIND_TEXT, KIND_VALUE, KIND_FORMULA = "EMPTY", "TEXT", "VALUE", "FORMULA"
NUMBER_FORMAT_MASK = 16  # com.sun.star.util.NumberFormat.NUMBER
NUMBER_FORMAT_DEFINED = 1  # com.sun.star.util.NumberFormat.DEFINED

# Truthful fidelity ledger for a successful analysis.  Order is not semantic.
SUCCESS_LEDGER = (
    ("exact", "selected_scalars"),
    ("strengthening", "explicit_schema"),
    ("strengthening", "new_identity"),
    ("presentation", "presentation_not_imported"),
    ("unresolved", "outside_selection_unassessed"),
)

_CLASSIFICATION = {
    "invalid_snapshot": "invalid",
    "invalid_mapping": "invalid",
    "mapping_mismatch": "mismatch",
    "unsupported_cell": "unsupported",
    "type_mismatch": "type",
    "native_failure": "native",
    "saved_file_required": "host",
    "invalid_input": "host",
    "source_changed": "host",
    "invalid_output": "host",
}


class Rejection(Exception):
    """Fail-closed outcome carrying the exact code and its ledger entry."""

    def __init__(self, code: str, *, detail: str = "", locator: dict | None = None):
        super().__init__(code)
        self.code = code
        self.detail = detail
        self.locator = locator

    def as_result(self) -> dict:
        entry = {
            "classification": _CLASSIFICATION.get(self.code, "invalid"),
            "code": self.code,
        }
        if self.detail:
            entry["detail"] = self.detail
        if self.locator:
            entry["source"] = self.locator
        return {"status": "rejected", "code": self.code, "ledger": [entry]}


def _reject(code: str, *, detail: str = "", locator: dict | None = None):
    raise Rejection(code, detail=detail, locator=locator)


def _is_index(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _nonempty_str(value) -> bool:
    return isinstance(value, str) and bool(value)


def _locator(selection: dict, row: int, column: int) -> dict:
    return {"sheet": selection["sheet"], "row": row, "column": column}


def _success_ledger() -> list:
    return [
        {"classification": classification, "code": code}
        for classification, code in SUCCESS_LEDGER
    ]


# --------------------------------------------------------------------------- #
# Snapshot / mapping admission (no semantic authority, pure functions)
# --------------------------------------------------------------------------- #
def _validate_selection(selection):
    if not isinstance(selection, dict):
        _reject("invalid_snapshot", detail="selection must be an object")
    if set(selection) != {"sheet", "row", "column", "height", "width"}:
        _reject("invalid_snapshot", detail="selection has unexpected or missing keys")
    if not _nonempty_str(selection.get("sheet")):
        _reject("invalid_snapshot", detail="selection.sheet must be a non-empty string")
    for key in ("row", "column", "height", "width"):
        if not _is_index(selection.get(key)):
            _reject(
                "invalid_snapshot",
                detail=f"selection.{key} must be a non-Boolean integer",
            )
    row, column = selection["row"], selection["column"]
    height, width = selection["height"], selection["width"]
    if row < 0 or column < 0:
        _reject("invalid_snapshot", detail="selection origin must not be negative")
    if not MIN_HEIGHT <= height <= MAX_HEIGHT:
        _reject(
            "invalid_snapshot",
            detail=f"selection height must be {MIN_HEIGHT}..{MAX_HEIGHT} including one header row",
        )
    if not 1 <= width <= MAX_COLUMNS:
        _reject("invalid_snapshot", detail=f"selection width must be 1..{MAX_COLUMNS}")


def _validate_observation(observed):
    if not isinstance(observed, dict):
        _reject("invalid_snapshot", detail="observation must be an object")
    if set(observed) != {"source", "rows"}:
        _reject("invalid_snapshot", detail="observation has unexpected or missing keys")
    source = observed.get("source")
    if not isinstance(source, dict):
        _reject("invalid_snapshot", detail="observation.source must be an object")
    if set(source) != {"kind", "sha256", "selection"}:
        _reject(
            "invalid_snapshot",
            detail="observation.source has unexpected or missing keys",
        )
    if source.get("kind") != "saved-file":
        _reject(
            "invalid_snapshot", detail="only a saved-file observation is admissible"
        )
    if not isinstance(source.get("sha256"), str) or not SHA256_RE.match(
        source["sha256"]
    ):
        _reject(
            "invalid_snapshot",
            detail="observation.source.sha256 must be lowercase 64-hex",
        )
    _validate_selection(source.get("selection"))
    selection = source["selection"]
    rows = observed.get("rows")
    if not isinstance(rows, list) or len(rows) < MIN_HEIGHT:
        _reject(
            "invalid_snapshot",
            detail="observation.rows must be a rectangle with one header row",
        )
    if len(rows) != selection["height"]:
        _reject(
            "invalid_snapshot",
            detail="observation.rows does not match the declared selection height",
        )
    for row in rows:
        if not isinstance(row, list) or len(row) != selection["width"]:
            _reject(
                "invalid_snapshot",
                detail="observation.rows does not match the declared selection width",
            )
        for cell in row:
            if not isinstance(cell, dict) or cell.get("kind") not in (
                KIND_EMPTY,
                KIND_TEXT,
                KIND_VALUE,
                KIND_FORMULA,
            ):
                _reject(
                    "invalid_snapshot",
                    detail="every observed cell needs a known UNO kind",
                )
            if not isinstance(cell.get("merged"), bool):
                _reject(
                    "invalid_snapshot",
                    detail="every observed cell needs a Boolean merged flag",
                )


def _validate_mapping(mapping):
    if not isinstance(mapping, dict):
        _reject("invalid_mapping", detail="mapping must be an object")
    if set(mapping) != {
        "profile",
        "document",
        "schema",
        "key_header",
        "columns",
        "rows",
    }:
        _reject("invalid_mapping", detail="mapping has unexpected or missing keys")
    if mapping.get("profile") != "libreoffice-literal-table-experiment/1":
        _reject(
            "invalid_mapping",
            detail="mapping.profile is not the closed experiment profile",
        )
    document = mapping.get("document")
    if (
        not isinstance(document, dict)
        or set(document) != {"id", "title"}
        or not _nonempty_str(document.get("id"))
        or not _nonempty_str(document.get("title"))
    ):
        _reject(
            "invalid_mapping", detail="mapping.document needs non-empty id and title"
        )
    schema = mapping.get("schema")
    if (
        not isinstance(schema, dict)
        or set(schema) != {"id", "key"}
        or not _nonempty_str(schema.get("id"))
        or not _nonempty_str(schema.get("key"))
    ):
        _reject("invalid_mapping", detail="mapping.schema needs non-empty id and key")
    if not _nonempty_str(mapping.get("key_header")):
        _reject(
            "invalid_mapping", detail="mapping.key_header must be a non-empty string"
        )

    columns = mapping.get("columns")
    if not isinstance(columns, list) or not columns:
        _reject("invalid_mapping", detail="mapping.columns must be a non-empty list")
    seen_ids, seen_keys, seen_headers = set(), set(), set()
    for column in columns:
        if (
            not isinstance(column, dict)
            or set(column) != {"header", "id", "key", "type"}
            or not _nonempty_str(column.get("header"))
            or not _nonempty_str(column.get("id"))
            or not _nonempty_str(column.get("key"))
        ):
            _reject(
                "invalid_mapping",
                detail="every mapping column needs non-empty header/id/key",
            )
        if column.get("type") not in ("text", "number"):
            _reject(
                "invalid_mapping",
                detail=f"unknown mapping column type: {column.get('type')!r}",
            )
        if column["id"] in seen_ids:
            _reject(
                "invalid_mapping",
                detail=f"duplicate mapping column id: {column['id']!r}",
            )
        if column["key"] in seen_keys:
            _reject(
                "invalid_mapping",
                detail=f"duplicate mapping column key: {column['key']!r}",
            )
        if column["header"] in seen_headers:
            _reject(
                "invalid_mapping",
                detail=f"duplicate mapping column header: {column['header']!r}",
            )
        seen_ids.add(column["id"])
        seen_keys.add(column["key"])
        seen_headers.add(column["header"])
    if mapping["key_header"] not in seen_headers:
        _reject(
            "invalid_mapping",
            detail="mapping.key_header must name one mapped column header",
        )

    rows = mapping.get("rows")
    if not isinstance(rows, list) or not rows:
        _reject("invalid_mapping", detail="mapping.rows must be a non-empty list")
    seen_row_ids, seen_row_keys, seen_source_keys = set(), set(), set()
    for row in rows:
        if (
            not isinstance(row, dict)
            or set(row) != {"source_key", "id", "key"}
            or not _nonempty_str(row.get("source_key"))
            or not _nonempty_str(row.get("id"))
            or not _nonempty_str(row.get("key"))
        ):
            _reject(
                "invalid_mapping",
                detail="every mapping row needs non-empty source_key/id/key",
            )
        if row["id"] in seen_row_ids:
            _reject(
                "invalid_mapping", detail=f"duplicate mapping row id: {row['id']!r}"
            )
        if row["key"] in seen_row_keys:
            _reject(
                "invalid_mapping", detail=f"duplicate mapping row key: {row['key']!r}"
            )
        if row["source_key"] in seen_source_keys:
            _reject(
                "invalid_mapping",
                detail=f"duplicate mapping source key: {row['source_key']!r}",
            )
        seen_row_ids.add(row["id"])
        seen_row_keys.add(row["key"])
        seen_source_keys.add(row["source_key"])


# --------------------------------------------------------------------------- #
# Literal transcription
# --------------------------------------------------------------------------- #
def _map_geometry(observed, mapping):
    """Resolve headers/keys by declared text; never by A1 position or sheet name."""
    rows = observed["rows"]
    selection = observed["source"]["selection"]
    width = selection["width"]

    header_index = {}
    for index, cell in enumerate(rows[0]):
        if cell["merged"]:
            _reject(
                "unsupported_cell",
                locator=_locator(
                    selection, selection["row"], selection["column"] + index
                ),
                detail="merged header cell",
            )
        if cell["kind"] == KIND_FORMULA:
            _reject(
                "unsupported_cell",
                locator=_locator(
                    selection, selection["row"], selection["column"] + index
                ),
                detail="formula is not a literal header",
            )
        if cell["kind"] != KIND_TEXT:
            _reject(
                "mapping_mismatch", detail="every selected header cell must be Text"
            )
        text = cell.get("text")
        if not _nonempty_str(text):
            _reject(
                "mapping_mismatch",
                detail="every selected header cell needs non-empty text",
            )
        if text in header_index:
            _reject("mapping_mismatch", detail=f"duplicate selected header: {text!r}")
        header_index[text] = index

    column_index = {}
    used = set()
    for column in mapping["columns"]:
        index = header_index.get(column["header"])
        if index is None or index in used:
            _reject(
                "mapping_mismatch",
                detail=f"mapped header {column['header']!r} is missing or ambiguous",
            )
        used.add(index)
        column_index[column["id"]] = index
    if len(used) != width:
        _reject(
            "mapping_mismatch",
            detail="the selection has headers no mapping column covers",
        )

    key_index = header_index[mapping["key_header"]]
    targets = {row["source_key"]: row for row in mapping["rows"]}
    target_row_index = {}
    for index in range(1, len(rows)):
        cell = rows[index][key_index]
        locator = _locator(
            selection, selection["row"] + index, selection["column"] + key_index
        )
        if cell["merged"]:
            _reject(
                "unsupported_cell", locator=locator, detail="merged source-key cell"
            )
        if cell["kind"] == KIND_FORMULA:
            _reject(
                "unsupported_cell",
                locator=locator,
                detail="formula is not a literal source key",
            )
        if cell["kind"] != KIND_TEXT:
            _reject("mapping_mismatch", detail="every selected source key must be Text")
        key = cell.get("text")
        if not _nonempty_str(key) or key not in targets or key in target_row_index:
            _reject(
                "mapping_mismatch",
                detail="unknown, empty or duplicate selected source key",
            )
        target_row_index[key] = index
    return column_index, target_row_index


def _scalar(cell, column, locator):
    if cell["merged"]:
        _reject("unsupported_cell", locator=locator, detail="merged cell in selection")
    kind = cell["kind"]
    if kind == KIND_FORMULA:
        _reject(
            "unsupported_cell",
            locator=locator,
            detail="formula is not a literal scalar",
        )
    if kind == KIND_EMPTY:
        _reject(
            "type_mismatch", locator=locator, detail="blank cell is not a mapped scalar"
        )
    if kind == KIND_TEXT:
        if column["type"] != "text":
            _reject(
                "type_mismatch",
                locator=locator,
                detail="Text cell for a non-Text mapping column",
            )
        if not isinstance(cell.get("text"), str):
            _reject(
                "invalid_snapshot",
                locator=locator,
                detail="Text cell has no text value",
            )
        return cell["text"]
    if column["type"] != "number":
        _reject(
            "type_mismatch",
            locator=locator,
            detail="Number cell for a non-Number mapping column",
        )
    value = cell.get("value")
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        _reject(
            "type_mismatch",
            locator=locator,
            detail="Number cell must hold a finite binary64 value",
        )
    if isinstance(value, float) and not math.isfinite(value):
        _reject(
            "type_mismatch", locator=locator, detail="nonfinite value is not admitted"
        )
    number_format = cell.get("number_format")
    if not _is_index(number_format):
        _reject(
            "invalid_snapshot",
            locator=locator,
            detail="Number cell has no integer number format",
        )
    if (number_format & ~NUMBER_FORMAT_DEFINED) != NUMBER_FORMAT_MASK:
        _reject(
            "unsupported_cell",
            locator=locator,
            detail="number format is not plain decimal NUMBER (presentation-only flags rejected)",
        )
    return value


def _build_candidate(observed, mapping):
    rows = observed["rows"]
    selection = observed["source"]["selection"]
    column_index, target_row_index = _map_geometry(observed, mapping)

    schema = dict(mapping["schema"])
    schema["fields"] = {
        column["id"]: {
            "id": column["id"],
            "key": column["key"],
            "field_type": {"type": column["type"]},
            "required": True,
        }
        for column in mapping["columns"]
    }
    entities = {}
    for target in mapping["rows"]:
        index = target_row_index[target["source_key"]]
        fields = {}
        for column in mapping["columns"]:
            column_position = column_index[column["id"]]
            cell = rows[index][column_position]
            locator = _locator(
                selection,
                selection["row"] + index,
                selection["column"] + column_position,
            )
            fields[column["id"]] = {
                "kind": column["type"],
                "value": _scalar(cell, column, locator),
            }
        entities[target["id"]] = {
            "id": target["id"],
            "key": target["key"],
            "schema": schema["id"],
            "fields": fields,
        }
    return {
        "format_version": FORMAT_VERSION,
        **mapping["document"],
        "schemas": {schema["id"]: schema},
        "entities": entities,
    }


# --------------------------------------------------------------------------- #
# Native authority
# --------------------------------------------------------------------------- #
def _write_json(path: Path, value) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, allow_nan=False) + "\n", encoding="utf-8"
    )


def _run_native(binary: str, args: list, *, expect_json: bool):
    try:
        result = subprocess.run(
            [binary, *args],
            capture_output=True,
            text=True,
            timeout=NATIVE_TIMEOUT_SECONDS,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        _reject(
            "native_failure", detail=f"{args[0]} could not run: {type(exc).__name__}"
        )
    if result.returncode != 0:
        _reject("native_failure", detail=f"{args[0]} exited {result.returncode}")
    if not expect_json:
        return None
    try:
        return json.loads(result.stdout)
    except ValueError:
        _reject("native_failure", detail=f"{args[0]} {args[1]} did not emit JSON")


def _native_reports(binary: str, candidate: dict, source_state: str) -> dict:
    scratch = Path(tempfile.mkdtemp(prefix="lo355-candidate-"))
    try:
        path = scratch / "candidate.ro"
        _write_json(path, candidate)
        _run_native(binary, ["validate", str(path)], expect_json=False)
        document = _run_native(
            binary,
            ["analyze", "document", str(path), "--source-state", source_state],
            expect_json=True,
        )
        validation = _run_native(
            binary,
            ["analyze", "validation", str(path), "--source-state", source_state],
            expect_json=True,
        )
        return {"document": document, "validation": validation}
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def analyze_snapshot(observation: dict, mapping: dict, *, tachiko_bin: str) -> dict:
    """Map one explicit UNO rectangle literally, then let real Tachiko report.

    Pure with respect to ``observation``/``mapping``; returns either the
    successful ``analyzed`` structure or the fail-closed ``rejected`` shape.
    """
    try:
        observed = copy.deepcopy(observation)
        mapping_copy = copy.deepcopy(mapping)
        _validate_observation(observed)
        _validate_mapping(mapping_copy)
        source = observed["source"]
        selection = source["selection"]
        if (
            len(mapping_copy["rows"]) != selection["height"] - 1
            or len(mapping_copy["columns"]) != selection["width"]
        ):
            _reject(
                "mapping_mismatch",
                detail="mapping row/column counts do not cover the whole selection",
            )
        candidate = _build_candidate(observed, mapping_copy)
        native = _native_reports(tachiko_bin, candidate, source["sha256"])
        return {
            "status": "analyzed",
            "source": source,
            "candidate": candidate,
            "native": native,
            "ledger": _success_ledger(),
        }
    except Rejection as rejection:
        return rejection.as_result()


# --------------------------------------------------------------------------- #
# Actual saved-file command: private LibreOffice adapter
# --------------------------------------------------------------------------- #
def _private_office_environment() -> dict:
    """Keep the owned office process bound to its selected app bundle.

    LibreOffice's Python launcher exports these bootstrap variables so pyuno can
    find its runtime.  A child ``soffice`` must not inherit them: they can make
    the selected app mix its Resources/Frameworks with the Python launcher's
    bundle instead of using the app named by ``REFERENCE_OFFICE_BIN``.
    """
    environment = dict(os.environ)
    for key in ("PYTHONHOME", "PYTHONPATH", "UNO_PATH", "URE_BOOTSTRAP"):
        environment.pop(key, None)
    return environment


class _LibreOffice:
    """Fresh, isolated, owned LibreOffice process; never a remote/shared session."""

    def __init__(self, binary: str):
        self.binary = binary
        self._temp = tempfile.TemporaryDirectory(prefix="lo355-office-")
        os.chmod(self._temp.name, 0o700)
        self.scratch = Path(self._temp.name)
        self._log = (self.scratch / "office.log").open("wb")
        self.process = None
        self.watchdog = None
        self.timed_out = False
        self.uno = None
        self.desktop = None

    def props(self, **values):
        result = []
        for name, value in values.items():
            item = self.uno.createUnoStruct("com.sun.star.beans.PropertyValue")
            item.Name, item.Value = name, value
            result.append(item)
        return tuple(result)

    def _expire(self):
        self.timed_out = True
        if self.process is not None:
            try:
                os.killpg(os.getpgid(self.process.pid), signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass

    def start(self):
        import uno  # supplied by the host interpreter at runtime only

        self.uno = uno
        endpoint = (
            f"pipe,name=tachiko355_{uuid.uuid4().hex};urp;StarOffice.ComponentContext"
        )
        profile = (self.scratch / "profile").as_uri()
        self.process = subprocess.Popen(
            [
                self.binary,
                f"-env:UserInstallation={profile}",
                "--headless",
                "--nologo",
                "--nodefault",
                "--norestore",
                "--nofirststartwizard",
                "--nolockcheck",
                f"--accept={endpoint}",
            ],
            stdin=subprocess.DEVNULL,
            stdout=self._log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            env=_private_office_environment(),
        )
        self.watchdog = threading.Timer(OFFICE_TIMEOUT_SECONDS, self._expire)
        self.watchdog.daemon = True
        self.watchdog.start()
        local = uno.getComponentContext()
        resolver = local.ServiceManager.createInstanceWithContext(
            "com.sun.star.bridge.UnoUrlResolver", local
        )
        deadline = time.monotonic() + 30
        while True:
            try:
                context = resolver.resolve(f"uno:{endpoint}")
                break
            except Exception as exc:  # NoConnectException while the pipe warms up
                if self.process.poll() is not None or time.monotonic() >= deadline:
                    raise Rejection(
                        "invalid_input",
                        detail=f"private UNO connection failed: {type(exc).__name__}",
                    ) from exc
                time.sleep(0.2)
        self.desktop = context.ServiceManager.createInstanceWithContext(
            "com.sun.star.frame.Desktop", context
        )
        return self

    def load(self, path: Path):
        document = self.desktop.loadComponentFromURL(
            path.as_uri(),
            "_blank",
            0,
            self.props(
                Hidden=True,
                ReadOnly=True,
                PickListEntry=False,
                MacroExecutionMode=self.uno.getConstantByName(
                    "com.sun.star.document.MacroExecMode.NEVER_EXECUTE"
                ),
                UpdateDocMode=self.uno.getConstantByName(
                    "com.sun.star.document.UpdateDocMode.NO_UPDATE"
                ),
            ),
        )
        if document is None:
            raise Rejection("invalid_input", detail="LibreOffice returned no document")
        return document

    def observe(self, document, selection: dict) -> dict:
        if not document.Sheets.hasByName(selection["sheet"]):
            raise Rejection(
                "invalid_snapshot",
                detail="selection sheet is not in the saved document",
            )
        sheet = document.Sheets.getByName(selection["sheet"])
        rows = []
        try:
            for row in range(selection["row"], selection["row"] + selection["height"]):
                cells = []
                for column in range(
                    selection["column"], selection["column"] + selection["width"]
                ):
                    item = sheet.getCellByPosition(column, row)
                    kind = item.Type.value
                    merged = bool(item.IsMerged)
                    if kind == KIND_TEXT:
                        cells.append(
                            {"kind": KIND_TEXT, "text": item.String, "merged": merged}
                        )
                    elif kind == KIND_VALUE:
                        number_format = document.NumberFormats.getByKey(
                            item.NumberFormat
                        ).Type
                        cells.append(
                            {
                                "kind": KIND_VALUE,
                                "value": item.Value,
                                "number_format": int(number_format),
                                "merged": merged,
                            }
                        )
                    elif kind == KIND_FORMULA:
                        cells.append(
                            {
                                "kind": KIND_FORMULA,
                                "formula": item.Formula,
                                "value": item.Value,
                                "error": item.Error,
                                "merged": merged,
                            }
                        )
                    else:
                        cells.append({"kind": KIND_EMPTY, "merged": merged})
                rows.append(cells)
        except Rejection:
            raise
        except Exception as exc:
            raise Rejection(
                "invalid_snapshot",
                detail=f"selection does not fit the saved document: {type(exc).__name__}",
            ) from exc
        return rows

    def dispose(self, document) -> None:
        try:
            document.dispose()
        except Exception:
            pass

    def close(self) -> None:
        if self.watchdog is not None:
            self.watchdog.cancel()
        if self.process is not None:
            try:
                os.killpg(os.getpgid(self.process.pid), signal.SIGTERM)
            except (ProcessLookupError, PermissionError):
                pass
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(os.getpgid(self.process.pid), signal.SIGKILL)
                except (ProcessLookupError, PermissionError):
                    pass
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    pass
        self._log.close()
        self._temp.cleanup()


def _observe_saved_copy(
    office_bin: str, data: bytes, suffix: str, selection: dict, sha256: str
) -> dict:
    office = _LibreOffice(office_bin)
    document = None
    try:
        office.start()
        private = office.scratch / f"source{suffix}"
        private.write_bytes(data)
        os.chmod(private, 0o600)
        document = office.load(private)
        rows = office.observe(document, selection)
        source = {
            "kind": "saved-file",
            "sha256": sha256,
            "selection": copy.deepcopy(selection),
        }
        return {"source": source, "rows": rows}
    finally:
        if document is not None:
            office.dispose(document)
        office.close()


# --------------------------------------------------------------------------- #
# Actual saved-file command: host guards, staging and publication
# --------------------------------------------------------------------------- #
def _load_request(path: Path) -> dict:
    try:
        raw = path.read_bytes()
        request = json.loads(raw.decode("utf-8"))
    except (OSError, ValueError, UnicodeDecodeError) as exc:
        _reject(
            "invalid_input", detail=f"request JSON unreadable: {type(exc).__name__}"
        )
    if not isinstance(request, dict):
        _reject("invalid_input", detail="request must be one JSON object")
    return request


def _capture_source(source) -> tuple:
    if not isinstance(source, dict):
        _reject("invalid_input", detail="request.source must be an object")
    if source.get("kind") != "saved-file":
        _reject(
            "saved_file_required",
            detail="only a saved regular ODS/XLSX file can be analysed; save a separate copy first",
        )
    location = source.get("path")
    if not _nonempty_str(location):
        _reject(
            "invalid_input", detail="request.source.path must be a non-empty string"
        )
    path = Path(location)
    if not path.is_absolute():
        _reject("invalid_input", detail="request.source.path must be absolute")
    try:
        info = path.lstat()
    except OSError as exc:
        _reject("invalid_input", detail=f"source is not readable: {type(exc).__name__}")
    if stat.S_ISLNK(info.st_mode):
        _reject("invalid_input", detail="source must not be a symbolic link")
    if not stat.S_ISREG(info.st_mode):
        _reject("invalid_input", detail="source must be a regular file")
    if path.suffix.lower() not in ALLOWED_SUFFIXES:
        _reject("invalid_input", detail="source must be a .ods or .xlsx file")
    if info.st_size > MAX_SOURCE_BYTES:
        _reject("invalid_input", detail="source exceeds the 2 MiB experiment bound")
    expected = source.get("expected_sha256")
    if not _nonempty_str(expected) or not SHA256_RE.match(expected):
        _reject(
            "invalid_input",
            detail="request.source.expected_sha256 must be lowercase 64-hex",
        )
    try:
        data = path.read_bytes()
    except OSError as exc:
        _reject(
            "invalid_input",
            detail=f"source disappeared while captured: {type(exc).__name__}",
        )
    if len(data) > MAX_SOURCE_BYTES:
        _reject("invalid_input", detail="source exceeds the 2 MiB experiment bound")
    if hashlib.sha256(data).hexdigest() != expected:
        _reject(
            "source_changed",
            detail="captured source bytes do not match the expected SHA-256",
        )
    return path, data, expected


def _check_output(output, source_path: Path, request_path: Path) -> Path:
    if not _nonempty_str(output):
        _reject("invalid_output", detail="request.output must be a non-empty string")
    path = Path(output)
    if not path.is_absolute():
        _reject("invalid_output", detail="request.output must be absolute")
    if path.is_symlink() or path.exists():
        _reject(
            "invalid_output",
            detail="refusing to replace an existing or symlinked output",
        )
    if os.path.abspath(path) == os.path.abspath(source_path):
        _reject("invalid_output", detail="output must not alias the Office source")
    if os.path.abspath(path) == os.path.abspath(request_path):
        _reject("invalid_output", detail="output must not alias the request file")
    if not path.parent.is_dir():
        _reject("invalid_output", detail="output parent directory does not exist")
    return path


def _source_unchanged(path: Path, expected: str) -> bool:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest() == expected
    except OSError:
        return False


def _publish_no_replace(path: Path, payload: bytes) -> None:
    """Stage privately beside the target, then link once without replacing."""
    handle, staged = tempfile.mkstemp(
        dir=str(path.parent), prefix=".lo355-report-", suffix=".tmp"
    )
    try:
        with os.fdopen(handle, "wb") as stream:
            stream.write(payload)
        os.chmod(staged, 0o600)
        try:
            os.link(staged, path)
        except FileExistsError:
            _reject(
                "invalid_output", detail="another writer won the output destination"
            )
    finally:
        try:
            os.unlink(staged)
        except OSError:
            pass


def run_request(request_path: Path) -> dict:
    request = _load_request(request_path)
    if set(request) != {"source", "mapping", "output"}:
        _reject("invalid_input", detail="request has unexpected or missing keys")
    source = request.get("source")
    source_path, data, expected = _capture_source(source)
    selection = source.get("selection") if isinstance(source, dict) else None
    _validate_selection(selection)
    output = _check_output(request.get("output"), source_path, request_path)

    tachiko_bin = os.environ.get("TACHIKO_BIN")
    if not _nonempty_str(tachiko_bin):
        _reject("native_failure", detail="TACHIKO_BIN is not set")
    office_bin = os.environ.get("REFERENCE_OFFICE_BIN")
    if not _nonempty_str(office_bin):
        _reject("invalid_input", detail="REFERENCE_OFFICE_BIN is not set")

    observed = _observe_saved_copy(
        office_bin, data, source_path.suffix.lower(), selection, expected
    )
    result = analyze_snapshot(observed, request.get("mapping"), tachiko_bin=tachiko_bin)
    if result.get("status") != "analyzed":
        return result

    if not _source_unchanged(source_path, expected):
        _reject(
            "source_changed",
            detail="source changed while the private copy was analysed",
        )
    payload = (json.dumps(result, ensure_ascii=False, allow_nan=False) + "\n").encode(
        "utf-8"
    )
    _publish_no_replace(output, payload)
    return result


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--request", required=True, type=Path, help="absolute saved-file request JSON"
    )
    args = parser.parse_args(argv)
    try:
        response = run_request(args.request)
        code = 0 if response.get("status") == "analyzed" else 1
    except Rejection as rejection:
        response = rejection.as_result()
        code = 1
    except Exception as exc:  # fail closed; never present an internal fault as success
        response = Rejection(
            "invalid_input", detail=f"{type(exc).__name__}: {exc}"
        ).as_result()
        code = 1
    sys.stdout.write(json.dumps(response, ensure_ascii=False, allow_nan=False) + "\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
