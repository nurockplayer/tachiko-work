import { afterEach, describe, expect, it } from "vitest";
import { startDashboard, type RunningDashboard } from "../../src/server/application.js";
import { fixture } from "./github-fixture.js";

const running: RunningDashboard[] = [];
afterEach(async () => { await Promise.all(running.splice(0).map(app => app.close())); });

describe("#229 absent control-route matrix", () => {
  it.each(["merge", "dispatch", "run-agent"])("keeps /api/%s absent for read and mutating methods", async route => {
    const remote = fixture();
    const server = await startDashboard(remote.options());
    running.push(server);
    const before = remote.requests.length;

    for (const method of ["GET", "POST", "PUT", "PATCH", "DELETE"]) {
      const response = await fetch(`${server.origin}/api/${route}`, { method });
      expect(response.status).toBe(404);
    }

    expect(remote.requests).toHaveLength(before);
    expect(remote.violations).toEqual([]);
  });
});
