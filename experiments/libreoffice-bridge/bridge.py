"""Deliberate missing-seam seed for #355. Not a functioning LibreOffice bridge."""
from __future__ import annotations

import json


def analyze_snapshot(observation: dict, mapping: dict, *, tachiko_bin: str) -> dict:
    """Experimental integration boundary; implementation belongs to the delivery agent."""
    return {"status": "rejected", "code": "BRIDGE_NOT_IMPLEMENTED", "ledger": []}


if __name__ == "__main__":
    print(json.dumps({"status": "rejected", "code": "BRIDGE_NOT_IMPLEMENTED", "ledger": []}))
    raise SystemExit(2)
