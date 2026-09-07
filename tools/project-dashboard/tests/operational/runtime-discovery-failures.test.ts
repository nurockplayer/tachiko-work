import { describe, expect, it } from "vitest";
import { observeGitHub } from "../../src/server/application.js";
import { projectObservation } from "../../src/server/observation.js";
import { fixture } from "./github-fixture.js";

describe("#229 hard discovery failures", () => {
  it("classifies an initial Issue-discovery 503 as unavailable while PR/main/activity facts stay current", async () => {
    const remote = fixture();
    remote.failures.add("issues");

    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.issues).toEqual({ availability: "unavailable", value: null });
    expect(snapshot.main.availability).toBe("complete");
    expect(snapshot.pullRequests.availability).toBe("complete");
    expect(snapshot.recentActivity.availability).toBe("complete");

    const projection = projectObservation({ latest: snapshot });
    const prLane = projection.deliveries.find(lane => lane.pullRequestNumber === 322);
    expect(prLane?.pullRequestTitle).toEqual({ availability: "complete", value: "Implement the operational Dashboard" });
    expect(prLane?.issueTitle).toEqual({ availability: "unavailable", value: null });
    expect(projection.criticalPath).toEqual({ availability: "unavailable", issueNumbers: [] });
    expect(projection.attention.items).toHaveLength(2);
    expect(remote.violations).toEqual([]);
  });

  it("classifies an initial PR-discovery 503 as unavailable while Issue/main/activity facts stay current", async () => {
    const remote = fixture();
    remote.failures.add("pulls");

    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.pullRequests).toEqual({ availability: "unavailable", value: null });
    expect(snapshot.main.availability).toBe("complete");
    expect(snapshot.issues.availability).toBe("complete");
    expect(snapshot.recentActivity.availability).toBe("complete");

    const projection = projectObservation({ latest: snapshot });
    const issueLane = projection.deliveries.find(lane => lane.issueNumber === 229);
    expect(issueLane?.issueTitle).toEqual({ availability: "complete", value: "Dashboard operational acceptance" });
    expect(issueLane?.pullRequestTitle).toEqual({ availability: "unavailable", value: null });
    expect(projection.attention.items).toEqual([]);
    expect(projection.recentActivity.availability).toBe("complete");
    expect(remote.violations).toEqual([]);
  });
});
