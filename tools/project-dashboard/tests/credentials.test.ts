import { describe, expect, it, vi } from "vitest";

import { githubToken } from "../src/server/credentials.ts";

describe("githubToken", () => {
  it("uses a nonblank GH_TOKEN when GITHUB_TOKEN is blank", async () => {
    const fallback = vi.fn(async () => "cli-token");

    await expect(githubToken({ GITHUB_TOKEN: "  ", GH_TOKEN: " gh-token " }, fallback)).resolves.toBe("gh-token");
    expect(fallback).not.toHaveBeenCalled();
  });

  it("prefers a nonblank GITHUB_TOKEN and falls back to gh auth only when both variables are blank", async () => {
    const fallback = vi.fn(async () => " cli-token ");

    await expect(githubToken({ GITHUB_TOKEN: " github-token ", GH_TOKEN: "gh-token" }, fallback)).resolves.toBe("github-token");
    await expect(githubToken({ GITHUB_TOKEN: "", GH_TOKEN: "\t" }, fallback)).resolves.toBe("cli-token");
    expect(fallback).toHaveBeenCalledOnce();
  });

  it("rejects a blank CLI fallback token", async () => {
    await expect(githubToken({}, async () => "  \n")).rejects.toThrow("No GitHub read credential is available");
  });
});
