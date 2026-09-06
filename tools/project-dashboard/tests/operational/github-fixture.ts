/** Raw GitHub transport fixture. No production projector/normalizer is imported. */
export const MAIN = "1".repeat(40);
export const HEAD = "2".repeat(40);
export const OTHER = "3".repeat(40);
export const SECRET = "github_pat_operational_acceptance_canary_DO_NOT_EXPOSE";
export const STEWARD = "fixture-steward";
export const DEFAULT_REPO = "acceptance-fixture/dashboard";
export type Fault = "main" | "issues" | "pulls" | "dependencies" | "linkage" | "comments" | "activity";

export function connection<T>(nodes: T[], next = false) {
  return { nodes, pageInfo: { hasNextPage: next, endCursor: next ? "more" : null } };
}

export function watchBody(verdict = "HOLD", action = "required", head = HEAD, main = MAIN) {
  return `<!-- project-steward-watch:v1 -->\nVERDICT: ${verdict}\nHEAD: ${head}\nMAIN: ${main}\nHUMAN_ACTION: ${action}\n\nNarrative: GREEN, all tests pass, no action needed. Not machine authority.`;
}

export function fixture(repository = DEFAULT_REPO) {
  const [owner, name] = repository.split("/");
  const web = `https://github.com/${repository}`;
  const api = `https://api.github.com/repos/${repository}`;
  const issue = (number: number, title: string) => ({
    number, title, state: "open", html_url: `${web}/issues/${number}`,
  });
  const comment = (id: number, body: string, login = STEWARD) => ({
    id, node_id: `IC_${id}`, body, user: { login }, author_association: "OWNER",
    html_url: `${web}/pull/322#issuecomment-${id}`,
    created_at: "2026-09-01T00:00:00Z", updated_at: "2026-09-06T00:00:00Z",
  });
  const data = {
    main: MAIN,
    issues: [issue(229, "Dashboard operational acceptance"), issue(231, "Independent issue")],
    pulls: [{
      number: 322, title: "Implement the operational Dashboard", state: "open", draft: true,
      html_url: `${web}/pull/322`, head: { sha: HEAD }, base: { sha: OTHER },
    }],
    dependencies: [issue(900, "Unobserved dependency target")],
    linked: [{ number: 229, repository: { nameWithOwner: repository } }],
    comments: [comment(700, watchBody())],
    activity: [{ number: 320, title: "Previously merged change", state: "closed", merged_at: "2026-09-01T00:00:00Z", html_url: `${web}/pull/320` },
      { number: 319, title: "Closed but never merged", state: "closed", merged_at: null, html_url: `${web}/pull/319` }],
  };
  const failures = new Set<Fault>();
  const partial = new Set<Fault>();
  const requests: Request[] = [];
  const violations: string[] = [];
  let errorText = "upstream unavailable";
  const json = (value: unknown, status = 200, headers: Record<string, string> = {}) =>
    new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json", ...headers } });
  const fetcher: typeof globalThis.fetch = async (input, init) => {
    const request = new Request(input, init);
    requests.push(request.clone());
    const url = new URL(request.url);
    const fail = (reason: string) => { violations.push(reason); return json({ message: reason }, 400); };
    if (url.origin !== "https://api.github.com") return fail(`unexpected origin: ${url.origin}`);
    if (request.headers.get("authorization") !== `Bearer ${SECRET}` && request.headers.get("authorization") !== `token ${SECRET}`)
      return fail("fake credential did not reach the upstream boundary");

    if (url.pathname === "/graphql") {
      if (request.method !== "POST") return fail("GraphQL requires POST of a query");
      const body = await request.json() as { query?: string; variables?: Record<string, unknown> };
      const query = body.query ?? "";
      // Finite linkage-only query profile. Verify the selection, not an operation name.
      if (/\bmutation\b/.test(query) || !/\bclosingIssuesReferences\b/.test(query) ||
          !/\bpageInfo\b/.test(query) || !/\bhasNextPage\b/.test(query) ||
          !/\bnumber\b/.test(query) || !/\bnameWithOwner\b/.test(query))
        return fail("linkage query omits required identity/completeness fields or writes");
      const variables = body.variables ?? {};
      if (variables.owner !== owner || variables.name !== name || variables.number !== 322)
        return fail("linkage query is not scoped to the configured repository and PR");
      if (failures.has("linkage")) return json({ data: { repository: { pullRequest: { closingIssuesReferences: null } } }, errors: [{ message: errorText, path: ["repository", "pullRequest", "closingIssuesReferences"] }] });
      if (partial.has("linkage") && variables.after) return json({ errors: [{ message: errorText }] }, 503);
      return json({ data: { repository: { pullRequest: { closingIssuesReferences: connection(data.linked, partial.has("linkage")) } } } });
    }
    if (request.method !== "GET") return fail(`non-read REST request: ${request.method}`);
    const pathname = url.pathname;
    const prefix = `/repos/${repository}`;
    let family: Fault | undefined;
    let payload: unknown;
    if (pathname === prefix) {
      family = "main"; payload = { full_name: repository, default_branch: "trunk", html_url: web };
    } else if (pathname === `${prefix}/git/ref/heads/trunk`) {
      family = "main"; payload = { ref: "refs/heads/trunk", object: { type: "commit", sha: data.main } };
    } else if (pathname === `${prefix}/issues`) {
      if (url.searchParams.get("state") !== "open") return fail("Issue discovery must request the open set");
      family = "issues";
      // REST /issues also returns PR nodes. This one must not become an Issue lane.
      payload = [...data.issues, { ...issue(322, "Not an Issue"), pull_request: { url: `${api}/pulls/322` } }];
    } else if (pathname === `${prefix}/pulls`) {
      if (url.searchParams.get("state") === "closed") { family = "activity"; payload = data.activity; }
      else if (url.searchParams.get("state") === "open") { family = "pulls"; payload = data.pulls; }
      else return fail("PR discovery must distinguish current open PRs from bounded activity");
    } else if (/\/issues\/(229|231)\/dependencies\/blocked_by$/.test(pathname) && pathname.startsWith(`${prefix}/`)) {
      family = "dependencies"; payload = pathname.includes("/229/") ? data.dependencies : [];
    } else if (pathname === `${prefix}/issues/322/comments`) {
      family = "comments"; payload = data.comments;
    } else return fail(`unexpected REST path: ${pathname}`);
    if (failures.has(family)) return json({ message: errorText }, 503);
    if (partial.has(family) && url.searchParams.get("page") === "2") return json({ message: errorText }, 503);
    const headers: Record<string, string> = {};
    if (partial.has(family)) {
      const next = new URL(url); next.searchParams.set("page", "2");
      headers.link = `<${next}>; rel="next"`;
    }
    return json(payload, 200, headers);
  };
  return { repository, web, api, data, failures, partial, requests, violations, comment,
    fetch: fetcher, setErrorText: (text: string) => { errorText = text; },
    options: () => ({ repository, token: SECRET, trustedStewardLogins: [STEWARD], fetch: fetcher, port: 0 }),
  };
}
