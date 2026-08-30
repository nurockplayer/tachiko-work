import { fixtureProjection } from "./fixture.ts";
import { githubToken } from "./credentials.ts";
import { GithubApiClient, loadGithubSnapshot } from "./github.ts";
import { normalizeRepositorySnapshot } from "./normalize.ts";
import { createDashboardServer } from "./server.ts";
import type { RepositoryProjection } from "../shared/types.ts";

const cacheDurationMs = 30_000;

function liveProjectionLoader(): () => Promise<RepositoryProjection> {
  let cached: { value: RepositoryProjection; expiresAt: number } | null = null;
  let pending: Promise<RepositoryProjection> | null = null;
  return async () => {
    if (cached !== null && cached.expiresAt > Date.now()) return cached.value;
    if (pending !== null) return pending;
    pending = (async () => {
      const token = await githubToken();
      const snapshot = await loadGithubSnapshot(new GithubApiClient(token), {
        owner: "nurockplayer",
        repo: "tachiko-work",
      });
      const value = normalizeRepositorySnapshot(snapshot);
      cached = { value, expiresAt: Date.now() + cacheDurationMs };
      return value;
    })();
    try {
      return await pending;
    } finally {
      pending = null;
    }
  };
}

const loadProjection = process.env.DASHBOARD_FIXTURE === "pressure-tests"
  ? () => Promise.resolve(fixtureProjection())
  : liveProjectionLoader();
const server = createDashboardServer({
  loadProjection,
  assetRoot: new URL("../../dist/", import.meta.url),
});
const port = Number.parseInt(process.env.PORT ?? "4178", 10);

server.listen(Number.isFinite(port) ? port : 4178, "127.0.0.1", () => {
  process.stdout.write(`Tachiko Work read-only dashboard: http://127.0.0.1:${Number.isFinite(port) ? port : 4178}\n`);
});
