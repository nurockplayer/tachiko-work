import { describe, expect, it } from "vitest";
import { fixture, SECRET } from "../operational/github-fixture.js";

describe("Steward raw-source fixture self-check (not production acceptance)", () => {
  it("requires a real credential injection and yields raw REST rows with pagination metadata", async () => {
    const remote = fixture(); remote.partial.add("issues");
    const response = await remote.fetch(`${remote.api}/issues?state=open`, { headers: { authorization: `Bearer ${SECRET}` } });
    expect(response.headers.get("link")).toContain('rel="next"');
    const rows = await response.json() as Record<string, unknown>[];
    expect(rows).toHaveLength(3);
    expect(rows[2]).toHaveProperty("pull_request");
    expect(remote.requests[0]?.headers.get("authorization")).toContain(SECRET);
    expect(remote.violations).toEqual([]);
    const noCredential = fixture();
    expect((await noCredential.fetch(`${remote.api}/issues?state=open`)).status).toBe(400);
    expect(noCredential.violations).toHaveLength(1);
  });
  it("requires the declared activity ordering and supports a second independent PR", async () => {
    const remote = fixture(); remote.addPull(444, [231]);
    const headers = { authorization: `Bearer ${SECRET}` };
    expect((await remote.fetch(`${remote.api}/pulls?state=closed`, { headers })).status).toBe(400);
    expect((await remote.fetch(`${remote.api}/pulls?state=closed&sort=updated&direction=desc`, { headers })).status).toBe(200);
    const response = await remote.fetch(`${remote.api}/issues/444/comments`, { headers });
    expect(await response.json()).toMatchObject([{ html_url: `${remote.web}/pull/444#issuecomment-800` }]);
  });
  it("refuses mutation queries and can distinguish known-empty linkage from a failed connection", async () => {
    const remote = fixture(); remote.data.linked = [];
    const send = (query: string) => remote.fetch("https://api.github.com/graphql", {
      method: "POST", headers: { authorization: `Bearer ${SECRET}`, "content-type": "application/json" },
      body: JSON.stringify({ query, variables: { owner: "acceptance-fixture", name: "dashboard", number: 322 } }),
    });
    const query = "query($owner: String!, $name: String!, $number: Int!) { repository(owner:$owner, name:$name) { pullRequest(number:$number) { closingIssuesReferences(first:100) { nodes { number repository { nameWithOwner } } pageInfo { hasNextPage endCursor } } } } }";
    expect(await (await send(query)).json()).toMatchObject({ data: { repository: { pullRequest: { closingIssuesReferences: { nodes: [], pageInfo: { hasNextPage: false } } } } } });
    remote.failures.add("linkage");
    expect(await (await send(query)).json()).toHaveProperty("errors");
    expect(remote.violations).toEqual([]);
    expect((await send(`mutation ${query}`)).status).toBe(400);
    expect(remote.violations).toHaveLength(1);
  });
});
