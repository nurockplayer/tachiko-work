import type {
  Availability,
  DashboardProjection,
  IssueObservation,
  ObservationSnapshot,
  Observed,
  PullObservation,
  RecentActivityObservation,
  StewardWatchObservation,
} from "./observation.js";
import { projectObservation } from "./observation.js";

/** Repository-local integration seam, not a public product API. */
export interface DashboardOptions {
  repository: string;
  token: string;
  trustedStewardLogins: readonly string[];
  /** Server-only injection of GitHub HTTP transport; never browser configuration. */
  fetch?: typeof globalThis.fetch;
  /** Test-only widening of the conservative external request budget. */
  maxExternalRequests?: number;
  /** Bind only to loopback. Zero selects an ephemeral port for acceptance. */
  port?: number;
}

export interface DashboardEnvelope {
  repository: string;
  projection: DashboardProjection;
  /** Fixed source families, not a generic fact registry or freshness DSL. */
  sources: {
    main: { availability: Availability; url: string };
    issues: { availability: Availability; url: string };
    pullRequests: { availability: Availability; url: string };
    recentActivity: { availability: Availability; url: string };
  };
}

interface Page<T> {
  availability: Availability;
  value: T[] | null;
}

interface GitHubRow {
  number?: unknown;
  title?: unknown;
  state?: unknown;
  draft?: unknown;
  pull_request?: unknown;
  head?: { sha?: unknown };
  base?: { sha?: unknown };
  merged_at?: unknown;
}

const API = "https://api.github.com";
const SHA = /^[0-9a-f]{40}$/i;
export const RECENT_ACTIVITY_WINDOW = 50;
// Keep one refresh within the Workers Free external-subrequest budget; paid
// deployments may raise the platform limit, but this adapter remains fail-closed.
const MAX_EXTERNAL_REQUESTS_PER_SNAPSHOT = 50;
const MAX_PAGES_PER_CONNECTION = 100;
const MAX_ENTITY_CONCURRENCY = 6;
const unavailable = <T>(): Observed<T> => ({ availability: "unavailable", value: null });
const observed = <T>(availability: Availability, value: T | null): Observed<T> =>
  value === null || availability === "unavailable"
    ? unavailable<T>()
    : { availability, value };
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;
const text = (value: unknown): string | null =>
  typeof value === "string" ? value : null;
const positiveNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isSafeInteger(value) && value > 0
    ? value
    : null;
const issueState = (value: unknown): "OPEN" | "CLOSED" | null => {
  const result = text(value)?.toUpperCase();
  return result === "OPEN" || result === "CLOSED" ? result : null;
};
const nextPage = (response: Response): string | null =>
  /<([^>]+)>;\s*rel="next"/.exec(response.headers.get("link") ?? "")?.[1] ??
  null;

async function mapBounded<T, R>(
  values: readonly T[],
  limit: number,
  mapper: (value: T) => Promise<R>,
): Promise<R[]> {
  const results = Array<R>(values.length);
  let next = 0;
  async function worker(): Promise<void> {
    while (true) {
      const index = next++;
      if (index >= values.length) return;
      results[index] = await mapper(values[index]!);
    }
  }
  await Promise.all(
    Array.from({ length: Math.min(limit, values.length) }, () => worker()),
  );
  return results;
}

/** Validate the repository identifier before it can be used to construct an API URL. */
export function assertDashboardRepository(repository: string): void {
  const parts = repository.split("/");
  if (parts.length !== 2 || !parts[0] || !parts[1]) {
    throw new Error("Dashboard repository must be owner/name");
  }
}

function repositoryParts(repository: string): [string, string] {
  assertDashboardRepository(repository);
  const parts = repository.split("/");
  return [parts[0]!, parts[1]!];
}

class GitHubObserver {
  private readonly fetcher: typeof globalThis.fetch;
  private readonly repositoryUrl: string;
  private readonly repositoryPath: string;
  readonly web: string;
  private readonly maxExternalRequests: number;
  private requestsUsed = 0;

  constructor(private readonly options: DashboardOptions) {
    const [owner, name] = repositoryParts(options.repository);
    this.fetcher = options.fetch ?? globalThis.fetch;
    this.repositoryPath = `/repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`;
    this.repositoryUrl = `${API}${this.repositoryPath}`;
    this.web = `https://github.com/${options.repository}`;
    this.maxExternalRequests =
      typeof options.maxExternalRequests === "number" &&
      Number.isSafeInteger(options.maxExternalRequests) &&
      options.maxExternalRequests >= 0
        ? options.maxExternalRequests
        : MAX_EXTERNAL_REQUESTS_PER_SNAPSHOT;
  }

  /**
   * Keep the token on the configured GitHub API origin and repository routes.
   * This also covers untrusted pagination Link headers from an upstream response.
   */
  private allowedApiUrl(url: string): boolean {
    try {
      const parsed = new URL(url);
      return (
        parsed.protocol === "https:" &&
        parsed.origin === API &&
        parsed.port === "" &&
        (parsed.pathname === "/graphql" ||
          parsed.pathname === this.repositoryPath ||
          parsed.pathname.startsWith(`${this.repositoryPath}/`))
      );
    } catch {
      return false;
    }
  }

  private async request(url: string, init: RequestInit = {}): Promise<Response | null> {
    if (!this.allowedApiUrl(url)) return null;
    if (this.requestsUsed >= this.maxExternalRequests) return null;
    this.requestsUsed += 1;
    try {
      const headers = new Headers(init.headers);
      headers.set("authorization", `Bearer ${this.options.token}`);
      headers.set("accept", "application/vnd.github+json");
      headers.set("user-agent", "tachiko-work-project-dashboard/0.1");
      return await this.fetcher(url, {
        ...init,
        headers,
        redirect: "error",
      });
    } catch {
      return null;
    }
  }

  private async json(url: string, init: RequestInit = {}): Promise<unknown | null> {
    const response = await this.request(url, init);
    if (response === null || !response.ok) return null;
    try {
      return await response.json();
    } catch {
      return null;
    }
  }

  /** A later-page error makes the whole collection partial, never complete. */
  private async pages<T>(url: string, followNext = true): Promise<Page<T>> {
    const rows: T[] = [];
    let current: string | null = url;
    let sawPage = false;
    const visited = new Set<string>();
    let fetchedPages = 0;
    while (current !== null) {
      if (fetchedPages >= MAX_PAGES_PER_CONNECTION || visited.has(current)) {
        return sawPage
          ? { availability: "partial", value: rows }
          : { availability: "unavailable", value: null };
      }
      visited.add(current);
      fetchedPages += 1;
      const response = await this.request(current);
      if (response === null || !response.ok) {
        return sawPage
          ? { availability: "partial", value: rows }
          : { availability: "unavailable", value: null };
      }
      let value: unknown;
      try {
        value = await response.json();
      } catch {
        return sawPage
          ? { availability: "partial", value: rows }
          : { availability: "unavailable", value: null };
      }
      if (!Array.isArray(value)) {
        return sawPage
          ? { availability: "partial", value: rows }
          : { availability: "unavailable", value: null };
      }
      rows.push(...(value as T[]));
      sawPage = true;
      current = followNext ? nextPage(response) : null;
    }
    return { availability: "complete", value: rows };
  }

  private async main(): Promise<Observed<string>> {
    const repository = await this.json(this.repositoryUrl);
    const branch = record(repository) ? text(repository.default_branch) : null;
    if (branch === null) return unavailable();
    const ref = await this.json(
      `${this.repositoryUrl}/git/ref/heads/${encodeURIComponent(branch)}`,
    );
    const sha = record(ref) && record(ref.object) ? text(ref.object.sha) : null;
    return sha === null ? unavailable() : observed("complete", sha);
  }

  private async dependencies(issue: number): Promise<Observed<readonly number[]>> {
    const page = await this.pages<GitHubRow>(
      `${this.repositoryUrl}/issues/${issue}/dependencies/blocked_by`,
    );
    if (page.value === null) return unavailable();
    const values: number[] = [];
    for (const row of page.value) {
      if (!record(row)) return unavailable();
      const dependency = positiveNumber(row.number);
      if (dependency === null) return unavailable();
      values.push(dependency);
    }
    return observed(page.availability, values);
  }

  private async linkedIssues(pr: number): Promise<Observed<readonly number[]>> {
    const [owner, name] = repositoryParts(this.options.repository);
    const query =
      `query ClosingIssues($owner: String!, $name: String!, $number: Int!, $after: String) { repository(owner: $owner, name: $name) { pullRequest(number: $number) { closingIssuesReferences(first: 100, after: $after) { nodes { number repository { nameWithOwner } } pageInfo { hasNextPage endCursor } } } } }`;
    const values: number[] = [];
    let after: string | null = null;
    let sawPage = false;
    const visitedCursors = new Set<string>();
    let fetchedPages = 0;
    while (true) {
      const cursor = after ?? "<initial>";
      if (fetchedPages >= MAX_PAGES_PER_CONNECTION || visitedCursors.has(cursor)) {
        return sawPage ? observed("partial", values) : unavailable();
      }
      visitedCursors.add(cursor);
      fetchedPages += 1;
      const payload = await this.json(`${API}/graphql`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          query,
          variables: { owner, name, number: pr, after },
        }),
      });
      const connection =
        record(payload) &&
        record(payload.data) &&
        record(payload.data.repository) &&
        record(payload.data.repository.pullRequest) &&
        record(payload.data.repository.pullRequest.closingIssuesReferences)
          ? payload.data.repository.pullRequest.closingIssuesReferences
          : null;
      const pageInfo =
        connection !== null && record(connection.pageInfo)
          ? connection.pageInfo
          : null;
      const nodes =
        connection !== null && Array.isArray(connection.nodes)
          ? connection.nodes
          : null;
      if (
        connection === null ||
        pageInfo === null ||
        nodes === null ||
        (record(payload) && Array.isArray(payload.errors))
      ) {
        return sawPage ? observed("partial", values) : unavailable();
      }
      for (const node of nodes) {
        if (!record(node) || positiveNumber(node.number) === null || !record(node.repository)) {
          return unavailable();
        }
        // The number-only projection cannot represent a foreign repository link safely.
        if (text(node.repository.nameWithOwner) !== this.options.repository) {
          return unavailable();
        }
        values.push(positiveNumber(node.number)!);
      }
      sawPage = true;
      if (pageInfo.hasNextPage !== true) return observed("complete", values);
      after = text(pageInfo.endCursor);
      if (after === null) return observed("partial", values);
    }
  }

  private parseWatch(
    row: unknown,
    prNumber: number,
    availability: Availability,
  ): StewardWatchObservation | null {
    if (
      !record(row) ||
      !record(row.user) ||
      !this.options.trustedStewardLogins.includes(text(row.user.login) ?? "")
    ) {
      return null;
    }
    const body = text(row.body);
    const sourceUrl = text(row.html_url);
    if (body === null || sourceUrl === null || !body.startsWith("<!-- project-steward-watch:v1 -->\n")) {
      return null;
    }
    const separator = body.indexOf("\n\n");
    if (separator < 0) return null;
    const lines = body.slice(0, separator).split("\n");
    if (lines.length !== 5) return null;
    const fields = new Map<string, string>();
    for (const line of lines.slice(1)) {
      const match = /^(VERDICT|HEAD|MAIN|HUMAN_ACTION): ([^\r\n]+)$/.exec(line);
      if (match === null || fields.has(match[1]!)) return null;
      fields.set(match[1]!, match[2]!);
    }
    if (["VERDICT", "HEAD", "MAIN", "HUMAN_ACTION"].some((key) => !fields.has(key))) {
      return null;
    }
    const verdict = fields.get("VERDICT");
    const humanAction = fields.get("HUMAN_ACTION");
    const head = fields.get("HEAD");
    const main = fields.get("MAIN");
    if (
      (verdict !== "GREEN" && verdict !== "AMBER" && verdict !== "HOLD") ||
      (humanAction !== "none" && humanAction !== "required") ||
      head === undefined ||
      main === undefined ||
      !SHA.test(head) ||
      !SHA.test(main)
    ) {
      return null;
    }
    return { prNumber, availability, verdict, humanAction, head, main, sourceUrl };
  }

  private async comments(pr: number): Promise<Page<StewardWatchObservation>> {
    const page = await this.pages<unknown>(`${this.repositoryUrl}/issues/${pr}/comments`);
    const watches =
      page.value
        ?.map((row) => this.parseWatch(row, pr, page.availability))
        .filter((watch): watch is StewardWatchObservation => watch !== null) ?? null;
    return page.availability === "unavailable"
      ? { availability: "unavailable", value: null }
      : { availability: page.availability, value: watches };
  }

  async snapshot(): Promise<ObservationSnapshot> {
    const [main, issuePage, pullPage, activityPage] = await Promise.all([
      this.main(),
      this.pages<GitHubRow>(`${this.repositoryUrl}/issues?state=open`),
      this.pages<GitHubRow>(`${this.repositoryUrl}/pulls?state=open`),
      this.pages<GitHubRow>(
        `${this.repositoryUrl}/pulls?state=closed&sort=updated&direction=desc&per_page=${RECENT_ACTIVITY_WINDOW}`,
        false,
      ),
    ]);
    const issueCore = (issuePage.value ?? [])
      .filter((row) => row.pull_request === undefined)
      .map((row) => ({
        number: positiveNumber(row.number),
        title: text(row.title),
        state: issueState(row.state),
      }));
    const issueValid = issueCore.every(
      (row) => row.number !== null && row.title !== null && row.state !== null,
    );
    const issues =
      issuePage.availability === "unavailable" || !issueValid
        ? unavailable<readonly IssueObservation[]>()
        : observed(
            issuePage.availability,
            await mapBounded(
              issueCore,
              MAX_ENTITY_CONCURRENCY,
              async (row) => ({
                number: row.number!,
                title: row.title!,
                state: row.state!,
                dependencies: await this.dependencies(row.number!),
              }),
            ),
          );
    const pullCore = (pullPage.value ?? []).map((row) => ({
      number: positiveNumber(row.number),
      title: text(row.title),
      state: issueState(row.state),
      draft: typeof row.draft === "boolean" ? row.draft : null,
      head: text(row.head?.sha),
      base: text(row.base?.sha),
    }));
    const pullValid = pullCore.every(
      (row) =>
        row.number !== null &&
        row.title !== null &&
        row.state !== null &&
        row.draft !== null,
    );
    const pullRequests =
      pullPage.availability === "unavailable" || !pullValid
        ? unavailable<readonly PullObservation[]>()
        : observed(
            pullPage.availability,
            await mapBounded(
              pullCore,
              MAX_ENTITY_CONCURRENCY,
              async (row) => ({
                number: row.number!,
                title: row.title!,
                state: row.state!,
                draft: row.draft!,
                head: row.head === null ? unavailable<string>() : observed("complete", row.head),
                base: row.base === null ? unavailable<string>() : observed("complete", row.base),
                linkedIssues: await this.linkedIssues(row.number!),
              }),
            ),
          );
    const commentResults = await mapBounded(
      pullRequests.value ?? [],
      MAX_ENTITY_CONCURRENCY,
      (pull) => this.comments(pull.number),
    );
    const commentAvailability: Availability =
      commentResults.length === 0
        ? pullRequests.availability
        : commentResults.every((result) => result.availability === "unavailable")
          ? "unavailable"
          : commentResults.some((result) => result.availability !== "complete")
            ? "partial"
            : "complete";
    const parsedWatches = commentResults.flatMap((result) =>
      (result.value ?? []).map((watch) => ({
        ...watch,
        availability:
          pullRequests.availability === "complete"
            ? watch.availability
            : pullRequests.availability,
      })),
    );
    const watchCounts = new Map<number, number>();
    for (const watch of parsedWatches) {
      watchCounts.set(watch.prNumber, (watchCounts.get(watch.prNumber) ?? 0) + 1);
    }
    // A PR has at most one canonical watch; ambiguity cannot become attention.
    const uniqueWatches = parsedWatches.filter(
      (watch) => watchCounts.get(watch.prNumber) === 1,
    );
    const stewardWatches =
      commentAvailability === "unavailable"
        ? unavailable<readonly StewardWatchObservation[]>()
        : observed(commentAvailability, uniqueWatches);
    const recentCore = (activityPage.value ?? [])
      .filter((row) => row.merged_at !== null && row.merged_at !== undefined)
      .map((row) => ({ number: positiveNumber(row.number), title: text(row.title) }));
    const recentValid = recentCore.every(
      (row) => row.number !== null && row.title !== null,
    );
    const recentActivity =
      activityPage.availability === "unavailable" || !recentValid
        ? unavailable<readonly RecentActivityObservation[]>()
        : observed(
            activityPage.availability,
            recentCore.map((row) => ({ number: row.number!, title: row.title! })),
          );
    return { main, issues, pullRequests, stewardWatches, recentActivity };
  }
}

/** Must observe GitHub, not accept an already-normalized projection as its input. */
export async function observeGitHub(options: DashboardOptions): Promise<ObservationSnapshot> {
  return new GitHubObserver(options).snapshot();
}

function sourceUrl(repository: string, path: string): string {
  return `https://github.com/${repository}${path}`;
}

/** Build the exact envelope consumed by both the Node and Worker runtimes. */
export function createDashboardEnvelope(
  options: DashboardOptions,
  latest: ObservationSnapshot,
): DashboardEnvelope {
  const projection = projectObservation({ latest });
  const main = projection.executive.mainSha.value;
  return {
    repository: options.repository,
    projection,
    sources: {
      main: {
        availability: latest.main.availability,
        url: main === null ? sourceUrl(options.repository, "") : sourceUrl(options.repository, `/commit/${main}`),
      },
      issues: {
        availability: latest.issues.availability,
        url: sourceUrl(options.repository, "/issues"),
      },
      pullRequests: {
        availability: latest.pullRequests.availability,
        url: sourceUrl(options.repository, "/pulls"),
      },
      recentActivity: {
        availability: latest.recentActivity.availability,
        url: sourceUrl(
          options.repository,
          "/pulls?state=closed&sort=updated&direction=desc",
        ),
      },
    },
  };
}

export function unknownDashboardEnvelope(repository: string): DashboardEnvelope {
  const fact = { availability: "unavailable" as const, value: null };
  return {
    repository,
    projection: {
      executive: { mainSha: fact },
      deliveries: [],
      criticalPath: { availability: "unavailable", issueNumbers: [] },
      recentActivity: fact,
      attention: { items: [] },
    },
    sources: {
      main: { availability: "unavailable", url: "" },
      issues: { availability: "unavailable", url: "" },
      pullRequests: { availability: "unavailable", url: "" },
      recentActivity: { availability: "unavailable", url: "" },
    },
  };
}

/** Static application shell shared by the Node and Worker deployments. */
export const dashboardHtml = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Operational Dashboard</title><style>*{box-sizing:border-box}body{margin:0;background:#0b1020;color:#edf2ff;font:16px system-ui,sans-serif;overflow-wrap:anywhere}main{max-width:1100px;margin:auto;padding:20px}section{border:1px solid #35405c;border-radius:10px;margin:14px 0;padding:14px;background:#121a2d}h1,h2{margin:0 0 10px}h2{font-size:1.05rem;color:#b7c9ff}a{color:#9ed5ff}button{font:inherit;padding:8px 14px;border-radius:6px;border:1px solid #9ed5ff;background:#172948;color:#fff;cursor:pointer}.card{border-top:1px solid #35405c;padding:10px 0}.label{color:#aebbd8}.unknown{color:#ffd38a}@media (max-width:390px){main{padding:10px}section{padding:10px}}</style></head><body><main><h1>Operational Dashboard</h1><button id="refresh" type="button">Refresh</button><div id="dashboard"></div></main><script>const root=document.getElementById('dashboard');const esc=v=>String(v??'Unknown').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;');const fact=x=>x&&x.availability==='complete'?'<span>'+esc(x.value)+'</span>':'<span class="unknown">Unknown ('+esc(x?.availability)+')</span>';const link=(url,label)=>{try{if(new URL(url).protocol!=='https:')return label;return '<a href="'+esc(url)+'" rel="noreferrer">'+label+'</a>'}catch{return label}};function render(data){const p=data.projection,src=data.sources;const lanes=p.deliveries.map(l=>'<article class="card" data-testid="delivery-'+(l.issueNumber??'none')+'-'+(l.pullRequestNumber??'none')+'">'+(l.issueNumber===null?'Issue: Unknown':link(src.issues.url+'/'+l.issueNumber,'Issue #'+l.issueNumber))+': '+fact(l.issueTitle)+'<br>'+(l.pullRequestNumber===null?'PR: Unknown':link(src.pullRequests.url.replace('/pulls','/pull')+'/'+l.pullRequestNumber,'PR #'+l.pullRequestNumber))+': '+fact(l.pullRequestTitle)+'<br><span class="label">HEAD</span> '+fact(l.pullRequestHead)+'<br><span class="label">BASE</span> '+fact(l.pullRequestBase)+'</article>').join('')||'<span class="unknown">Unknown</span>';const work=p.criticalPath.availability==='complete'?'Current issues: '+link(src.issues.url,'Issues')+' '+p.criticalPath.issueNumbers.map(n=>link(src.issues.url+'/'+n,String(n))).join(', '):'<span class="unknown">Unknown ('+p.criticalPath.availability+')</span>';const activity=p.recentActivity.availability==='complete'?p.recentActivity.value.map(a=>'<div>'+link(src.recentActivity.url,'#'+a.number)+' '+esc(a.title)+'</div>').join(''):'<span class="unknown">Unknown ('+p.recentActivity.availability+')</span>';const attention=p.attention.items.map(a=>'<div>'+link(a.sourceUrl,a.kind==='steward-hold'?'HOLD':'Human action required')+'</div>').join('')||'<span>Attention is shown only when positively observed.</span>';root.innerHTML='<section><h2>Executive strip</h2><div data-testid="main-sha">'+(p.executive.mainSha.availability==='complete'?link(src.main.url,esc(p.executive.mainSha.value)):fact(p.executive.mainSha))+'</div></section><section><h2>Delivery command center</h2><div data-testid="deliveries">'+lanes+'</div></section><section><h2>Current work</h2><div data-testid="current-work">'+work+'</div></section><section><h2>Recent activity</h2><div data-testid="recent-activity">'+activity+'</div></section><section><h2>Authority & attention</h2><div data-testid="attention">'+attention+'</div></section>'}async function refresh(){try{const r=await fetch('/api/projection',{cache:'no-store'});if(!r.ok)throw new Error('refresh failed');render(await r.json())}catch{render(${JSON.stringify(unknownDashboardEnvelope(""))})}}document.getElementById('refresh').addEventListener('click',refresh);refresh();</script></body></html>`;
