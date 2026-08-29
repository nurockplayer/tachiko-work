import "cybercore-css";
import "./styles.css";

import type { DeliveryLane, RepositoryProjection, SourceRef } from "./shared/types.ts";

type Child = Node | string | null;

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Record<string, string> = {},
  children: Child[] = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (name === "class") node.className = value;
    else node.setAttribute(name, value);
  }
  for (const child of children) {
    if (child === null) continue;
    node.append(child instanceof Node ? child : document.createTextNode(child));
  }
  return node;
}

function githubUrl(value: string): string {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "https:" && parsed.hostname === "github.com" ? parsed.href : "https://github.com/nurockplayer/tachiko-work";
  } catch {
    return "https://github.com/nurockplayer/tachiko-work";
  }
}

function sourceLinks(refs: SourceRef[], label = "Sources"): HTMLElement {
  const container = element("div", { class: "source-links", role: "group", "aria-label": label });
  for (const ref of refs) {
    const identity = ref.observedIdentity === null ? "" : ` · ${shortIdentity(ref.observedIdentity)}`;
    container.append(
      element("a", { href: githubUrl(ref.url), target: "_blank", rel: "noreferrer", class: `source source--${ref.class}` }, [
        `${ref.label} · ${ref.class.replaceAll("_", " ")}${identity}`,
      ]),
    );
  }
  if (refs.length === 0) container.append(element("span", { class: "unknown" }, ["Source unavailable"]));
  return container;
}

function label(value: string): string {
  return value.replaceAll("_", " ").replace(/\b\w/g, (character) => character.toUpperCase());
}

function shortIdentity(value: string | null): string {
  if (value === null || value === "") return "Unknown";
  return /^[0-9a-f]{12,40}$/i.test(value) ? value.slice(0, 8) : value;
}

function statusTone(value: string): string {
  if (value === "no human action") return "green";
  if (/healthy|success|current|approved|none|ready|\bsatisfied\b/.test(value)) return "green";
  if (/failure|blocked|inconsistent|review_fix|changes_requested|human|unsatisfied/.test(value)) return "magenta";
  if (/partial|pending|stale|suspected|validating|rereview|unknown/.test(value)) return "yellow";
  return "chrome";
}

function badge(value: string, pulse = false): HTMLElement {
  const tone = statusTone(value);
  return element("span", { class: `cyber-badge cyber-badge--${tone} status-badge${pulse ? " status-pulse" : ""}` }, [label(value)]);
}

function definition(labelText: string, value: Child, testId?: string): HTMLElement {
  const wrapper = element("div", { class: "definition" });
  wrapper.append(element("dt", {}, [labelText]));
  const attributes = testId === undefined ? {} : { "data-testid": testId };
  wrapper.append(element("dd", attributes, [value]));
  return wrapper;
}

function executiveStrip(projection: RepositoryProjection): HTMLElement {
  const active = projection.deliveries.filter((lane) => lane.phase !== "completed" && lane.phase !== "parked").length;
  const ready = projection.deliveries.filter((lane) => lane.phase === "ready").length;
  const humanAction = projection.attention.humanActionRequired;
  const mainRef = projection.repo.sourceRefs[0];
  const roadmapRef = projection.repo.sourceRefs[1];
  const mainValue = projection.repo.mainSha === null
    ? element("span", { class: "unknown" }, ["Unknown"])
    : element("a", {
        href: githubUrl(`${mainRef?.url ?? "https://github.com/nurockplayer/tachiko-work"}/commit/${projection.repo.mainSha}`),
        target: "_blank",
        rel: "noreferrer",
      }, [shortIdentity(projection.repo.mainSha)]);
  const horizon = projection.repo.productHorizon ?? "Unknown";
  const section = element("section", { class: "panel executive-panel", "aria-labelledby": "executive-title" });
  section.append(element("h2", { id: "executive-title" }, ["Executive strip"]));
  section.append(
    element("dl", { class: "executive-grid" }, [
      definition("Live main", mainValue, "main-sha"),
      definition("Product horizon", horizon),
      definition("Fetch health", badge(projection.repo.fetchHealth, true), "fetch-health"),
      definition("Active / Ready", `${active} / ${ready}`),
      definition(
        "Human action",
        humanAction === null ? element("span", { class: "unknown" }, ["Unknown"]) : humanAction ? badge("required") : badge("none"),
        "human-action",
      ),
    ]),
  );
  const refs = projection.repo.sourceRefs;
  const links = sourceLinks(refs, "Executive sources");
  if (roadmapRef !== undefined) {
    const roadmapLink = links.querySelector<HTMLAnchorElement>(`a[href="${CSS.escape(githubUrl(roadmapRef.url))}"]`);
    if (roadmapLink !== null) roadmapLink.textContent = `Roadmap source · ${roadmapRef.class}`;
  }
  section.append(links);
  return section;
}

function deliveryCard(lane: DeliveryLane): HTMLElement {
  const article = element("article", {
    class: `cyber-card lane lane--${statusTone(lane.phase)}`,
    "aria-label": `Issue ${lane.issue.number}: ${lane.issue.title}`,
  });
  const issueLink = element("a", { href: githubUrl(lane.issue.url), target: "_blank", rel: "noreferrer" }, [
    `#${lane.issue.number} · ${lane.issue.title}`,
  ]);
  article.append(
    element("header", { class: "lane-header" }, [
      element("div", {}, [element("p", { class: "eyebrow" }, [lane.issue.milestone ?? "Cross-stage tooling / research"]), element("h3", {}, [issueLink])]),
      badge(lane.phase),
    ]),
  );

  const facts = element("dl", { class: "lane-facts" }, [
    definition("Owner", lane.owner),
    definition("Issue state", label(lane.issue.readiness)),
    definition("Handoff", badge(lane.handoff.condition)),
    definition("Authority drift", badge(lane.authorityDrift)),
  ]);
  if (lane.pr === null) {
    facts.append(definition("Pull request", "Not opened — normal for a Ready lane"));
  } else {
    facts.append(
      definition(
        "Pull request",
        element("a", { href: githubUrl(lane.pr.url), target: "_blank", rel: "noreferrer" }, [`#${lane.pr.number}`]),
      ),
      definition("Head", element("code", {}, [shortIdentity(lane.pr.headSha)])),
      definition("Base tip", element("code", {}, [shortIdentity(lane.pr.baseSha)])),
      definition("Merge base", element("code", {}, [shortIdentity(lane.pr.mergeBaseSha)])),
      definition("Live main", element("code", {}, [shortIdentity(lane.pr.liveMainSha)])),
      definition("Relation to main", badge(lane.pr.relationToMain)),
      definition("Checks", element("span", {}, [badge(lane.checks.status), ` ${lane.checks.summary}`])),
      definition(
        "Required checks",
        element("span", {}, [badge(lane.checks.requiredStatus), ` ${lane.checks.requiredSummary}`]),
      ),
      definition("Checks observed head", element("code", {}, [shortIdentity(lane.checks.observedHeadSha)])),
      definition(
        "Reviews",
        `${label(lane.reviews.decision)} · ${label(lane.reviews.status)} · ${lane.reviews.unresolvedThreadCount ?? "Unknown"} unresolved`,
      ),
      definition("Reviewed head", element("code", {}, [shortIdentity(lane.reviews.reviewedHeadSha)])),
    );
  }
  article.append(facts);

  const blockers = element("div", { class: "lane-blockers" }, [element("h4", {}, ["Blocking / reconciliation conditions"])]);
  if (lane.blockers.length === 0) blockers.append(element("p", { class: "quiet" }, ["No substantive blocker is evidenced."]));
  else blockers.append(element("ul", {}, lane.blockers.map((blocker) => element("li", {}, [blocker]))));
  article.append(blockers, sourceLinks([...lane.sourceRefs, ...lane.handoff.sourceRefs, ...lane.checks.sourceRefs, ...lane.reviews.sourceRefs]));
  return article;
}

function deliveryCenter(projection: RepositoryProjection): HTMLElement {
  const section = element("section", { class: "panel", "aria-labelledby": "delivery-title" }, [
    element("div", { class: "section-heading" }, [
      element("div", {}, [element("p", { class: "eyebrow" }, ["Independent lanes · exact identities"]), element("h2", { id: "delivery-title" }, ["Delivery command center"])]),
      badge(`${projection.deliveries.length} lanes`),
    ]),
  ]);
  const grid = element("div", { class: "delivery-grid" });
  if (projection.deliveries.length === 0) grid.append(element("p", { class: "unknown" }, ["Delivery observation is Unknown."]));
  else for (const lane of projection.deliveries) grid.append(deliveryCard(lane));
  section.append(grid);
  return section;
}

function currentWork(projection: RepositoryProjection): HTMLElement {
  const laneMap = new Map(projection.deliveries.map((lane) => [lane.id, lane]));
  const section = element("section", { class: "panel", "aria-labelledby": "current-work-title" }, [
    element("div", { class: "section-heading" }, [
      element("div", {}, [element("p", { class: "eyebrow" }, ["Derived projection · not a roadmap editor"]), element("h2", { id: "current-work-title" }, ["Current work"])]),
      badge(projection.currentWork.dependencyHealth),
    ]),
  ]);
  const list = element("ol", { class: "work-sequence", "aria-label": "Current work sequence" });
  const groups: Array<[string, string[]]> = [
    ["Current product horizon lane", projection.currentWork.currentHorizon],
    ["Independent tooling / research lane", projection.currentWork.independent],
    ["Horizon classification Unknown", projection.currentWork.unclassified],
  ];
  for (const [group, ids] of groups) {
    for (const id of ids) {
      const lane = laneMap.get(id);
      if (lane === undefined) continue;
      const dependency = lane.issue.blockedBy;
      const dependencyText = dependency === null
        ? element("span", { class: "unknown" }, ["Dependency evidence: Unknown"])
        : dependency.length === 0
          ? element("span", { class: "quiet" }, ["No live blocked-by dependency reported"])
          : element("span", {}, [
              "Blocked by ",
              ...dependency.flatMap((item, index) => [
                index === 0 ? null : ", ",
                element("a", { href: githubUrl(item.url), target: "_blank", rel: "noreferrer" }, [`#${item.number}`]),
              ]),
            ]);
      list.append(
        element("li", {}, [
          element("p", { class: "eyebrow" }, [group]),
          element("div", { class: "work-row" }, [
            element("a", { href: githubUrl(lane.issue.url), target: "_blank", rel: "noreferrer" }, [`#${lane.issue.number} ${lane.issue.title}`]),
            badge(lane.phase),
          ]),
          dependencyText,
        ]),
      );
    }
  }
  if (list.childElementCount === 0) list.append(element("li", { class: "unknown" }, ["Current sequencing is Unknown."]));
  section.append(list, sourceLinks(projection.currentWork.sourceRefs));
  return section;
}

function recentMerges(projection: RepositoryProjection): HTMLElement {
  const section = element("section", { class: "panel", "aria-labelledby": "recent-title" }, [
    element("div", { class: "section-heading" }, [
      element("div", {}, [element("p", { class: "eyebrow" }, ["Bounded historical context"]), element("h2", { id: "recent-title" }, ["Recent merges"])]),
      badge("historical"),
    ]),
  ]);
  const list = element("ol", { class: "activity-list" });
  if (projection.recentCompletions.length === 0) {
    list.append(element("li", { class: "unknown" }, ["Recent completion data is Unknown or empty."]));
  } else {
    for (const completion of projection.recentCompletions) {
      list.append(
        element("li", {}, [
          element("div", {}, [
            element("a", { href: githubUrl(completion.url), target: "_blank", rel: "noreferrer" }, [`#${completion.number} · ${completion.title}`]),
            element("p", { class: "quiet" }, [`Merged by ${completion.author} · `, element("time", { datetime: completion.mergedAt }, [new Date(completion.mergedAt).toLocaleString()])]),
          ]),
          element("code", {}, [shortIdentity(completion.mergeSha)]),
          sourceLinks(completion.sourceRefs),
        ]),
      );
    }
  }
  section.append(list);
  return section;
}

function authorityAttention(projection: RepositoryProjection): HTMLElement {
  const state = projection.attention.humanActionRequired;
  const section = element("section", { class: "panel attention-panel", "aria-labelledby": "attention-title" }, [
    element("div", { class: "section-heading" }, [
      element("div", {}, [element("p", { class: "eyebrow" }, ["Authority · blockers · action ownership"]), element("h2", { id: "attention-title" }, ["Authority & attention"])]),
      state === null ? badge("unknown") : state ? badge("human required") : badge("no human action"),
    ]),
  ]);
  const reasons = element("ul", { class: "attention-list" });
  for (const reason of projection.attention.reasons) reasons.append(element("li", {}, [reason]));
  for (const lane of projection.deliveries) {
    for (const blocker of lane.blockers) {
      reasons.append(element("li", {}, [
        element("a", { href: githubUrl(lane.issue.url), target: "_blank", rel: "noreferrer" }, [`#${lane.issue.number}`]),
        ` · ${blocker} · action: ${label(lane.action.owner)}`,
      ]));
    }
  }
  if (reasons.childElementCount === 0) reasons.append(element("li", { class: "quiet" }, ["No human or Steward action is evidenced by the current complete observation."]));
  const legend = element("div", { class: "provenance-legend", role: "group", "aria-label": "Provenance classes" }, [
    element("span", { class: "source source--direct" }, ["Direct fact"]),
    element("span", { class: "source source--derived" }, ["Derived projection"]),
    element("span", { class: "source source--heuristic" }, ["Heuristic / advisory"]),
    element("span", { class: "source source--historical" }, ["Historical only"]),
  ]);
  section.append(reasons, legend, sourceLinks(projection.attention.sourceRefs));
  return section;
}

function announce(message: string): void {
  const status = document.querySelector<HTMLDivElement>("#dashboard-status");
  if (status !== null) status.textContent = message;
}

function render(projection: RepositoryProjection, restoreRefreshFocus = false): void {
  document.title = `${projection.repo.name} · Live Control Room`;
  const app = document.querySelector<HTMLDivElement>("#app");
  if (app === null) throw new Error("Dashboard root is missing");
  const refresh = element("button", { type: "button", class: "cyber-btn cyber-btn--ghost" }, ["Refresh projection"]);
  refresh.addEventListener("click", () => void load(true));
  const header = element("header", { class: "hero shell" }, [
    element("div", {}, [
      element("p", { class: "eyebrow" }, ["READ-ONLY / SOURCE-LINKED / REPOSITORY-LOCAL"]),
      element("h1", {}, ["Tachiko Work / Live Control Room"]),
      element("p", { class: "hero-copy" }, ["A replaceable operational projection. GitHub and repository authority remain canonical."]),
    ]),
    element("div", { class: "hero-actions" }, [
      element("p", { class: "quiet" }, ["Observed ", element("time", { datetime: projection.repo.observedAt }, [new Date(projection.repo.observedAt).toLocaleString()])]),
      refresh,
    ]),
  ]);
  const main = element("main", { id: "dashboard", class: "shell dashboard", tabindex: "-1" }, [
    executiveStrip(projection),
    deliveryCenter(projection),
    currentWork(projection),
    recentMerges(projection),
    authorityAttention(projection),
  ]);
  const footer = element("footer", { class: "shell footer" }, [
    "No write, merge, scheduling, dispatch, or agent-control capability is exposed by this dashboard.",
    " ",
    element("a", { href: "/THIRD_PARTY_LICENSES.txt" }, ["Third-party notices"]),
    ".",
  ]);
  app.replaceChildren(header, main, footer);
  if (restoreRefreshFocus) {
    refresh.focus();
    announce("Repository projection refreshed.");
  }
}

function renderFailure(restoreRetryFocus = false): void {
  const app = document.querySelector<HTMLDivElement>("#app");
  if (app === null) return;
  app.replaceChildren(
    element("main", { class: "shell fatal-state" }, [
      element("p", { class: "eyebrow" }, ["FETCH HEALTH / UNAVAILABLE"]),
      element("h1", {}, ["Repository projection is Unknown"]),
      element("p", {}, ["The localhost server could not provide a complete normalized projection. No healthy state has been inferred."]),
      element("button", { type: "button", class: "cyber-btn cyber-btn--ghost" }, ["Retry read-only fetch"]),
    ]),
  );
  const retry = app.querySelector<HTMLButtonElement>("button");
  retry?.addEventListener("click", () => void load(true));
  if (restoreRetryFocus) retry?.focus();
  announce("Repository projection is unavailable.");
}

async function load(restoreActionFocus = false): Promise<void> {
  const app = document.querySelector<HTMLDivElement>("#app");
  app?.setAttribute("aria-busy", "true");
  try {
    const response = await fetch("/api/projection", { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error("Projection endpoint unavailable");
    render((await response.json()) as RepositoryProjection, restoreActionFocus);
  } catch {
    renderFailure(restoreActionFocus);
  } finally {
    app?.setAttribute("aria-busy", "false");
  }
}

void load();
