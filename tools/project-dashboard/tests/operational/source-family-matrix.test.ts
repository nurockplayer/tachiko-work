import { describe, expect, it } from "vitest";
import { observeGitHub } from "../../src/server/application.js";
import { projectObservation } from "../../src/server/observation.js";
import { fixture, HEAD, MAIN, watchBody } from "./github-fixture.js";

const STALE = "9".repeat(40);

describe("#229 finite raw-source family audit", () => {
  it.each(["trunk", "main"])("derives the live ref from repository.default_branch=%s", async defaultBranch => {
    const remote = fixture();
    remote.data.defaultBranch = defaultBranch;

    const snapshot = await observeGitHub(remote.options());

    expect(snapshot.main).toEqual({ availability: "complete", value: MAIN });
    const paths = remote.requests.map(request => new URL(request.url).pathname);
    expect(paths).toContain(`/repos/${remote.repository}`);
    expect(paths).toContain(`/repos/${remote.repository}/git/ref/heads/${defaultBranch}`);
    expect(remote.violations).toEqual([]);
  });

  it.each([false, true])("enriches every observed Issue independently (reversed=%s)", async reversed => {
    const remote = fixture();
    if (reversed) remote.data.issues.reverse();

    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.issues.availability).toBe("complete");
    const issue229 = snapshot.issues.value?.find(issue => issue.number === 229);
    const issue231 = snapshot.issues.value?.find(issue => issue.number === 231);
    expect(issue229?.dependencies).toEqual({ availability: "complete", value: [900] });
    expect(issue231?.dependencies).toEqual({ availability: "complete", value: [901] });

    const dependencyPaths = remote.requests
      .map(request => new URL(request.url).pathname)
      .filter(path => path.endsWith("/dependencies/blocked_by"));
    expect(dependencyPaths).toEqual(expect.arrayContaining([
      `/repos/${remote.repository}/issues/229/dependencies/blocked_by`,
      `/repos/${remote.repository}/issues/231/dependencies/blocked_by`,
    ]));
    expect(remote.violations).toEqual([]);
  });

  it.each([
    ["HEAD", STALE, MAIN],
    ["MAIN", HEAD, STALE],
  ] as const)("preserves a valid stale %s anchor and keeps its attention non-current", async (_kind, watchHead, watchMain) => {
    const remote = fixture();
    remote.data.comments = [remote.comment(700, watchBody("HOLD", "required", watchHead, watchMain))];

    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.stewardWatches.value).toContainEqual({
      prNumber: 322,
      availability: "complete",
      verdict: "HOLD",
      humanAction: "required",
      head: watchHead,
      main: watchMain,
      sourceUrl: `${remote.web}/pull/322#issuecomment-700`,
    });
    expect(projectObservation({ latest: snapshot }).attention.items).toEqual([]);
    expect(remote.violations).toEqual([]);
  });

  it("keeps complete PR facts current when only Issue discovery is truncated", async () => {
    const remote = fixture();
    remote.partial.add("issues");

    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.issues.availability).toBe("partial");
    expect(snapshot.pullRequests.availability).toBe("complete");

    const projection = projectObservation({ latest: snapshot });
    const lane = projection.deliveries.find(candidate => candidate.pullRequestNumber === 322);
    expect(lane?.issueNumber).toBe(229);
    expect(lane?.issueTitle).toEqual({ availability: "partial", value: null });
    expect(lane?.pullRequestTitle).toEqual({ availability: "complete", value: "Implement the operational Dashboard" });
    expect(lane?.pullRequestHead).toEqual({ availability: "complete", value: HEAD });
    expect(remote.violations).toEqual([]);
  });

  it("keeps complete Issue facts current when only PR discovery is truncated", async () => {
    const remote = fixture();
    remote.partial.add("pulls");

    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.issues.availability).toBe("complete");
    expect(snapshot.pullRequests.availability).toBe("partial");

    const projection = projectObservation({ latest: snapshot });
    const issue229 = projection.deliveries.find(candidate => candidate.issueNumber === 229);
    const issue231 = projection.deliveries.find(candidate => candidate.issueNumber === 231);
    expect(issue229?.pullRequestNumber).toBeNull();
    expect(issue231?.pullRequestNumber).toBeNull();
    expect(issue229?.issueTitle).toEqual({ availability: "complete", value: "Dashboard operational acceptance" });
    expect(issue231?.issueTitle).toEqual({ availability: "complete", value: "Independent issue" });
    expect(issue229?.pullRequestTitle).toEqual({ availability: "partial", value: null });
    expect(issue231?.pullRequestTitle).toEqual({ availability: "partial", value: null });
    expect(remote.violations).toEqual([]);
  });
});
