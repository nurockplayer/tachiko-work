import { startDashboard, type DashboardOptions } from "./server/application.js";
import { pathToFileURL } from "node:url";

type Environment = Readonly<Record<string, string | undefined>>;

function required(environment: Environment, name: string): string {
  const value = environment[name];
  if (value === undefined || value.trim() === "") throw new Error(`${name} is required`);
  return value;
}

function port(environment: Environment): number {
  const value = environment.DASHBOARD_PORT;
  if (value === undefined) return 4311;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > 65_535) {
    throw new Error("DASHBOARD_PORT must be an integer from 1 through 65535");
  }
  return parsed;
}

/** Server-only runtime configuration; neither it nor its token reaches the browser. */
export function dashboardOptionsFromEnvironment(environment: Environment = process.env): DashboardOptions {
  const trustedStewardLogins = required(environment, "DASHBOARD_TRUSTED_STEWARD_LOGINS")
    .split(",")
    .map(login => login.trim())
    .filter(login => login !== "");
  if (trustedStewardLogins.length === 0) throw new Error("DASHBOARD_TRUSTED_STEWARD_LOGINS must name at least one login");
  return {
    repository: required(environment, "DASHBOARD_REPOSITORY"),
    token: required(environment, "DASHBOARD_GITHUB_TOKEN"),
    trustedStewardLogins,
    port: port(environment),
  };
}

export async function runDashboard(environment: Environment = process.env): Promise<void> {
  const dashboard = await startDashboard(dashboardOptionsFromEnvironment(environment));
  process.stdout.write(`Operational Dashboard listening at ${dashboard.origin}\n`);
  await new Promise<void>((resolve, reject) => {
    let closing = false;
    const close = () => {
      if (closing) return;
      closing = true;
      void dashboard.close().then(resolve, reject);
    };
    process.once("SIGINT", close);
    process.once("SIGTERM", close);
  });
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  void runDashboard().catch(error => {
    process.stderr.write(`${error instanceof Error ? error.message : "Dashboard startup failed"}\n`);
    process.exitCode = 1;
  });
}
