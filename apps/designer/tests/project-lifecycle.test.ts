import { describe, expect, it, vi } from "vitest";

import { createProjectLifecycle } from "../src/project-lifecycle.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type { OpenedProjection } from "../src/runtime/protocol.ts";

const opened: OpenedProjection = {
  bootstrap: {
    title: "Weapons",
    revision: "resident/1",
    default_collection: "weapons",
    collections: [{ id: "weapons", key: "weapons", entity_count: 0 }],
  },
  table: {
    revision: "resident/1",
    collection: { id: "weapons", key: "weapons", entity_count: 0 },
    columns: [],
    rows: [],
  },
};

function lifecycleFixture(client: Partial<DesignerClient> = {}) {
  let busy = false;
  const trace: string[] = [];
  const host: DesignerProjectHost = {
    list: async () => [],
    read: async () => new Uint8Array([7, 8]).buffer,
    readSnapshot: async () => ({ bytes: new Uint8Array([7, 8]).buffer, presentation: "{}" }),
    publish: async () => {},
  };
  const lifecycle = createProjectLifecycle(client as DesignerClient, host, {
    isBusy: () => busy,
    setBusy: (next) => { busy = next; },
    render: () => {},
    clearNotice: () => {},
    showFailure: () => { trace.push("failure"); },
    showSuccess: () => { trace.push("success"); },
    hasPendingScalarDrafts: () => false,
    hasEditDrafts: () => false,
    confirmDiscard: () => true,
    confirmLocalDiscard: async () => true,
    isDestroyed: () => false,
    installOpenedOccurrence: () => { trace.push("install"); },
    installClosedOccurrence: () => { trace.push("closed"); },
    selectedSavedProject: () => "baseline.roproj",
    readOpenedPresentation: async () => { trace.push("inspect"); },
    installOpenedPresentation: async () => { trace.push("presentation"); },
    revision: () => "resident/1",
    presentation: () => "{}",
    suggestedProjectName: () => "weapons.roproj",
    savedPresentation: () => {},
    refreshSavedProjects: async () => {},
    describeProject: () => "project",
    syncBeforeUnloadGuard: () => {},
  });
  return { lifecycle, trace };
}

describe("project lifecycle owner", () => {
  it("admits a saved snapshot in the runtime before installing it in the shell", async () => {
    let trace: string[] = [];
    const fixture = lifecycleFixture({
      openProject: async () => {
        expect(trace).toEqual(["inspect"]);
        return opened;
      },
    });
    const { lifecycle } = fixture;
    trace = fixture.trace;

    await lifecycle.openSavedProject();

    expect(trace).toEqual(["inspect", "install", "presentation", "success"]);
    expect(lifecycle.durability.snapshot()).toEqual({
      current_revision: "resident/1",
      durable_revision: "resident/1",
      dirty: false,
    });
  });

  it("keeps the existing occurrence when runtime open rejects", async () => {
    const { lifecycle, trace } = lifecycleFixture({
      openProject: async () => { throw new Error("corrupt project"); },
    });
    lifecycle.replaceOccurrence(opened, true);
    trace.length = 0;

    await lifecycle.openSavedProject();

    expect(trace).toEqual(["inspect", "failure"]);
    expect(lifecycle.durability.snapshot()).toEqual({
      current_revision: "resident/1",
      durable_revision: "resident/1",
      dirty: false,
    });
  });

  it("does not clear shell state when runtime close rejects", async () => {
    const { lifecycle, trace } = lifecycleFixture({
      closeProject: vi.fn(async () => { throw new Error("close rejected"); }),
    });
    lifecycle.replaceOccurrence(opened, true);
    trace.length = 0;

    await lifecycle.close();

    expect(trace).toEqual(["failure"]);
    expect(lifecycle.durability.snapshot()).toEqual({
      current_revision: "resident/1",
      durable_revision: "resident/1",
      dirty: false,
    });
  });
});
