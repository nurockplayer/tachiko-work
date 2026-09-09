import worker from "../../src/cloudflare/worker.js";
import { fixture } from "../operational/github-fixture.js";

const remote = fixture();
remote.data.comments[0]!.html_url = "javascript:window.__sourceLinkExecuted=true";

const browserFixtureWorker = {
  async fetch(request: Request, environment: Readonly<Record<string, unknown>>, context: unknown) {
    const url = new URL(request.url);
    if (url.pathname === "/__test__/set-safe") {
      remote.data.comments[0]!.html_url = "https://github.com/acceptance-fixture/dashboard/pull/322#issuecomment-700";
      return new Response(null, { status: 204 });
    }
    if (url.pathname === "/__test__/set-quote") {
      remote.data.comments[0]!.html_url = 'https://github.com/" data-injected="yes';
      return new Response(null, { status: 204 });
    }
    if (url.pathname === "/api/projection") {
      // Workerd isolates do not accept a Node fetch override, so this fixture
      // supplies deterministic projection data while production Worker routing
      // still serves the browser asset and handles all non-fixture requests.
      const sourceUrl = remote.data.comments[0]!.html_url;
      const envelope = {
        repository: remote.repository,
        projection: {
          executive: { mainSha: { availability: "complete", value: "1".repeat(40) } },
          deliveries: [],
          criticalPath: { availability: "complete", issueNumbers: [] },
          recentActivity: { availability: "complete", value: [] },
          attention: {
            items: [{ kind: "steward-hold", prNumber: 322, sourceUrl }],
          },
        },
        sources: {
          main: { availability: "complete", url: `${remote.web}/commit/${"1".repeat(40)}` },
          issues: { availability: "complete", url: `${remote.web}/issues` },
          pullRequests: { availability: "complete", url: `${remote.web}/pulls` },
          recentActivity: { availability: "complete", url: `${remote.web}/pulls?state=closed` },
        },
      };
      return new Response(JSON.stringify(envelope), {
        headers: { "content-type": "application/json", "cache-control": "no-store" },
      });
    }
    return worker.fetch(request, environment, context);
  },
};

export default browserFixtureWorker;
