import { afterEach, describe, expect, it } from "vitest";
import {
  observeGitHub,
  startDashboard,
  type DashboardEnvelope,
  type RunningDashboard,
} from "../../src/server/application.js";
import { projectObservation } from "../../src/server/observation.js";
import { fixture } from "./github-fixture.js";

const running: RunningDashboard[] = [];
afterEach(async () => { await Promise.all(running.splice(0).map(app => app.close())); });

async function app(remote = fixture()) {
  const server = await startDashboard(remote.options());
  running.push(server);
  return { server, remote };
}

async function read(server: RunningDashboard): Promise<DashboardEnvelope> {
  const response = await fetch(`${server.origin}/api/projection`);
  expect(response.status).toBe(200);
  return await response.json() as DashboardEnvelope;
}

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

  it("success -> hard Issue-discovery failure clears stale Issue membership through the HTTP application", async () => {
    const { server, remote } = await app();
    const initial = await read(server);
    expect(initial.projection.deliveries.some(lane => lane.issueNumber === 231)).toBe(true);
    expect(initial.sources.issues.availability).toBe("complete");

    remote.failures.add("issues");
    const failed = await read(server);
    expect(failed.sources.issues.availability).toBe("unavailable");
    expect(failed.projection.criticalPath).toEqual({ availability: "unavailable", issueNumbers: [] });
    expect(failed.projection.deliveries.some(lane => lane.issueNumber === 231)).toBe(false);
    const prLane = failed.projection.deliveries.find(lane => lane.pullRequestNumber === 322);
    expect(prLane?.pullRequestTitle).toEqual({ availability: "complete", value: "Implement the operational Dashboard" });
    expect(prLane?.issueTitle).toEqual({ availability: "unavailable", value: null });
    expect(failed.projection.recentActivity.availability).toBe("complete");
    expect(remote.violations).toEqual([]);
  });

  it("success -> hard PR-discovery failure clears stale PR membership and attention through the HTTP application", async () => {
    const { server, remote } = await app();
    const initial = await read(server);
    expect(initial.projection.deliveries.some(lane => lane.pullRequestNumber === 322)).toBe(true);
    expect(initial.projection.attention.items).toHaveLength(2);
    expect(initial.sources.pullRequests.availability).toBe("complete");

    remote.failures.add("pulls");
    const failed = await read(server);
    expect(failed.sources.pullRequests.availability).toBe("unavailable");
    expect(failed.projection.attention.items).toEqual([]);
    expect(failed.projection.deliveries.every(lane => lane.pullRequestNumber === null)).toBe(true);
    const issue229 = failed.projection.deliveries.find(lane => lane.issueNumber === 229);
    const issue231 = failed.projection.deliveries.find(lane => lane.issueNumber === 231);
    expect(issue229?.issueTitle).toEqual({ availability: "complete", value: "Dashboard operational acceptance" });
    expect(issue231?.issueTitle).toEqual({ availability: "complete", value: "Independent issue" });
    expect(issue229?.pullRequestTitle).toEqual({ availability: "unavailable", value: null });
    expect(issue231?.pullRequestTitle).toEqual({ availability: "unavailable", value: null });
    expect(failed.projection.recentActivity.availability).toBe("complete");
    expect(remote.violations).toEqual([]);
  });
});
