import { describe, expect, it } from "vitest";

import { readServerCredential } from "../src/server/github.js";

describe("readServerCredential", () => {
  it("prefers trimmed GITHUB_TOKEN", () => {
    expect(readServerCredential({
      GITHUB_TOKEN: "  primary  ",
      GH_TOKEN: "secondary",
    })).toBe("primary");
  });

  it("falls back and treats blanks as missing", () => {
    expect(readServerCredential({ GH_TOKEN: " fallback " })).toBe("fallback");
    expect(readServerCredential({
      GITHUB_TOKEN: "   ",
      GH_TOKEN: " fallback ",
    })).toBe("fallback");
    expect(readServerCredential({ GITHUB_TOKEN: "   " })).toBeUndefined();
  });
});
