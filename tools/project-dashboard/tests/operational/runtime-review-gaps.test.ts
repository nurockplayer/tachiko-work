import { afterEach, describe, expect, it } from "vitest";
import {
  observeGitHub,
  startDashboard,
  type DashboardEnvelope,
  type RunningDashboard,
} from "../../src/server/application.js";
import { projectObservation } from "../../src/server/observation.js";
import { fixture, SECRET } from "./github-fixture.js";

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

describe("#229 operational refresh fail-closed gaps", () => {
  it("comments failure clears previously observed attention while independent facts remain current", async () => {
    const remote = fixture();
    const initial = await observeGitHub(remote.options());
    expect(initial.stewardWatches.availability).toBe("complete");
    const previous = projectObservation({ latest: initial });
    expect(previous.attention.items).toHaveLength(2);

    remote.failures.add("comments");
    const failed = await observeGitHub(remote.options());
    expect(failed.stewardWatches).toEqual({ availability: "unavailable", value: null });
    const projection = projectObservation({ latest: failed, previous });
    expect(projection.attention.items).toEqual([]);
    expect(projection.executive.mainSha.availability).toBe("complete");
    expect(projection.deliveries.find(lane => lane.issueNumber === 229)?.issueTitle.availability).toBe("complete");
    expect(remote.violations).toEqual([]);
  });

  it("recent-activity failure after success clears old activity and marks its source unavailable", async () => {
    const { server, remote } = await app();
    const initial = await read(server);
    expect(initial.projection.recentActivity).toEqual({
      availability: "complete",
      value: [{ number: 320, title: "Previously merged change" }],
    });
    expect(initial.sources.recentActivity.availability).toBe("complete");

    remote.failures.add("activity");
    const failed = await read(server);
    expect(failed.projection.recentActivity).toEqual({ availability: "unavailable", value: null });
    expect(failed.sources.recentActivity.availability).toBe("unavailable");
    expect(failed.projection.executive.mainSha.availability).toBe("complete");
    expect(failed.projection.deliveries.find(lane => lane.issueNumber === 229)?.issueTitle.availability).toBe("complete");
    expect(remote.violations).toEqual([]);
  });
});

describe("#229 server-only configuration boundary", () => {
  it("rejects browser query attempts to override repository, credential, or Steward trust", async () => {
    const { server, remote } = await app();
    const before = remote.requests.length;
    const response = await fetch(
      `${server.origin}/api/projection?repository=evil/repository&token=attacker&trustedStewardLogins=untrusted-visitor`,
    );
    expect(response.status).toBe(400);
    expect(await response.text()).not.toContain(SECRET);
    expect(remote.requests).toHaveLength(before);
    expect(remote.violations).toEqual([]);
  });
});
