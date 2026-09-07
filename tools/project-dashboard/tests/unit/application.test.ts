import { describe, expect, it } from "vitest";
import { observeGitHub } from "../../src/server/application.js";
import { fixture } from "../operational/github-fixture.js";

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
});
