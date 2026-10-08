"""Independent typed acceptance-receipt checks; no producer/runtime imports."""


class MeasurementIncomplete(ValueError):
    pass


def check_rss_samples(samples):
    def require(condition):
        if not condition:
            raise MeasurementIncomplete("BLOCKED_MEASUREMENT: incomplete/invalid RSS evidence; performance UNVERIFIED")

    def integer(value):
        return type(value) is int and 0 <= value <= 9007199254740991

    require(type(samples) is list and len(samples) >= 2)
    previous = -1
    totals = []
    for sample in samples:
        require(type(sample) is dict)
        require(integer(sample.get("at")) and integer(sample.get("completedAt")))
        require(previous <= sample["at"] <= sample["completedAt"])
        previous = sample["completedAt"]
        require(sample.get("complete") is True)
        enumeration = sample.get("enumeration")
        require(type(enumeration) is dict and enumeration.get("status") == "complete")
        expected = enumeration.get("processes")
        observed = sample.get("processes")
        require(type(expected) is list and len(expected) > 0 and type(observed) is list)
        inventory = {}
        for process in expected:
            require(type(process) is dict and integer(process.get("pid")) and process["pid"] > 0)
            require(type(process.get("type")) is str and len(process["type"]) > 0)
            require(process["pid"] not in inventory)
            inventory[process["pid"]] = process["type"]
        seen = set()
        total = 0
        measured = 0
        for process in observed:
            require(type(process) is dict and integer(process.get("pid")))
            pid = process["pid"]
            require(pid in inventory and pid not in seen and process.get("type") == inventory[pid])
            seen.add(pid)
            if process.get("state") == "measured":
                require(integer(process.get("rssBytes")) and process["rssBytes"] > 0)
                require("exitProof" not in process and "reason" not in process and "liveness" not in process)
                total += process["rssBytes"]
                measured += 1
            elif process.get("state") == "exited":
                proof = process.get("exitProof")
                require(type(proof) is dict and proof.get("method") == "kill-0" and proof.get("errno") == "ESRCH")
                require(integer(proof.get("at")) and sample["at"] <= proof["at"] <= sample["completedAt"])
                require("rssBytes" not in process and "reason" not in process and "liveness" not in process)
            else:
                require(False)
        require(seen == set(inventory) and measured > 0 and integer(total) and total > 0)
        require(integer(sample.get("rss")) and sample["rss"] == total)
        totals.append(total)
    return max(totals) - totals[0]


def check_rss_budget(samples):
    increase = check_rss_samples(samples)
    # Only complete measurement reaches the unchanged product budget assertion.
    assert increase <= 512 * 1024 * 1024, "process memory increase budget"
    return increase
