"""Native evidence under existing Cargo CI, not public-package/Office acceptance."""
from __future__ import annotations

import json
import os
import shutil
import unittest

import acceptance as contract


class NativeQueryEvidence(contract.RuntimePreflight):
    def test_existing_inspection_supplies_typed_scalar_and_schema_facts(self):
        before = contract.tree_digest(self.project)
        inspected = json.loads(contract.run([
            self.cli, "analyze", "document", str(self.project),
        ]).stdout)
        self.assertEqual(inspected["source"]["document_id"], contract.FIXTURE["document"]["id"])
        self.assertEqual(
            {item["id"]: item["key"] for item in inspected["entities"]},
            {key: item["key"] for key, item in contract.FIXTURE["document"]["entities"].items()},
        )
        self.assertEqual(
            {item["id"]: {field["id"]: field for field in item["fields"]}
             for item in inspected["schemas"]},
            {key: item["fields"] for key, item in contract.FIXTURE["document"]["schemas"].items()},
        )
        for entity in contract.FIXTURE["document"]["entities"].values():
            schema = contract.FIXTURE["document"]["schemas"][entity["schema"]]
            for field, value in entity["fields"].items():
                address = f"{entity['key']}.{schema['fields'][field]['key']}"
                observed = json.loads(contract.run([
                    self.cli, "analyze", "field", str(self.project), address,
                ]).stdout)
                self.assertEqual(observed["field"], {"entity": entity["id"], "field": field})
                self.assertEqual(observed["stored_value"], value)
        self.assertEqual(contract.tree_digest(self.project), before)


if __name__ == "__main__":
    node = shutil.which("node")
    if node is None:
        raise RuntimeError("ENVIRONMENT UNVERIFIED: Node is missing")
    os.environ.setdefault("OPEN_SHEET_POC_COMMAND", json.dumps([
        node, str(contract.ROOT / "export.mjs"),
    ]))
    loader = unittest.defaultTestLoader
    suite = unittest.TestSuite()
    # The public npm package and Office are separate mandatory delivery gates.
    # Do not call this native subset a passing complete integration suite.
    for case in (NativeQueryEvidence, contract.ExportAcceptance):
        for test in loader.loadTestsFromTestCase(case):
            if not test.id().endswith(".test_public_upstream_writer_without_evaluator"):
                suite.addTest(test)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)
