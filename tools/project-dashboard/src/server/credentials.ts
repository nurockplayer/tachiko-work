import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

type GithubTokenEnvironment = {
  GITHUB_TOKEN?: string;
  GH_TOKEN?: string;
};

async function cliGithubToken(): Promise<string> {
  const { stdout } = await execFileAsync("gh", ["auth", "token"], {
    encoding: "utf8",
    windowsHide: true,
  });
  return stdout;
}

export async function githubToken(
  environment: GithubTokenEnvironment = process.env,
  fallback: () => Promise<string> = cliGithubToken,
): Promise<string> {
  const environmentToken = [environment.GITHUB_TOKEN, environment.GH_TOKEN]
    .map((value) => value?.trim())
    .find((value): value is string => value !== undefined && value !== "");
  if (environmentToken !== undefined) return environmentToken;
  const token = (await fallback()).trim();
  if (token === "") throw new Error("No GitHub read credential is available");
  return token;
}
