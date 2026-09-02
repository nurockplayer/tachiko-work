import "cybercore-css";
import "./styles.css";

import type {
  Availability,
  DashboardProjection,
  DeliveryLane,
  ObservedValue,
  PullRequestFact,
  SourceLink,
  StructuredFact,
} from "../shared/model.js";

const root = document.querySelector<HTMLDivElement>("#app");
if (root === null) throw new Error("dashboard root is missing");
const app: HTMLDivElement = root;
let lastProjection: DashboardProjection | null = null;

function element<K extends keyof HTMLElementTagNameMap>(
  name: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(name);
  if (className !== undefined) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function display(value: string | number | boolean | null): string {
  if (value === null) return "Unknown";
  return typeof value === "boolean" ? (value ? "Yes" : "No") : String(value);
}

function externalLink(url: string, text: string, className?: string): HTMLAnchorElement {
  const link = element("a", className, text);
  link.href = url;
  link.target = "_blank";
  link.rel = "noreferrer noopener";
  return link;
}

function sourceLink(source: SourceLink): HTMLAnchorElement {
  const link = externalLink(source.url, source.label, `source-link source-${source.kind}`);
  link.dataset.sourceKind = source.kind;
  return link;
}

function sources(values: SourceLink[]): HTMLElement {
  const container = element("div", "source-list");
  const unique = new Map(values.map((value) => [`${value.label}:${value.url}`, value]));
  for (const value of unique.values()) container.append(sourceLink(value));
  return container;
}

function availability(value: Availability): HTMLElement {
  return element("span", `availability availability-${value}`, value);
}

function section(
  id: string,
  eyebrow: string,
  title: string,
  surfaceName: string,
): HTMLElement {
  const node = element("section", "panel");
  node.id = id;
  node.dataset.surface = surfaceName;
  const heading = element("div", "section-heading");
  heading.append(element("span", "eyebrow", eyebrow), element("h2", undefined, title));
  node.append(heading);
  return node;
}

function executiveValue(label: string, observed: ObservedValue<string | number>): HTMLElement {
  const cell = element("div", `executive-cell availability-${observed.availability}`);
  cell.append(
    element("span", "metric-label", label),
    element("strong", "metric-value", display(observed.value)),
    availability(observed.availability),
  );
  cell.append(sources([observed.source, ...(observed.additionalSources ?? [])]));
  return cell;
}

function executiveStrip(projection: DashboardProjection): HTMLElement {
  const node = section("executive-state", "00 / NOW", "Executive / current state", "executive-strip");
  const grid = element("div", "executive-grid");
  grid.append(
    executiveValue("LIVE MAIN", {
      ...projection.executive.mainSha,
      value: projection.executive.mainSha.value,
    }),
    executiveValue("HORIZON", projection.executive.productHorizon),
    executiveValue("FETCH", {
      value: projection.fetchHealth.toUpperCase(),
      availability: projection.fetchHealth === "healthy" ? "complete" : projection.fetchHealth,
      source: projection.sources[0] ?? projection.executive.mainSha.source,
    }),
    executiveValue("ACTIVE / READY", {
      value: projection.executive.activeCount.value === null || projection.executive.readyCount.value === null
        ? null
        : `${String(projection.executive.activeCount.value)} / ${String(projection.executive.readyCount.value)}`,
      availability: projection.executive.activeCount.availability === "complete" && projection.executive.readyCount.availability === "complete"
        ? "complete"
        : projection.executive.activeCount.availability === "unavailable" && projection.executive.readyCount.availability === "unavailable"
          ? "unavailable"
          : "partial",
      source: projection.executive.activeCount.source,
    }),
    executiveValue("HUMAN ACTION", projection.executive.humanAction),
  );
  node.append(grid);
  return node;
}

function fact(label: string, value: string | number | boolean | null): HTMLElement {
  const item = element("div", "identity-item");
  item.append(element("span", "metric-label", label), element("code", undefined, display(value)));
  return item;
}

function factGroup(title: string, factAvailability: Availability, items: HTMLElement[]): HTMLElement {
  const group = element("section", "fact-group");
  const heading = element("div", "fact-group-heading");
  heading.append(element("h4", undefined, title), availability(factAvailability));
  const grid = element("div", "identity-grid");
  grid.append(...items);
  group.append(heading, grid);
  return group;
}

function listGroup(
  title: string,
  listAvailability: Availability,
  rows: { text: string; source: SourceLink }[],
): HTMLElement {
  const group = element("section", "fact-group");
  const heading = element("div", "fact-group-heading");
  heading.append(element("h4", undefined, title), availability(listAvailability));
  group.append(heading);
  if (rows.length === 0) {
    group.append(element("p", "empty-state", listAvailability === "complete" ? "None observed" : "Unknown"));
  } else {
    const list = element("ul", "fact-list");
    for (const row of rows) {
      const item = element("li");
      item.append(element("span", undefined, row.text), sourceLink(row.source));
      list.append(item);
    }
    group.append(list);
  }
  return group;
}

function structuredGroup(title: string, value: StructuredFact): HTMLElement {
  const group = element("section", `structured-fact structured-${value.status}`);
  const heading = element("div", "fact-group-heading");
  heading.append(element("h4", undefined, title), element("span", "structured-status", value.status));
  group.append(heading, element("p", undefined, value.value ?? `Unknown · ${value.reason}`));
  if (value.source !== null) group.append(sourceLink(value.source));
  return group;
}

function pullRequestGroups(pull: PullRequestFact): HTMLElement[] {
  const checks = pull.checks.items.map((check) => ({
    text: `${check.name} · status ${display(check.status)} · conclusion ${display(check.conclusion)} · head ${display(check.headSha)}`,
    source: { label: "Native check", url: check.url, kind: "github" as const },
  }));
  const reviews = pull.reviews.items.map((review) => ({
    text: `${review.author} · ${review.state} · commit ${review.commitSha ?? "Unknown"} · exact head ${display(review.exactHead)}`,
    source: { label: "Native review", url: review.url, kind: "github" as const },
  }));
  return [
    factGroup("Pull request identity", pull.identityAvailability, [
      fact("STATE", pull.state),
      fact("DRAFT", pull.draft),
      fact("HEAD", pull.headSha),
      fact("BASE", `${pull.baseRef} · ${pull.baseSha}`),
    ]),
    factGroup("GitHub native fields · displayed verbatim", pull.nativeAvailability, [
      fact("MERGEABLE", pull.mergeable),
      fact("MERGE STATE", pull.mergeStateStatus),
      fact("REVIEW DECISION", pull.reviewDecision),
      fact(
        "LINKED ISSUES",
        pull.linkedIssueNumbers.length === 0
          ? pull.linkageAvailability === "complete" ? "None observed" : null
          : pull.linkedIssueNumbers.map((number) => `#${String(number)}`).join(" · "),
      ),
    ]),
    listGroup("Checks", pull.checks.availability, checks),
    listGroup("Reviews", pull.reviews.availability, reviews),
    structuredGroup("Canonical handoff", pull.handoff),
    structuredGroup("Steward watch", pull.stewardWatch),
  ];
}

function laneCard(lane: DeliveryLane): HTMLElement {
  const card = element("article", "lane-card");
  const header = element("header", "lane-header");
  if (lane.issue !== null) {
    header.append(externalLink(
      lane.issue.url,
      `#${String(lane.issue.number)} · ${lane.issue.title}`,
      "lane-title",
    ));
  } else if (lane.pullRequest !== null) {
    header.append(externalLink(
      lane.pullRequest.url,
      `PR #${String(lane.pullRequest.number)} · ${lane.pullRequest.title}`,
      "lane-title",
    ));
  }
  card.append(header);

  if (lane.issue !== null) {
    card.append(factGroup("Issue facts", lane.issue.availability, [
      fact("STATE", lane.issue.state),
      fact(
        "LABELS",
        lane.issue.labels.length === 0
          ? lane.issue.labelsAvailability === "complete" ? "None observed" : null
          : lane.issue.labels.join(" · "),
      ),
      fact(
        "MILESTONE",
        lane.issue.milestone ?? (lane.issue.milestoneAvailability === "complete" ? "None observed" : null),
      ),
      fact(
        "BLOCKED BY",
        lane.issue.blockedBy.length === 0
          ? lane.issue.dependenciesAvailability === "complete" ? "None observed" : null
          : lane.issue.blockedBy.map((item) => `#${String(item.number)} · ${item.state}`).join(" · "),
      ),
    ]));
  }
  if (lane.pullRequest !== null) {
    if (lane.issue !== null) {
      card.append(externalLink(lane.pullRequest.url, `Open PR #${String(lane.pullRequest.number)}`, "pr-link"));
    }
    card.append(...pullRequestGroups(lane.pullRequest));
  } else {
    card.append(
      element(
        "p",
        "empty-state",
        lane.linkageAvailability === "complete"
          ? "No implementation pull request observed"
          : "Implementation pull request linkage Unknown",
      ),
      availability(lane.linkageAvailability),
    );
  }
  return card;
}

function commandCenter(projection: DashboardProjection): HTMLElement {
  const node = section(
    "delivery-command-center",
    "01 / DELIVERY",
    "Delivery command center",
    "delivery-command-center",
  );
  const grid = element("div", "lane-grid");
  if (projection.deliveries.length === 0) {
    grid.append(
      element(
        "p",
        "empty-state",
        projection.deliveriesAvailability === "complete"
          ? "No active delivery lanes observed"
          : "Delivery observation Unknown",
      ),
      availability(projection.deliveriesAvailability),
    );
  } else {
    for (const lane of projection.deliveries) grid.append(laneCard(lane));
  }
  node.append(grid);
  return node;
}

function criticalPath(projection: DashboardProjection): HTMLElement {
  const node = section(
    "critical-path",
    "02 / SEQUENCE",
    "Critical path / current work",
    "critical-path",
  );
  node.append(availability(projection.criticalPath.availability));
  if (projection.criticalPath.nodes.length === 0) {
    node.append(element("p", "empty-state", projection.criticalPath.availability === "complete" ? "No dependencies observed" : "Unknown"));
  } else {
    const nodes = element("div", "path-nodes");
    for (const item of projection.criticalPath.nodes) {
      nodes.append(externalLink(item.url, item.label, `path-node path-${item.state.toLowerCase()}`));
    }
    node.append(nodes);
    for (const edge of projection.criticalPath.edges) {
      node.append(element(
        "p",
        "path-edge",
        `#${String(edge.from)} depends on #${String(edge.to)} · ${edge.state}`,
      ));
    }
  }
  node.append(sourceLink(projection.criticalPath.source));
  return node;
}

function recentActivity(projection: DashboardProjection): HTMLElement {
  const node = section(
    "recent-activity",
    "03 / RECALIBRATE",
    "Recent merges & activity",
    "recent-activity",
  );
  node.append(availability(projection.recentActivity.availability));
  const list = element("ol", "activity-list");
  for (const item of projection.recentActivity.items) {
    const row = element("li", "activity-item");
    row.append(
      externalLink(item.url, `#${String(item.number)} · ${item.title}`),
      element("time", undefined, item.mergedAt),
      element("code", undefined, item.mergeSha),
    );
    list.append(row);
  }
  if (projection.recentActivity.items.length === 0) {
    list.append(element("li", "empty-state", projection.recentActivity.availability === "complete" ? "No recent merges observed" : "Unknown"));
  }
  node.append(list, sourceLink(projection.recentActivity.source));
  return node;
}

function attentionPanel(projection: DashboardProjection): HTMLElement {
  const node = section(
    "authority-attention",
    "04 / AUTHORITY",
    "Authority / attention",
    "authority-attention",
  );
  const list = element("div", "attention-list");
  for (const item of projection.attention) {
    const row = element("article", `attention-item attention-${item.level}`);
    row.append(element("strong", undefined, item.label), element("p", undefined, item.detail));
    if (item.sources.length > 0) row.append(sources(item.sources));
    list.append(row);
  }
  node.append(list, sources(projection.sources));
  return node;
}

function render(projection: DashboardProjection, warning?: string): void {
  const main = element("main", "dashboard-shell");
  main.id = "dashboard-main";
  const ambient = element("div", "ambient-grid");
  ambient.setAttribute("aria-hidden", "true");
  const header = element("header", "site-header");
  const identity = element("div");
  identity.append(
    element("p", "eyebrow", "TW / LIVE · READ ONLY"),
    element("h1", undefined, "Live Project Control Room"),
    element("p", "header-copy", `${projection.repository} · observed ${projection.observedAt}`),
  );
  const refresh = element("button", "refresh-button", "Refresh observation");
  refresh.type = "button";
  refresh.addEventListener("click", () => {
    void refreshProjection(true);
  });
  header.append(identity, refresh);

  const liveStatus = element("p", "sr-only");
  liveStatus.id = "refresh-status";
  liveStatus.setAttribute("aria-live", "polite");
  const refreshWarning = warning === undefined
    ? null
    : element("p", "refresh-warning", warning);
  refreshWarning?.setAttribute("role", "alert");
  main.append(
    ambient,
    header,
    liveStatus,
    ...(refreshWarning === null ? [] : [refreshWarning]),
    executiveStrip(projection),
    commandCenter(projection),
    criticalPath(projection),
    recentActivity(projection),
    attentionPanel(projection),
  );
  app.replaceChildren(main);
}

async function refreshProjection(restoreFocus: boolean): Promise<void> {
  const active = document.activeElement;
  const refresh = document.querySelector<HTMLButtonElement>(".refresh-button");
  const shouldRestoreFocus = restoreFocus && active === refresh;
  if (refresh !== null) refresh.disabled = true;
  try {
    const response = await fetch("/api/project", { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${String(response.status)}`);
    const projection = await response.json() as DashboardProjection;
    lastProjection = projection;
    render(projection);
    const nextRefresh = document.querySelector<HTMLButtonElement>(".refresh-button");
    if (shouldRestoreFocus) nextRefresh?.focus();
    const status = document.querySelector<HTMLElement>("#refresh-status");
    if (restoreFocus && status !== null) status.textContent = "Observation refreshed";
  } catch {
    if (lastProjection !== null) {
      render(
        { ...lastProjection, fetchHealth: "unavailable" },
        "Refresh failed · displayed facts are retained as stale · current live state Unknown",
      );
      const nextRefresh = document.querySelector<HTMLButtonElement>(".refresh-button");
      const status = document.querySelector<HTMLElement>("#refresh-status");
      if (status !== null) status.textContent = "Observation refresh failed · current display retained as stale";
      if (shouldRestoreFocus) nextRefresh?.focus();
      return;
    }
    const failure = element("main", "fatal-observation", "Dashboard observation unavailable · Unknown");
    failure.id = "dashboard-main";
    const retry = element("button", "refresh-button", "Retry observation");
    retry.type = "button";
    retry.addEventListener("click", () => {
      void refreshProjection(true);
    });
    failure.append(retry);
    app.replaceChildren(failure);
  }
}

void refreshProjection(false);
