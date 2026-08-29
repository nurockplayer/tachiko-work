import { describe, expect, it } from "vitest";

import { createDurabilityState } from "../src/durability-state.ts";

describe("Designer durability state", () => {
  it("becomes dirty after an edit and clean only for the exact published revision", () => {
    const state = createDurabilityState();
    state.install("resident/0", true);
    expect(state.snapshot().dirty).toBe(false);

    state.observe("resident/1");
    expect(state.snapshot()).toEqual({
      current_revision: "resident/1",
      durable_revision: "resident/0",
      dirty: true,
    });

    state.published("resident/1");
    expect(state.snapshot().dirty).toBe(false);
  });

  it("never marks a later revision durable when publication was bound to an earlier one", () => {
    const state = createDurabilityState();
    state.install("resident/0", false);

    state.observe("resident/1");
    state.observe("resident/2");
    state.published("resident/1");

    expect(state.snapshot()).toEqual({
      current_revision: "resident/2",
      durable_revision: "resident/1",
      dirty: true,
    });
  });

  it("starts opened host projects clean and destroys occurrence state on close", () => {
    const state = createDurabilityState();
    state.install("resident/0", true);
    expect(state.snapshot().dirty).toBe(false);

    state.close();
    expect(state.snapshot()).toEqual({
      current_revision: null,
      durable_revision: null,
      dirty: false,
    });
  });
});
