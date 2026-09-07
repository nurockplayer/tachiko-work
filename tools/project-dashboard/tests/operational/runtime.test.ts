import { request as httpRequest } from "node:http";
import { afterEach, describe, expect, it } from "vitest";
import { observeGitHub, startDashboard, type DashboardEnvelope, type RunningDashboard } from "../../src/server/application.js";
import { projectObservation } from "../../src/server/observation.js";
import { fixture, HEAD, MAIN, OTHER, SECRET, STEWARD, watchBody } from "./github-fixture.js";

const running: RunningDashboard[] = [];
afterEach(async () => { await Promise.all(running.splice(0).map(app => app.close())); });
async function app(remote = fixture()) {
  const server = await startDashboard(remote.options());
  running.push(server);
  expect(new URL(server.origin).hostname).toMatch(/^(127\.0\.0\.1|\[::1\])$/);
  return { server, remote };
}
async function read(server: RunningDashboard) {
  const response = await fetch(`${server.origin}/api/projection`);
  expect(response.status).toBe(200);
  return await response.json() as DashboardEnvelope;
}
const issueLane = (value: DashboardEnvelope, number = 229) => value.projection.deliveries.find(x => x.issueNumber === number);

// Adapter acceptance uses raw HTTP payloads; projectObservation only consumes its output,
// never generates an expected value. The independently tested projection core is unchanged.
describe("#229 raw GitHub observation", () => {
  it("reads configured repo/default branch, filters PR-as-Issue rows, and preserves native linkage/edges", async () => {
    const remote = fixture("another-owner/not-the-production-repository");
    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.main).toEqual({ availability: "complete", value: MAIN });
    expect(snapshot.issues.value?.map(x => x.number)).toEqual([229, 231]);
    expect(snapshot.issues.value?.[0]?.dependencies).toEqual({ availability: "complete", value: [900] });
    expect(snapshot.pullRequests).toEqual({ availability: "complete", value: [{
      number: 322, title: "Implement the operational Dashboard", state: "OPEN", draft: true,
      head: { availability: "complete", value: HEAD }, base: { availability: "complete", value: OTHER },
      linkedIssues: { availability: "complete", value: [229] },
    }] });
    expect(snapshot.recentActivity).toEqual({ availability: "complete", value: [{ number: 320, title: "Previously merged change" }] });
    expect(snapshot.stewardWatches.value).toContainEqual({ prNumber: 322, availability: "complete", verdict: "HOLD", humanAction: "required", head: HEAD, main: MAIN, sourceUrl: `${remote.web}/pull/322#issuecomment-700` });
    expect(remote.requests.length).toBeGreaterThan(0);
    expect(remote.violations).toEqual([]);
  });

  it.each([false, true])("observes distinct linkage and attention for each PR (reversed=%s)", async reverse => {
    const remote = fixture(); remote.addPull(444, [231]);
    if (reverse) remote.data.pulls.reverse();
    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.pullRequests.availability).toBe("complete");
    expect(snapshot.pullRequests.value?.map(pull => [pull.number, pull.linkedIssues])).toEqual(
      remote.data.pulls.map(pull => [pull.number, { availability: "complete", value: [pull.number === 322 ? 229 : 231] }]),
    );
    const other = snapshot.pullRequests.value?.find(pull => pull.number === 444);
    expect(other).toMatchObject({ number: 444, title: "Independent PR 444", draft: false,
      head: { availability: "complete", value: OTHER }, base: { availability: "complete", value: MAIN } });
    const projected = projectObservation({ latest: snapshot });
    expect(projected.deliveries.map(lane => `${lane.issueNumber}:${lane.pullRequestNumber}`).sort()).toEqual(["229:322", "231:444"]);
    expect(projected.attention.items).toHaveLength(3);
    expect(projected.attention.items).toEqual(expect.arrayContaining([
      { kind: "steward-hold", prNumber: 322, sourceUrl: `${remote.web}/pull/322#issuecomment-700` },
      { kind: "human-action-required", prNumber: 322, sourceUrl: `${remote.web}/pull/322#issuecomment-700` },
      { kind: "human-action-required", prNumber: 444, sourceUrl: `${remote.web}/pull/444#issuecomment-800` },
    ]));
    expect(remote.violations).toEqual([]);
  });

  it.each(["VERDICT", "HEAD", "MAIN", "HUMAN_ACTION"])("rejects duplicated or absent %s header fields", async field => {
    for (const duplicate of [true, false]) {
      const remote = fixture();
      const lines = watchBody().split("\n");
      const index = lines.findIndex(line => line.startsWith(`${field}:`));
      if (duplicate) lines.splice(index, 0, lines[index]!); else lines.splice(index, 1);
      remote.data.comments = [remote.comment(700, lines.join("\n"))];
      expect(projectObservation({ latest: await observeGitHub(remote.options()) }).attention.items).toEqual([]);
      expect(remote.violations).toEqual([]);
    }
  });

  it.each([
    ["unknown header", (body: string) => body.replace("VERDICT: HOLD", "VERDICT: HOLD\nEXTRA: trusted")],
    ["invalid verdict", (body: string) => body.replace("VERDICT: HOLD", "VERDICT: PASS")],
    ["invalid action", (body: string) => body.replace("HUMAN_ACTION: required", "HUMAN_ACTION: yes")],
    ["invalid head", (body: string) => body.replace(`HEAD: ${HEAD}`, "HEAD: short")],
    ["invalid main", (body: string) => body.replace(`MAIN: ${MAIN}`, "MAIN: short")],
  ] as const)("rejects a trusted %s", async (_name, change) => {
    const remote = fixture(); remote.data.comments = [remote.comment(700, change(watchBody()))];
    expect(projectObservation({ latest: await observeGitHub(remote.options()) }).attention.items).toEqual([]);
    expect(remote.violations).toEqual([]);
  });

  it("makes unavailable dependencies Unknown without erasing independently observed Issue core", async () => {
    const remote = fixture(); remote.failures.add("dependencies");
    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.issues.availability).toBe("complete");
    expect(snapshot.issues.value?.[0]).toMatchObject({ number: 229, title: "Dashboard operational acceptance", dependencies: { availability: "unavailable", value: null } });
    expect(projectObservation({ latest: snapshot }).criticalPath).toEqual({ availability: "unavailable", issueNumbers: [] });
    expect(remote.violations).toEqual([]);
  });

  it("does not promote incomplete Issue/PR discovery into current membership; current positive watch survives", async () => {
    const remote = fixture(); remote.partial.add("issues"); remote.partial.add("pulls");
    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.issues.availability).toBe("partial");
    expect(snapshot.pullRequests.availability).toBe("partial");
    const projection = projectObservation({ latest: snapshot });
    expect(projection.deliveries).toEqual([]);
    expect(projection.attention.items).toEqual([
      { kind: "steward-hold", prNumber: 322, sourceUrl: `${remote.web}/pull/322#issuecomment-700` },
      { kind: "human-action-required", prNumber: 322, sourceUrl: `${remote.web}/pull/322#issuecomment-700` },
    ]);
    expect(remote.violations).toEqual([]);
  });

  it.each(["empty", "partial", "unavailable", "foreign"] as const)("never infers an Issue link from %s native linkage", async kind => {
    const remote = fixture();
    if (kind === "empty") remote.data.linked = [];
    if (kind === "partial") remote.partial.add("linkage");
    if (kind === "unavailable") remote.failures.add("linkage");
    if (kind === "foreign") remote.data.linked[0]!.repository.nameWithOwner = "foreign/repository";
    const snapshot = await observeGitHub(remote.options());
    const projection = projectObservation({ latest: snapshot });
    expect(projection.deliveries.map(x => [x.issueNumber, x.pullRequestNumber])).toEqual([[null, 322], [229, null], [231, null]]);
    const link = projection.deliveries[0]!.linkedIssues;
    expect(link).toEqual(kind === "empty" ? { availability: "complete", value: [] } : { availability: kind === "partial" ? "partial" : "unavailable", value: null });
    expect(projection.attention.items).toHaveLength(2);
    expect(remote.violations).toEqual([]);
  });

  it.each(["untrusted", "duplicate-field", "missing-anchor", "quoted-marker", "handoff-only"] as const)("rejects %s structured attention despite current-looking prose", async kind => {
    const remote = fixture();
    let body = watchBody(); let login = STEWARD;
    if (kind === "untrusted") login = "untrusted-visitor";
    if (kind === "duplicate-field") body = body.replace("VERDICT: HOLD", "VERDICT: HOLD\nVERDICT: GREEN");
    if (kind === "missing-anchor") body = body.replace(`HEAD: ${HEAD}\n`, "");
    if (kind === "quoted-marker") body = `Someone said:\n${body}`;
    if (kind === "handoff-only") body = "<!-- agent-handoff:v1 -->\nHUMAN_ACTION: required\n\nHOLD, merge-ready, passed review.";
    remote.data.comments = [remote.comment(700, body, login)];
    const snapshot = await observeGitHub(remote.options());
    expect(projectObservation({ latest: snapshot }).attention.items).toEqual([]);
    expect(remote.violations).toEqual([]);
  });

  it("accepts an edited trusted canonical watch, PR-scoped and multi-Issue, not the newest untrusted comment", async () => {
    const remote = fixture(); remote.data.linked.push({ number: 231, repository: { nameWithOwner: remote.repository } });
    remote.data.comments.push(remote.comment(701, watchBody("GREEN", "none"), "untrusted-visitor"));
    const snapshot = await observeGitHub(remote.options());
    const projection = projectObservation({ latest: snapshot });
    expect(projection.deliveries.map(x => x.issueNumber)).toEqual([229, 231]);
    expect(projection.attention.items).toHaveLength(2);
    expect(projection.attention.items.every(x => x.sourceUrl.endsWith("700"))).toBe(true);
    expect(remote.violations).toEqual([]);
  });

  it("fails closed when more than one trusted canonical Steward watch exists for the same PR", async () => {
    const remote = fixture();
    remote.data.comments.push(remote.comment(701, watchBody("GREEN", "none")));
    const snapshot = await observeGitHub(remote.options());
    expect(snapshot.stewardWatches.value?.filter(watch => watch.prNumber === 322)).toEqual([]);
    expect(projectObservation({ latest: snapshot }).attention.items).toEqual([]);
    expect(remote.violations).toEqual([]);
  });
});

describe("#229 actual read-only HTTP/application boundary", () => {
  it("serves a credential-free projection with source provenance and a real HTML entry", async () => {
    const { server, remote } = await app();
    const response = await fetch(`${server.origin}/api/projection`);
    const text = await response.text();
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toContain("no-store");
    expect(response.headers.get("access-control-allow-origin")).toBeNull();
    expect(text).not.toContain(SECRET);
    const value = JSON.parse(text) as DashboardEnvelope;
    expect(value.repository).toBe(remote.repository);
    expect(value.projection.executive.mainSha).toEqual({ availability: "complete", value: MAIN });
    expect(issueLane(value)?.issueTitle).toEqual({ availability: "complete", value: "Dashboard operational acceptance" });
    expect(value.sources.issues).toEqual({ availability: "complete", url: `${remote.web}/issues` });
    expect(value.sources.pullRequests).toEqual({ availability: "complete", url: `${remote.web}/pulls` });
    expect(value.sources.main.url).toBe(`${remote.web}/commit/${MAIN}`);
    expect(value.sources.recentActivity.url.startsWith(`${remote.web}/pulls`)).toBe(true);
    const html = await fetch(server.origin);
    expect(html.status).toBe(200);
    expect(html.headers.get("content-type")).toContain("text/html");
    expect(await html.text()).not.toContain(SECRET);
    expect(remote.requests.length).toBeGreaterThan(0);
    expect(remote.violations).toEqual([]);
  });

  it("success -> failed upstream MAIN -> recovery clears only dependent facts and never leaks error credentials", async () => {
    const { server, remote } = await app();
    expect((await read(server)).projection.attention.items).toHaveLength(2);
    remote.failures.add("main"); remote.setErrorText(`Forbidden: ${SECRET}`);
    const failed = await read(server);
    expect(failed.projection.executive.mainSha).toEqual({ availability: "unavailable", value: null });
    expect(failed.projection.attention.items).toEqual([]);
    expect(issueLane(failed)?.issueTitle.availability).toBe("complete");
    expect(failed.sources.main.availability).toBe("unavailable");
    expect(JSON.stringify(failed)).not.toContain(SECRET);
    remote.failures.clear(); remote.data.main = "4".repeat(40);
    remote.data.comments = [remote.comment(702, watchBody("AMBER", "required", HEAD, remote.data.main))];
    const restored = await read(server);
    expect(restored.projection.executive.mainSha.value).toBe(remote.data.main);
    expect(restored.projection.attention.items).toEqual([{ kind: "human-action-required", prNumber: 322, sourceUrl: `${remote.web}/pull/322#issuecomment-702` }]);
    expect(remote.violations).toEqual([]);
  });

  it("denies mutation/control routes, cross-origin reads, and hostile Host headers", async () => {
    const { server } = await app();
    for (const method of ["POST", "PUT", "PATCH", "DELETE"]) {
      expect((await fetch(`${server.origin}/api/projection`, { method })).status).toBe(405);
    }
    for (const route of ["merge", "dispatch", "run-agent"]) {
      expect((await fetch(`${server.origin}/api/${route}`)).status).toBe(404);
    }
    const cross = await fetch(`${server.origin}/api/projection`, { headers: { Origin: "https://evil.example", "Sec-Fetch-Site": "cross-site" } });
    expect(cross.status).toBe(403);
    expect(cross.headers.get("access-control-allow-origin")).toBeNull();
    const hostile = await new Promise<number>( (resolve, reject) => {
      const req = httpRequest(new URL("/api/projection", server.origin), { headers: { Host: "evil.example" } }, res => { res.resume(); resolve(res.statusCode ?? 0); });
      req.on("error", reject); req.end();
    });
    expect(hostile).toBe(403);
  });

  it("does not expose credentials/source files through direct or encoded static paths", async () => {
    const { server } = await app();
    for (const path of ["/.env", "/package.json", "/src/server/application.ts", "/%2e%2e%2f.env", "/%2e%2e%5c.env"]) {
      const response = await fetch(`${server.origin}${path}`);
      expect([400, 403, 404]).toContain(response.status);
      expect(await response.text()).not.toContain(SECRET);
    }
  });
});
