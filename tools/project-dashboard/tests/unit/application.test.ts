import { describe, expect, it } from "vitest";
import { RECENT_ACTIVITY_WINDOW, observeGitHub } from "../../src/server/application.js";
import { dashboardOptionsFromEnvironment } from "../../src/cli.js";
import { fixture } from "../operational/github-fixture.js";
import { projectObservation } from "../../src/server/observation.js";

describe("Dashboard GitHub adapter unit boundaries", () => {
  it("does not promote a missing native PR head SHA into a current fact", async () => {
    const remote = fixture();
    (remote.data.pulls[0]!.head as { sha?: string }).sha = undefined;

    const snapshot = await observeGitHub(remote.options());

    expect(snapshot.pullRequests.value?.[0]?.head).toEqual({ availability: "unavailable", value: null });
    expect(snapshot.pullRequests.value?.[0]?.base).toEqual({ availability: "complete", value: remote.data.pulls[0]!.base.sha });
    expect(remote.violations).toEqual([]);
  });

  it("rejects malformed repository configuration before it can reach the transport", async () => {
    const remote = fixture();
    const options = { ...remote.options(), repository: "not-a-repository" };

    await expect(observeGitHub(options)).rejects.toThrow("owner/name");
    expect(remote.requests).toEqual([]);
  });

  it("builds the production application configuration only from server environment", () => {
    expect(dashboardOptionsFromEnvironment({
      DASHBOARD_REPOSITORY: "owner/repository",
      DASHBOARD_GITHUB_TOKEN: "server-only-token",
      DASHBOARD_TRUSTED_STEWARD_LOGINS: "steward-one, steward-two",
      DASHBOARD_PORT: "4312",
    })).toEqual({
      repository: "owner/repository",
      token: "server-only-token",
      trustedStewardLogins: ["steward-one", "steward-two"],
      port: 4312,
    });
    expect(() => dashboardOptionsFromEnvironment({
      DASHBOARD_REPOSITORY: "owner/repository",
      DASHBOARD_GITHUB_TOKEN: "server-only-token",
      DASHBOARD_TRUSTED_STEWARD_LOGINS: "steward-one",
      DASHBOARD_PORT: "65536",
    })).toThrow("DASHBOARD_PORT");
  });

  it("fails a malformed dependency row closed and declares a bounded activity request", async () => {
    const remote = fixture();
    remote.data.dependencyByIssue.set(229, [{ number: 0, title: "invalid", state: "open", html_url: `${remote.web}/issues/0` }]);

    const snapshot = await observeGitHub(remote.options());

    expect(snapshot.issues.value?.find(issue => issue.number === 229)?.dependencies).toEqual({ availability: "unavailable", value: null });
    const activityRequest = remote.requests.find(request => new URL(request.url).searchParams.get("state") === "closed");
    expect(new URL(activityRequest?.url ?? "https://invalid.example").searchParams.get("per_page")).toBe(String(RECENT_ACTIVITY_WINDOW));
    expect(remote.violations).toEqual([]);
  });

  it("contains a structurally malformed dependency row to that family", async () => {
    const remote = fixture();
    remote.data.dependencyByIssue.set(229, [null] as unknown as typeof remote.data.dependencies);

    const snapshot = await observeGitHub(remote.options());

    expect(snapshot.issues.value?.find(issue => issue.number === 229)?.dependencies).toEqual({ availability: "unavailable", value: null });
    expect(snapshot.main.availability).toBe("complete");
    expect(snapshot.pullRequests.availability).toBe("complete");
    expect(snapshot.recentActivity.availability).toBe("complete");
    expect(remote.violations).toEqual([]);
  });

  it("does not follow activity pagination or promote a watch from incomplete comment discovery", async () => {
    const remote = fixture();
    remote.partial.add("activity");
    remote.partial.add("comments");

    const snapshot = await observeGitHub(remote.options());

    expect(snapshot.recentActivity.availability).toBe("complete");
    expect(snapshot.stewardWatches.value?.[0]?.availability).toBe("partial");
    expect(projectObservation({ latest: snapshot }).attention.items).toEqual([]);
    const activityRequests = remote.requests.filter(request => new URL(request.url).searchParams.get("state") === "closed");
    expect(activityRequests).toHaveLength(1);
    expect(remote.violations).toEqual([]);
  });
});
