import type { Availability, DashboardProjection, ObservationSnapshot } from "./observation.js";

/** Repository-local integration seam, not a public product API. */
export interface DashboardOptions {
  repository: string;
  token: string;
  trustedStewardLogins: readonly string[];
  /** Server-only injection of GitHub HTTP transport; never browser configuration. */
  fetch?: typeof globalThis.fetch;
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

export interface RunningDashboard {
  origin: string;
  close(): Promise<void>;
}

export const OPERATIONAL_STUB = "#229 operational Dashboard deliberate acceptance stub";

/** Must observe GitHub, not accept an already-normalized projection as its input. */
export async function observeGitHub(_options: DashboardOptions): Promise<ObservationSnapshot> {
  throw new Error(OPERATIONAL_STUB);
}

/** The real HTTP/application boundary used by the eventual CLI and browser tests. */
export async function startDashboard(_options: DashboardOptions): Promise<RunningDashboard> {
  throw new Error(OPERATIONAL_STUB);
}
