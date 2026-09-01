import "cybercore-css";
import "./styles.css";

import type {
  DeliveryLane,
  DisplaySignal,
  RepositoryProjection,
  SourceLink,
} from "../shared/model.js";

const root = document.querySelector<HTMLDivElement>("#app");
if (root === null) throw new Error("dashboard root is missing");
const app: HTMLDivElement = root;

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

function shortSha(value: string): string {
  return /^[0-9a-f]{40}$/.test(value) ? value.slice(0, 10) : value;
}

function sourceLink(source: SourceLink): HTMLAnchorElement {
  const link = externalLink(
    source.url,
    source.label,
    `source-link source-${source.evidenceClass}`,
  );
  link.dataset.evidenceClass = source.evidenceClass;
  return link;
}

function externalLink(url: string, text: string, className?: string): HTMLAnchorElement {
  const link = element("a", className, text);
  link.href = url;
  link.target = "_blank";
  link.rel = "noreferrer";
  return link;
}

function sources(values: SourceLink[]): HTMLElement {
  const container = element("div", "source-list");
  const unique = new Map(values.map((value) => [`${value.label}:${value.url}`, value]));
  for (const value of unique.values()) container.append(sourceLink(value));
  return container;
}

function signal(value: DisplaySignal): HTMLElement {
  const container = element("div", `signal signal-${value.state}`);
  const dot = element("span", "signal-dot");
  dot.setAttribute("aria-hidden", "true");
  container.append(dot, element("span", "signal-label", value.label));
  container.title = value.reason;
  return container;
}

function section(
  id: string,
  eyebrow: string,
  title: string,
  surface: string,
): HTMLElement {
  const node = element("section", "panel command-panel");
  node.id = id;
  node.dataset.surface = surface;
  const heading = element("div", "section-heading");
  heading.append(element("span", "eyebrow", eyebrow), element("h2", undefined, title));
  node.append(heading);
  return node;
}

function executiveStrip(projection: RepositoryProjection): HTMLElement {
  const strip = element("section", "executive-strip");
  strip.dataset.surface = "executive-strip";
  const cells: [string, string, string][] = [
    ["LIVE MAIN", shortSha(projection.executive.mainSha.value), projection.executive.mainSha.state],
    ["HORIZON", projection.executive.productHorizon.value, projection.executive.productHorizon.state],
    ["FETCH", projection.fetchHealth.toUpperCase(), projection.fetchHealth === "healthy" ? "satisfied" : "unknown"],
    [
      "ACTIVE / READY",
      `${String(projection.executive.activeCount.value)} / ${String(projection.executive.readyCount.value)}`,
      projection.executive.activeCount.state === "satisfied" &&
      projection.executive.readyCount.state === "satisfied"
        ? "satisfied"
        : "unknown",
    ],
    ["HUMAN ACTION", projection.humanAction.label, projection.humanAction.state],
  ];
  for (const [label, value, state] of cells) {
    const cell = element("div", `executive-cell state-${state}`);
    cell.append(element("span", "metric-label", label), element("strong", "metric-value", value));
    strip.append(cell);
  }
  return strip;
}

function laneCard(lane: DeliveryLane): HTMLElement {
  const card = element("article", "lane-card");
  const header = element("header", "lane-header");
  const laneIdentity =
    lane.issue === null
      ? {
          text: `PR #${String(lane.pullRequest?.number ?? 0)} · ${lane.pullRequest?.title ?? "Unlinked pull request"}`,
          url: lane.pullRequest?.url ?? "#",
        }
      : {
          text: `#${String(lane.issue.number)} · ${lane.issue.title}`,
          url: lane.issue.url,
        };
  const issue = externalLink(laneIdentity.url, laneIdentity.text, "lane-issue");
  header.append(issue, element("span", "phase-chip", lane.phase.replace("_", " ")));

  const identity = element("div", "identity-grid");
  if (lane.pullRequest === null) {
    identity.append(
      element(
        "span",
        "identity-empty",
        lane.mergeGate.reason === "not-required"
          ? "No implementation PR · native Ready lane"
          : "Implementation PR linkage Unknown · native Issue lane",
      ),
    );
  } else {
    const values: [string, string][] = [
      ["PR", `#${String(lane.pullRequest.number)}`],
      ["HEAD", shortSha(lane.pullRequest.headSha)],
      ["BASE TIP", shortSha(lane.pullRequest.baseSha)],
      ["MERGE BASE", shortSha(lane.pullRequest.mergeBaseSha ?? "Unknown")],
      ["LIVE MAIN", shortSha(lane.pullRequest.liveMainSha)],
      ["RELATION", lane.pullRequest.relationToMain],
    ];
    for (const [label, value] of values) {
      const item = element("div", "identity-item");
      item.append(element("span", "metric-label", label), element("code", undefined, value));
      identity.append(item);
    }
  }

  const conditions = element("div", "condition-grid");
  for (const value of [
    lane.readiness,
    lane.checks,
    lane.review,
    lane.handoff,
    lane.stewardWatch,
    lane.authority,
    lane.humanAction,
    lane.mergeGate,
  ]) {
    conditions.append(signal(value));
  }
  const evidence = element("div", "evidence-row");
  const evidenceChip = (label: string, value: DisplaySignal) =>
    element(
      "span",
      `evidence-chip state-${value.state}`,
      `${label} · ${value.state}`,
    );
  evidence.append(
    evidenceChip("Automated browser", lane.evidence.automatedBrowser),
    evidenceChip("Perceptual review", lane.evidence.perceptualReview),
    evidenceChip("Delivery integrity", lane.evidence.deliveryIntegrity),
  );
  card.append(header, element("p", "lane-owner", `OWNER / ${lane.owner}`), identity, conditions, evidence, sources(lane.sources));
  return card;
}

function commandCenter(projection: RepositoryProjection): HTMLElement {
  const node = section("delivery-command-center", "01 / DELIVERY", "Command center", "delivery-command-center");
  const grid = element("div", "lane-grid");
  if (projection.deliveries.length === 0) {
    grid.append(element("p", "empty-state", "Delivery observation unavailable or no agent-owned lanes found."));
  } else {
    for (const lane of projection.deliveries) grid.append(laneCard(lane));
  }
  node.append(grid);
  return node;
}

function criticalPath(projection: RepositoryProjection): HTMLElement {
  const node = section("critical-path", "02 / SEQUENCE", "Critical path · current work", "critical-path");
  const track = element("div", "critical-track");
  for (const item of projection.criticalPath.nodes) {
    const card = externalLink(item.url, "", `critical-node state-${item.state}`);
    const dependencies = projection.criticalPath.edges.filter(
      (edge) => edge.to === item.issueNumber,
    );
    card.append(
      element("span", "metric-label", `ISSUE #${String(item.issueNumber)}`),
      element("strong", undefined, item.label),
      element("span", "critical-state", item.state),
      element(
        "span",
        "critical-dependencies",
        dependencies.length === 0
          ? "No native dependency"
          : dependencies
              .map(
                (edge) =>
                  `${edge.state === "satisfied" ? "Cleared" : "Waiting on"} #${String(edge.from)}`,
              )
              .join(" · "),
      ),
    );
    track.append(card);
  }
  if (projection.criticalPath.nodes.length === 0) {
    track.append(element("p", "empty-state", "Current dependency observation is Unknown."));
  }
  node.append(track);
  return node;
}

function recentActivity(projection: RepositoryProjection): HTMLElement {
  const node = section("recent-activity", "03 / RECALIBRATE", "Recent merges & activity", "recent-activity");
  const list = element("ol", "activity-list");
  for (const activity of projection.recentActivity) {
    const item = element("li", "activity-item");
    const link = externalLink(
      activity.url,
      `PR #${String(activity.number)} · ${activity.title}`,
    );
    item.append(
      element("time", undefined, new Date(activity.mergedAt).toLocaleString()),
      link,
      element("code", undefined, shortSha(activity.mergeSha)),
    );
    list.append(item);
  }
  if (projection.recentActivity.length === 0) {
    list.append(element("li", "empty-state", "Recent activity is Unknown."));
  }
  node.append(list);
  return node;
}

function attentionPanel(projection: RepositoryProjection): HTMLElement {
  const node = section("authority-attention", "04 / AUTHORITY", "Attention & reconciliation", "authority-attention");
  const summary = element("div", `attention-summary state-${projection.humanAction.state}`);
  summary.append(element("span", "metric-label", "FOUNDER / STEWARD ACTION"), signal(projection.humanAction));
  node.append(summary);
  const list = element("div", "attention-list");
  if (projection.attention.length === 0) {
    list.append(element("p", "empty-state", "No current structured attention items."));
  }
  for (const item of projection.attention) {
    const card = element("article", `attention-item signal-${item.state}`);
    card.append(
      element("span", "metric-label", item.issueNumber === undefined ? "REPOSITORY" : `ISSUE #${String(item.issueNumber)}`),
      element("strong", undefined, item.label),
      element("code", undefined, item.reason),
      sources(item.sources),
    );
    list.append(card);
  }
  node.append(list);
  return node;
}

function shell(): { main: HTMLElement; refresh: HTMLButtonElement; live: HTMLElement } {
  app.replaceChildren();
  const ambient = element("div", "ambient-grid");
  ambient.setAttribute("aria-hidden", "true");
  const header = element("header", "site-header");
  const brand = element("div", "brand-lockup");
  brand.append(
    element("span", "brand-mark", "TW / 06"),
    element("h1", undefined, "Live Project Control Room"),
    element("p", undefined, "Read-only · source-linked · exact-identity operational projection"),
  );
  const refresh = element("button", "refresh-button", "Refresh observation");
  refresh.type = "button";
  const live = element("p", "sr-only");
  live.setAttribute("aria-live", "polite");
  header.append(brand, refresh, live);
  const main = element("main", "dashboard-shell");
  main.id = "dashboard-main";
  main.tabIndex = -1;
  app.append(ambient, header, main);
  return { main, refresh, live };
}

const frame = shell();

async function loadProjection(restoreFocus = false): Promise<void> {
  frame.refresh.disabled = true;
  frame.refresh.dataset.loading = "true";
  try {
    const response = await fetch("/api/project", { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(`observation request failed (${String(response.status)})`);
    const projection = (await response.json()) as RepositoryProjection;
    frame.main.replaceChildren(
      executiveStrip(projection),
      commandCenter(projection),
      criticalPath(projection),
      recentActivity(projection),
      attentionPanel(projection),
      sources(projection.sources),
    );
    frame.live.textContent = `Observation refreshed. Fetch health ${projection.fetchHealth}.`;
  } catch {
    frame.main.replaceChildren(
      element("section", "fatal-observation signal-unknown", "Project observation unavailable · state is Unknown."),
    );
    frame.live.textContent = "Observation unavailable. State is Unknown.";
  } finally {
    frame.refresh.disabled = false;
    delete frame.refresh.dataset.loading;
    if (restoreFocus) frame.refresh.focus();
  }
}

frame.refresh.addEventListener("click", () => {
  void loadProjection(true);
});

void loadProjection();
