"""Delivery-agent unit coverage for ``bridge.py``.

Independent of the Steward oracles: these tests exercise the native adapter
with a stub ``tachiko`` and the saved-file host guards without pyuno or
LibreOffice.  They never assert Steward acceptance behavior.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import bridge  # noqa: E402  (stdlib-only module; pyuno is imported lazily on demand)

STUB_SOURCE = """#!{python}
import json, os, pathlib, sys
args = sys.argv[1:]
mode = os.environ.get("BRIDGE_STUB_MODE", "normal")
if mode == "fail":
    sys.stderr.write("forced native rejection\\n")
    sys.exit(73)
if args and args[0] == "validate":
    if len(args) < 2 or not pathlib.Path(args[1]).is_file():
        sys.exit(64)
    sys.exit(0)
label = args[args.index("--source-state") + 1] if "--source-state" in args else "unknown"
if args[:2] == ["analyze", "document"]:
    if mode == "bad-json":
        print("not native JSON")
        sys.exit(0)
    print(json.dumps({{"source": {{"document_id": "stub", "source_label": label}},
                       "schemas": [], "entities": []}}))
    sys.exit(0)
if args[:2] == ["analyze", "validation"]:
    print(json.dumps({{"source": {{"document_id": "stub", "source_label": label}},
                       "is_valid": True, "diagnostics": []}}))
    sys.exit(0)
sys.exit(2)
"""


def text(value):
    return {"kind": "TEXT", "text": value, "merged": False}


def number(value, number_format=16):
    return {
        "kind": "VALUE",
        "value": value,
        "number_format": number_format,
        "merged": False,
    }


def observation():
    return {
        "source": {
            "kind": "saved-file",
            "sha256": "a" * 64,
            "selection": {
                "sheet": "Sheet1",
                "row": 0,
                "column": 0,
                "height": 3,
                "width": 2,
            },
        },
        "rows": [
            [text("code"), text("qty")],
            [text("k1"), number(1)],
            [text("k2"), number(2.5)],
        ],
    }


def mapping():
    return {
        "profile": "libreoffice-literal-table-experiment/1",
        "document": {"id": "unit-document", "title": "Unit document"},
        "schema": {"id": "unit-schema", "key": "unit"},
        "key_header": "code",
        "columns": [
            {"header": "code", "id": "unit-code", "key": "code", "type": "text"},
            {"header": "qty", "id": "unit-qty", "key": "qty", "type": "number"},
        ],
        "rows": [
            {"source_key": "k1", "id": "unit-e1", "key": "k1"},
            {"source_key": "k2", "id": "unit-e2", "key": "k2"},
        ],
    }


class NativeAdapter(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lo355-unit-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.stub = self.directory / "tachiko-stub"
        self.stub.write_text(
            STUB_SOURCE.format(python=sys.executable), encoding="utf-8"
        )
        self.stub.chmod(0o700)

    def analyze(self, observed=None, mapping_value=None, *, mode="normal"):
        observed = observation() if observed is None else observed
        mapping_value = mapping() if mapping_value is None else mapping_value
        before = (
            json.loads(json.dumps(observed)),
            json.loads(json.dumps(mapping_value)),
        )
        previous = os.environ.get("BRIDGE_STUB_MODE")
        os.environ["BRIDGE_STUB_MODE"] = mode
        try:
            result = bridge.analyze_snapshot(
                observed, mapping_value, tachiko_bin=str(self.stub)
            )
        finally:
            if previous is None:
                os.environ.pop("BRIDGE_STUB_MODE", None)
            else:
                os.environ["BRIDGE_STUB_MODE"] = previous
        self.assertEqual(
            (observed, mapping_value), before, "adapter mutated caller input"
        )
        return result

    def assert_rejected(self, result, code):
        self.assertEqual(result.get("status"), "rejected", result)
        self.assertEqual(result.get("code"), code, result)
        self.assertNotIn("candidate", result)
        self.assertNotIn("native", result)
        self.assertTrue(
            any(entry.get("code") == code for entry in result["ledger"]), result
        )

    def test_success_shape_and_ledger(self):
        result = self.analyze()
        self.assertEqual(result["status"], "analyzed", result)
        self.assertEqual(
            result["candidate"]["entities"]["unit-e1"]["fields"]["unit-qty"]["value"], 1
        )
        self.assertEqual(
            result["candidate"]["entities"]["unit-e2"]["fields"]["unit-code"]["value"],
            "k2",
        )
        self.assertEqual(result["source"], observation()["source"])
        self.assertEqual(set(result["native"]), {"document", "validation"})
        codes = {
            (entry.get("classification"), entry.get("code"))
            for entry in result["ledger"]
        }
        self.assertTrue(set(bridge.SUCCESS_LEDGER) <= codes, codes)

    def test_native_rejection_is_fail_closed(self):
        before = set(Path(tempfile.gettempdir()).glob("lo355-candidate-*"))
        result = self.analyze(mode="fail")
        self.assert_rejected(result, "native_failure")
        self.assertEqual(
            set(Path(tempfile.gettempdir()).glob("lo355-candidate-*")), before
        )

    def test_non_json_native_output_is_fail_closed(self):
        self.assert_rejected(self.analyze(mode="bad-json"), "native_failure")

    def test_snapshot_bounds_reject(self):
        observed = observation()
        observed["source"]["selection"]["width"] = 17
        self.assert_rejected(self.analyze(observed), "invalid_snapshot")
        observed = observation()
        observed["source"]["kind"] = "active-document"
        self.assert_rejected(self.analyze(observed), "invalid_snapshot")

    def test_mapping_structure_rejects(self):
        broken = mapping()
        broken["columns"][1]["type"] = "date"
        self.assert_rejected(self.analyze(mapping_value=broken), "invalid_mapping")
        broken = mapping()
        broken["rows"] = broken["rows"][:1]
        self.assert_rejected(self.analyze(mapping_value=broken), "mapping_mismatch")

    def test_cell_guards_reject(self):
        observed = observation()
        observed["rows"][1][1] = {
            "kind": "FORMULA",
            "formula": "=1+1",
            "value": 2,
            "merged": False,
        }
        self.assert_rejected(self.analyze(observed), "unsupported_cell")
        observed = observation()
        observed["rows"][1][1] = {"kind": "EMPTY", "merged": False}
        self.assert_rejected(self.analyze(observed), "type_mismatch")
        observed = observation()
        observed["rows"][1][1] = number(2.5, number_format=128)
        self.assert_rejected(self.analyze(observed), "unsupported_cell")


class SavedFileHostGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lo355-host-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.source = self.directory / "inventory.ods"
        self.source.write_bytes(b"original-source-bytes")
        self.request = self.directory / "request.json"
        self.output = self.directory / "report.json"

    def request_payload(self, *, output=None, **source_overrides):
        selection = {"sheet": "Sheet1", "row": 0, "column": 0, "height": 3, "width": 2}
        source = {
            "kind": "saved-file",
            "path": str(self.source),
            "expected_sha256": hashlib.sha256(self.source.read_bytes()).hexdigest(),
            "selection": selection,
        }
        source.update(source_overrides)
        target = str(self.output) if output is None else output
        return {"source": source, "mapping": mapping(), "output": target}

    def invoke(self, payload, env_extra=None):
        self.request.write_text(json.dumps(payload), encoding="utf-8")
        environment = dict(os.environ)
        environment.update({"TACHIKO_BIN": "", "REFERENCE_OFFICE_BIN": ""})
        environment.update(env_extra or {})
        return subprocess.run(
            [sys.executable, str(HERE / "bridge.py"), "--request", str(self.request)],
            capture_output=True,
            text=True,
            timeout=60,
            env=environment,
        )

    def assert_rejected(self, result, code):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        response = json.loads(result.stdout)
        self.assertEqual(response.get("status"), "rejected", response)
        self.assertEqual(response.get("code"), code, response)
        self.assertNotIn("candidate", response)
        self.assertNotIn("native", response)
        self.assertTrue(
            any(entry.get("code") == code for entry in response["ledger"]), response
        )
        return response

    def test_active_document_requires_a_saved_file(self):
        self.assert_rejected(
            self.invoke(self.request_payload(kind="active-document")),
            "saved_file_required",
        )

    def test_symlink_source_is_rejected_without_touching_target(self):
        link = self.directory / "link.ods"
        link.symlink_to(self.source)
        self.assert_rejected(
            self.invoke(self.request_payload(path=str(link))), "invalid_input"
        )
        self.assertTrue(link.is_symlink())
        self.assertEqual(self.source.read_bytes(), b"original-source-bytes")

    def test_missing_and_wrong_suffix_are_rejected(self):
        self.assert_rejected(
            self.invoke(self.request_payload(path=str(self.directory / "absent.ods"))),
            "invalid_input",
        )
        other = self.directory / "notes.txt"
        other.write_bytes(b"text")
        self.assert_rejected(
            self.invoke(self.request_payload(path=str(other))), "invalid_input"
        )

    def test_oversized_source_is_rejected_before_office(self):
        oversized = self.directory / "big.ods"
        oversized.write_bytes(b"x" * (bridge.MAX_SOURCE_BYTES + 1))
        payload = self.request_payload(
            path=str(oversized),
            expected_sha256=hashlib.sha256(oversized.read_bytes()).hexdigest(),
        )
        self.assert_rejected(self.invoke(payload), "invalid_input")

    def test_stale_and_malformed_digests_reject(self):
        self.assert_rejected(
            self.invoke(self.request_payload(expected_sha256="0" * 64)),
            "source_changed",
        )
        self.assert_rejected(
            self.invoke(self.request_payload(expected_sha256="XYZ")), "invalid_input"
        )

    def test_existing_or_symlinked_output_is_preserved(self):
        self.output.write_bytes(b"existing-output")
        self.assert_rejected(self.invoke(self.request_payload()), "invalid_output")
        self.assertEqual(self.output.read_bytes(), b"existing-output")
        self.output.unlink()
        alias = self.directory / "alias.json"
        alias.symlink_to(self.directory / "elsewhere.json")
        self.assert_rejected(
            self.invoke(self.request_payload(output=str(alias))), "invalid_output"
        )
        self.assertTrue(alias.is_symlink())

    def test_output_aliasing_source_or_request_rejects(self):
        self.assert_rejected(
            self.invoke(self.request_payload(output=str(self.source))), "invalid_output"
        )

    def test_missing_office_or_native_engine_rejects(self):
        payload = self.request_payload(output=str(self.directory / "a.json"))
        self.assert_rejected(
            self.invoke(payload, {"TACHIKO_BIN": "tachiko-stub"}), "invalid_input"
        )
        payload = self.request_payload(output=str(self.directory / "b.json"))
        self.assert_rejected(
            self.invoke(
                payload, {"TACHIKO_BIN": "", "REFERENCE_OFFICE_BIN": "soffice"}
            ),
            "native_failure",
        )


class RequestPublication(unittest.TestCase):
    """Covers stage/publish semantics without pyuno by patching the Office adapter."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lo355-publish-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.source = self.directory / "inventory.ods"
        self.source.write_bytes(b"original-source-bytes")
        self.output = self.directory / "report.json"
        self.request = self.directory / "request.json"
        self.stub = self.directory / "tachiko-stub"
        self.stub.write_text(
            STUB_SOURCE.format(python=sys.executable), encoding="utf-8"
        )
        self.stub.chmod(0o700)
        expected = hashlib.sha256(self.source.read_bytes()).hexdigest()
        payload = {
            "source": {
                "kind": "saved-file",
                "path": str(self.source),
                "expected_sha256": expected,
                "selection": {
                    "sheet": "Sheet1",
                    "row": 0,
                    "column": 0,
                    "height": 3,
                    "width": 2,
                },
            },
            "mapping": mapping(),
            "output": str(self.output),
        }
        self.request.write_text(json.dumps(payload), encoding="utf-8")

    def observe_with(self):
        def adapter(office_bin, data, suffix, selection, sha256):
            observed = observation()
            observed["source"]["sha256"] = sha256
            return observed

        return adapter

    def environment(self):
        return mock.patch.dict(
            os.environ,
            {"TACHIKO_BIN": str(self.stub), "REFERENCE_OFFICE_BIN": "soffice"},
        )

    def test_success_publishes_exact_response_and_keeps_source(self):
        with self.environment(), mock.patch.object(
            bridge, "_observe_saved_copy", side_effect=self.observe_with()
        ):
            result = bridge.run_request(self.request)
        self.assertEqual(result["status"], "analyzed", result)
        self.assertEqual(json.loads(self.output.read_text(encoding="utf-8")), result)
        self.assertEqual(self.source.read_bytes(), b"original-source-bytes")

    def test_concurrent_source_change_blocks_publication(self):
        def mutate(office_bin, data, suffix, selection, sha256):
            self.source.write_bytes(b"external-change")
            return self.observe_with()(office_bin, data, suffix, selection, sha256)

        with self.environment(), mock.patch.object(
            bridge, "_observe_saved_copy", side_effect=mutate
        ):
            with self.assertRaises(bridge.Rejection) as caught:
                bridge.run_request(self.request)
        self.assertEqual(caught.exception.code, "source_changed")
        self.assertFalse(self.output.exists())
        self.assertEqual(self.source.read_bytes(), b"external-change")

    def test_destination_race_preserves_external_winner(self):
        def race(office_bin, data, suffix, selection, sha256):
            self.output.write_bytes(b"external-output-winner")
            return self.observe_with()(office_bin, data, suffix, selection, sha256)

        with self.environment(), mock.patch.object(
            bridge, "_observe_saved_copy", side_effect=race
        ):
            with self.assertRaises(bridge.Rejection) as caught:
                bridge.run_request(self.request)
        self.assertEqual(caught.exception.code, "invalid_output")
        self.assertEqual(self.output.read_bytes(), b"external-output-winner")


if __name__ == "__main__":
    unittest.main(verbosity=2)
