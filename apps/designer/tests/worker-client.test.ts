import { afterEach, describe, expect, it, vi } from "vitest";

import { freshOccurrenceId } from "../src/runtime/worker-client.ts";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Designer occurrence identity", () => {
  it("uses the platform UUID generator when available", () => {
    vi.stubGlobal("crypto", {
      randomUUID: () => "123e4567-e89b-42d3-a456-426614174000",
    });

    expect(freshOccurrenceId()).toBe("123e4567-e89b-42d3-a456-426614174000");
  });

  it("builds a lowercase canonical UUID v4 from cryptographic bytes", () => {
    vi.stubGlobal("crypto", {
      getRandomValues: (bytes: Uint8Array) => {
        bytes.set(Array.from({ length: 16 }, (_, index) => index));
        return bytes;
      },
    });

    expect(freshOccurrenceId()).toBe("00010203-0405-4607-8809-0a0b0c0d0e0f");
  });
});
