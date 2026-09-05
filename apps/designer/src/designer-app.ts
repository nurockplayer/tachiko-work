import { reconcileTextEdit, normalizeLineEndings } from "./text-edit.ts";
import { TrackerGrid } from "./tracker-grid.ts";
import { defaultBudgetViews, addBudgetView, duplicateBudgetView, renameBudgetView, reorderBudgetViews, deleteBudgetView } from "./budget-views.ts";
import { mountBudgetTools, hasBudgetToolsDraft, type BudgetToolsDraft } from "./budget-tools.ts";
import { parseTrackerView, emptyTrackerView, cellKey, type NumberFormat, type TrackerView } from "./tracker-model.ts";
import { createProjectionStore, type ProjectionStore } from "./projection-store.ts";
import { createDurabilityState } from "./durability-state.ts";
import type {
  DesignerProjectHost,
  SavedProjectSummary,
} from "./host/browser-project-host.ts";
import { projectTransferFromFiles } from "./host/project-transfer.ts";
import {
  DesignerRuntimeError,
  type DesignerClient,
} from "./runtime/client.ts";
import type {
  BootstrapProjection,
  DiagnosticProjection,
  FieldProjection,
  FieldTarget,
  OpenedProjection,
  PublicationProjection,
  TableProjection,
} from "./runtime/protocol.ts";

type Notice = {
  tone: "error" | "success";
  title: string;
  message: string;
  diagnostics: DiagnosticProjection[];
};

export type MountedDesigner = {
  ready: Promise<void>;
  destroy(): void;
};

export function mountDesigner(
  root: HTMLElement,
  client: DesignerClient,
  host: DesignerProjectHost,
): MountedDesigner {
  let activeProject: {name: string; bytes: ArrayBuffer; presentation?: string | undefined} | null = null;
  let savedView = JSON.stringify(emptyTrackerView());
  const tracker = new TrackerGrid({
    command: async (request) => {
      if (!client.trackerCommand || !store || busy) throw new Error("Tracker is unavailable.");
      busy = true; notice = null; render();
      let published = false;
      try {
        const publication = await client.trackerCommand(request);
        published = true;
        store.beginPublication(publication);
        durability.observe(publication.resulting_revision);
        const table = await client.queryTable(selectedCollection);
        if (table.revision !== publication.resulting_revision) throw new Error("Tracker refresh is not current.");
        store = createProjectionStore(table);
        if (bootstrap) bootstrap = {...bootstrap, revision: table.revision, collections: bootstrap.collections.map(c => c.key === table.collection.key ? table.collection : c)};
      } catch (error) { showFailure(error, published); if (!published) throw error; }
      finally { busy = false; syncBeforeUnloadGuard(); render(); }
    },
    changed: () => { if (store?.snapshot().currentness === "current") notice = null; reflectUnsavedState(); },
    failed: error => { showProjectFailure("Tracker operation not completed", error); render(); },
    render: () => { render(); },
  });
  const viewDirty = (): boolean => JSON.stringify(tracker.view) !== savedView;
  let bootstrap: BootstrapProjection | null = null;
  let store: ProjectionStore | null = null;
  let selectedCollection = "";
  let notice: Notice | null = null;
  let startupFailure: string | null = null;
  let busy = false;
  let destroyed = false;
  let occurrenceClosed = false;
  const pendingTextBuffers = new Map<string, string>();
  const pendingBooleanBuffers = new Map<string, boolean>();
  const pendingDateBuffers = new Map<string, string>();
  const pendingFormulaBuffers = new Map<string, string>();
  const pendingNumberBuffers = new Map<string, string>();
  let budgetToolsDraft: BudgetToolsDraft = {};
  let budgetTables: TableProjection[] = [];
  const hasEditDrafts = (): boolean => tracker.pending || pendingTextBuffers.size > 0 || pendingBooleanBuffers.size > 0 || pendingDateBuffers.size > 0 || pendingFormulaBuffers.size > 0 || pendingNumberBuffers.size > 0 || hasBudgetToolsDraft(budgetToolsDraft);
  const hasPendingScalarDrafts = (): boolean =>
    hasEditDrafts() || viewDirty() ||
    pendingTextBuffers.size > 0 ||
    pendingBooleanBuffers.size > 0 ||
    pendingDateBuffers.size > 0 ||
    pendingFormulaBuffers.size > 0;
  let savedProjects: SavedProjectSummary[] = [];
  let selectedSavedProject = "";
  const durability = createDurabilityState();
  let beforeUnloadGuarded = false;

  const warnBeforeDirtyUnload = (event: BeforeUnloadEvent): void => {
    event.preventDefault();
    Reflect.set(event, "returnValue", true);
  };

  const syncBeforeUnloadGuard = (): void => {
    const shouldGuard =
      !destroyed && (durability.snapshot().dirty || hasPendingScalarDrafts());
    if (shouldGuard && !beforeUnloadGuarded) {
      window.addEventListener("beforeunload", warnBeforeDirtyUnload);
      beforeUnloadGuarded = true;
    } else if (!shouldGuard && beforeUnloadGuarded) {
      window.removeEventListener("beforeunload", warnBeforeDirtyUnload);
      beforeUnloadGuarded = false;
    }
  };

  const reflectUnsavedState = (): void => {
    syncBeforeUnloadGuard();
    const durabilityChip = root.querySelector<HTMLElement>('[data-testid="durability"]');
    if (durabilityChip !== null) {
      const dirty = durability.snapshot().dirty || hasPendingScalarDrafts();
      durabilityChip.dataset.dirty = String(dirty);
      const label = durabilityChip.querySelector("span");
      if (label !== null) label.textContent = dirty ? "Unsaved changes" : "Saved";
    }
  };

  const render = (): void => {
    if (destroyed) return;
    if (bootstrap === null || store === null) {
      if (occurrenceClosed) {
        root.innerHTML = closedMarkup(savedProjects, selectedSavedProject, busy, notice);
        bindInteractions();
      } else {
        root.innerHTML =
          startupFailure === null ? loadingMarkup() : startupFailureMarkup(startupFailure);
      }
      return;
    }
    const snapshot = store.snapshot();
    root.innerHTML = designerMarkup(
      bootstrap,
      snapshot.table,
      snapshot.currentness,
      selectedCollection,
      notice,
      busy,
      durability.snapshot().dirty || hasPendingScalarDrafts(),
      durability.snapshot().durable_revision,
      savedProjects,
      selectedSavedProject,
      tracker.view,
    );
    if (snapshot.table.tracker_profile === true) {
      const workbench = root.querySelector(".table-workbench");
      if (workbench) workbench.innerHTML = `${noticeMarkup(notice)}${snapshot.currentness === "refresh_failed" ? '<button data-tracker-refresh>Retry refresh</button>' : ""}${tracker.markup(snapshot.table, busy || snapshot.currentness !== "current")}`;
      tracker.bind(root, busy || snapshot.currentness !== "current");
      root.querySelector("[data-tracker-refresh]")?.addEventListener("click", () => { void selectCollection(selectedCollection); });
    }
    if (tracker.view.budgetViews) {
      renderBudgetViews();
      if (hasEditDrafts()) {
        const cancel = document.createElement("button");
        cancel.textContent = "Cancel pending Budget edits"; cancel.disabled = busy;
        cancel.addEventListener("click", () => {
          pendingTextBuffers.clear(); pendingBooleanBuffers.clear(); pendingDateBuffers.clear(); pendingNumberBuffers.clear(); pendingFormulaBuffers.clear(); budgetToolsDraft = {};
          reflectUnsavedState(); render();
        });
        root.querySelector(".workspace-actions")?.append(cancel);
      }
      const panel = document.createElement("div");
      root.querySelector(".table-workbench")?.append(panel);
      mountBudgetTools(panel, {
        tables: budgetTables,
        currentCollection: snapshot.table.collection.id,
        disabled: busy || snapshot.currentness !== "current" || budgetTables.some(t => t.revision !== snapshot.table.revision),
        draft: budgetToolsDraft,
        changed: reflectUnsavedState,
        updateFormula: async (target, source) => {
          const before = store?.snapshot().table.revision;
          await commitFormula(target, source);
          if (store?.snapshot().table.revision === before) throw new Error("Formula was not published. Correct the input and try again.");
        },
        copyFormula: async request => {
          const copy = client.copyFormula?.bind(client);
          if (!copy) throw new Error("Formula copying is unavailable.");
          const before = store?.snapshot().table.revision;
          await commitScalar(request.source, revision => copy(revision, request));
          if (store?.snapshot().table.revision === before) throw new Error("Copy was not published. Check the selected range and references.");
        },
      });
    }
    hydrateDraftControls();
    bindInteractions();
  };

  const showFailure = (error: unknown, published: boolean): void => {
    const failure =
      error instanceof DesignerRuntimeError
        ? error.failure
        : {
            message: error instanceof Error ? error.message : String(error),
            diagnostics: [],
          };
    notice = {
      tone: "error",
      title: published ? "Edit published; refresh incomplete" : "Edit not published",
      message: failure.message,
      diagnostics: failure.diagnostics,
    };
    if (published) {
      store?.failRefresh(failure.message);
    }
  };

  const commitScalar = async (
    target: FieldTarget,
    publish: (expectedRevision: string) => Promise<PublicationProjection>,
    onPublished?: () => void,
  ): Promise<void> => {
    if (store === null || busy || store.snapshot().currentness !== "current") return;
    busy = true;
    notice = null;
    render();
    let published = false;
    try {
      const publication = await publish(store.snapshot().table.revision);
      published = true;
      // Rust clears its session history for this accepted generic publication.
      // Invalidate the matching UI history before any fallible refresh work.
      tracker.invalidateHistory();
      onPublished?.();
      const requested = store.beginPublication(publication);
      durability.observe(publication.resulting_revision);
      syncBeforeUnloadGuard();
      render();
      const refresh = await client.queryFields(
        publication.resulting_revision,
        requested,
      );
      store.finishRefresh(refresh);
      if (tracker.view.budgetViews) await refreshBudgetTables(publication.resulting_revision);
      notice = {
        tone: "success",
        title: "Publication complete",
        message: `${humanize(target.field)} published. Dependent calculations are current.`,
        diagnostics: [],
      };
    } catch (error) {
      showFailure(error, published);
    } finally {
      busy = false;
      render();
    }
  };

  const commitNumber = async (target: FieldTarget, input: string): Promise<void> => {
    await commitScalar(target, (expectedRevision) => client.editNumber(expectedRevision, target, input), () => pendingNumberBuffers.delete(textBufferKey(target)));
    pendingNumberBuffers.delete(textBufferKey(target));
    syncBeforeUnloadGuard(); render();
  };

  const commitText = (target: FieldTarget, value: string): Promise<void> =>
    commitScalar(
      target,
      (expectedRevision) => client.editText(expectedRevision, target, value),
      () => {
        pendingTextBuffers.delete(textBufferKey(target));
        syncBeforeUnloadGuard();
      },
    );

  const commitBoolean = (target: FieldTarget, value: boolean): Promise<void> =>
    commitScalar(
      target,
      (expectedRevision) => client.editBoolean(expectedRevision, target, value),
      () => {
        pendingBooleanBuffers.delete(textBufferKey(target));
        syncBeforeUnloadGuard();
      },
    );

  const commitDate = (target: FieldTarget, value: string): Promise<void> =>
    commitScalar(
      target,
      (expectedRevision) => client.editDate(expectedRevision, target, value),
      () => {
        pendingDateBuffers.delete(textBufferKey(target));
        syncBeforeUnloadGuard();
      },
    );

  const commitFormula = (target: FieldTarget, source: string): Promise<void> => {
    if (!client.updateFormula) return Promise.reject(new Error("Formula authoring is unavailable."));
    return commitScalar(
      target,
      (expectedRevision) =>
        client.updateFormula?.(expectedRevision, target, source) ??
        Promise.reject(new Error("Formula authoring is unavailable.")),
      () => {
        pendingFormulaBuffers.delete(textBufferKey(target));
        syncBeforeUnloadGuard();
      },
    );
  };

  const selectCollection = async (collection: string): Promise<void> => {
    if (bootstrap === null || store === null || busy) return;
    busy = true;
    notice = null;
    render();
    try {
      const expectedRevision = store.snapshot().table.revision;
      const table = await client.queryTable(collection);
      if (table.revision !== expectedRevision) {
        throw new Error("Collection query returned a different semantic revision.");
      }
      store = createProjectionStore(table);
      selectedCollection = collection;
      if (tracker.view.budgetViews) {
        await refreshBudgetTables(expectedRevision);
        const matching = tracker.view.budgetViews.views.find(v => v.collection === table.collection.id);
        if (matching) tracker.view.budgetViews.active = matching.id;
      }
    } catch (error) {
      showFailure(error, false);
    } finally {
      busy = false;
      render();
    }
  };

  const refreshBudgetTables = async (revision: string): Promise<void> => {
    if (!bootstrap) return;
    const tables = await Promise.all(bootstrap.collections.map(c => client.queryTable(c.key)));
    if (tables.some(t => t.revision !== revision)) throw new Error("Budget projections are not current. Retry refresh.");
    budgetTables = tables;
  };

  const renderBudgetViews = (): void => {
    const views = tracker.view.budgetViews;
    if (!views || !bootstrap) return;
    const rail = root.querySelector(".collection-rail");
    if (!rail) return;
    const section = document.createElement("section");
    section.innerHTML = `<h3>Budget views</h3><p>Views share the same data. Duplicating or deleting a view keeps its source data.</p>
      <label>View <select aria-label="View" data-budget-view ${busy ? "disabled" : ""}>${views.views.map(v => `<option value="${escapeHtml(v.id)}" ${v.id === views.active ? "selected" : ""}>${escapeHtml(v.name)}</option>`).join("")}</select></label>
      <button data-view-action="add">Add view</button><button data-view-action="duplicate">Duplicate view</button><button data-view-action="rename">Rename view</button><button data-view-action="up">Move view up</button><button data-view-action="down">Move view down</button><button data-view-action="delete">Delete view</button>
      <p>Number input uses decimal dots, without grouping. Percentage: 0.2 means 20%. JPY and USD change display only; no currency conversion. Dates are Gregorian YYYY-MM-DD, without time or timezone.</p>`;
    rail.append(section);
    const heading = root.querySelector("#table-title");
    const active = views.views.find(v => v.id === views.active);
    if (heading && active && bootstrap.collections.find(c => c.id === active.collection)?.key === selectedCollection) heading.textContent = active.name;
    const selectView = async (id: string): Promise<void> => {
      const view = tracker.view.budgetViews?.views.find(v => v.id === id);
      const collection = bootstrap?.collections.find(c => c.id === view?.collection);
      if (!view || !collection) throw new Error("This view's source is unavailable.");
      await selectCollection(collection.key);
      if (selectedCollection !== collection.key) return;
      if (tracker.view.budgetViews) tracker.view.budgetViews.active = id;
      reflectUnsavedState(); render();
    };
    section.querySelector<HTMLSelectElement>("select")?.addEventListener("change", event => {
      void selectView((event.target as HTMLSelectElement).value).catch((error: unknown) => { showProjectFailure("View not opened", error); render(); });
    });
    section.querySelectorAll<HTMLButtonElement>("button").forEach(button => {
      button.disabled = busy;
      button.addEventListener("click", () => {
        try {
          let next = tracker.view.budgetViews;
          if (!next || !bootstrap) return;
          const ids = bootstrap.collections.map(c => c.id);
          const action = button.dataset.viewAction;
          if (action === "add" || action === "duplicate" || action === "rename") {
            const name = window.prompt("View name:", active?.name ?? "Budget view");
            if (name === null) return;
            if (action === "add") {
              const collection = bootstrap.collections.find(c => c.key === selectedCollection);
              if (!collection) return;
              next = addBudgetView(next, {id: crypto.randomUUID(), name, collection: collection.id}, ids);
            } else if (action === "duplicate") next = duplicateBudgetView(next, next.active, crypto.randomUUID(), name, ids);
            else next = renameBudgetView(next, next.active, name);
          } else if (action === "delete") next = deleteBudgetView(next, next.active);
          else {
            const order = next.views.map(v => v.id);
            const from = order.indexOf(next.active);
            const to = from + (action === "up" ? -1 : 1);
            if (to < 0 || to >= order.length) return;
            const sourceId = order[from], destinationId = order[to];
            if (sourceId === undefined || destinationId === undefined) return;
            [order[from], order[to]] = [destinationId, sourceId];
            next = reorderBudgetViews(next, order);
          }
          tracker.view.budgetViews = next;
          void selectView(next.active).catch((error: unknown) => { showProjectFailure("View not opened", error); render(); });
        } catch (error) { showProjectFailure("View not changed", error); render(); }
      });
    });
  };

  const installOccurrence = async (
    candidate: BootstrapProjection,
    durable: boolean,
  ): Promise<void> => {
    const table = await client.queryTable(candidate.default_collection);
    if (table.revision !== candidate.revision) {
      throw new Error("Initial projection does not match the bootstrap revision.");
    }
    const nextStore = createProjectionStore(table);
    tracker.reset(); savedView = JSON.stringify(tracker.view); activeProject = null;
    pendingTextBuffers.clear();
    pendingBooleanBuffers.clear();
    pendingDateBuffers.clear();
    pendingFormulaBuffers.clear();
    pendingNumberBuffers.clear(); budgetToolsDraft = {}; budgetTables = [];
    bootstrap = candidate;
    store = nextStore;
    selectedCollection = candidate.default_collection;
    occurrenceClosed = false;
    durability.install(candidate.revision, durable);
    syncBeforeUnloadGuard();
  };

  const installOpenedOccurrence = (opened: OpenedProjection): void => {
    const nextStore = createProjectionStore(opened.table);
    tracker.reset(); savedView = JSON.stringify(tracker.view); activeProject = null;
    pendingTextBuffers.clear();
    pendingBooleanBuffers.clear();
    pendingDateBuffers.clear();
    pendingFormulaBuffers.clear();
    pendingNumberBuffers.clear(); budgetToolsDraft = {}; budgetTables = [];
    bootstrap = opened.bootstrap;
    store = nextStore;
    selectedCollection = opened.bootstrap.default_collection;
    occurrenceClosed = false;
    durability.install(opened.bootstrap.revision, true);
    syncBeforeUnloadGuard();
  };

  const refreshSavedProjects = async (preferred?: string): Promise<void> => {
    savedProjects = await host.list();
    if (preferred !== undefined && savedProjects.some(({ name }) => name === preferred)) {
      selectedSavedProject = preferred;
    } else if (!savedProjects.some(({ name }) => name === selectedSavedProject)) {
      selectedSavedProject = savedProjects[0]?.name ?? "";
    }
  };

  const showProjectFailure = (title: string, error: unknown): void => {
    const failure =
      error instanceof DesignerRuntimeError
        ? error.failure
        : {
            message: error instanceof Error ? error.message : String(error),
            diagnostics: [],
          };
    notice = {
      tone: "error",
      title,
      message: failure.message,
      diagnostics: failure.diagnostics,
    };
  };

  const confirmDiscardDirtyOccurrence = (action: string): boolean =>
    (!durability.snapshot().dirty && !hasPendingScalarDrafts()) ||
    window.confirm(
      `${action} will discard unsaved changes in the current project. Continue?`,
    );

  const openSavedProject = async (): Promise<void> => {
    if (busy || selectedSavedProject === "") return;
    if (!confirmDiscardDirtyOccurrence("Open")) return;
    busy = true;
    notice = null;
    render();
    try {
      const snapshot = host.readSnapshot ? await host.readSnapshot(selectedSavedProject) : {bytes: await host.read(selectedSavedProject), presentation: undefined};
      const view = parseTrackerView(snapshot.presentation);
      const durableBytes = snapshot.bytes.slice(0);
      await installProjectBytes(snapshot.bytes);
      tracker.reset(view); savedView = JSON.stringify(view);
      if (view.budgetViews && store) {
        await refreshBudgetTables(store.snapshot().table.revision);
        const active = view.budgetViews.views.find(v => v.id === view.budgetViews?.active);
        const table = budgetTables.find(t => t.collection.id === active?.collection);
        if (table) { store = createProjectionStore(table); selectedCollection = table.collection.key; }
      }
      activeProject = {name: selectedSavedProject, bytes: durableBytes, presentation: snapshot.presentation};
      notice = {
        tone: "success",
        title: "Project opened",
        message: `${selectedSavedProject} is current in a fresh Rust occurrence. ${projectRepresentation(durableBytes)}`,
        diagnostics: [],
      };
    } catch (error) {
      showProjectFailure("Project not opened", error);
    } finally {
      busy = false;
      render();
    }
  };

  const installProjectBytes = async (bytes: ArrayBuffer): Promise<void> => {
    const opened = await client.openProject(bytes);
    installOpenedOccurrence(opened);
  };

  const importProjectDirectory = async (input: HTMLInputElement): Promise<void> => {
    if (busy) return;
    if (!confirmDiscardDirtyOccurrence("Open")) {
      input.value = "";
      return;
    }
    const files = input.files;
    if (files === null) return;
    busy = true;
    notice = null;
    render();
    try {
      const bytes = await projectTransferFromFiles(files);
      await installProjectBytes(bytes);
      notice = {
        tone: "success",
        title: "Project opened",
        message: "The selected canonical .roproj/v1 is current in a fresh Rust occurrence.",
        diagnostics: [],
      };
    } catch (error) {
      showProjectFailure("Project not opened", error);
    } finally {
      input.value = "";
      busy = false;
      render();
    }
  };

  const saveAs = async (): Promise<void> => {
    if (store === null || busy) return;
    if (tracker.pending || (tracker.view.budgetViews && hasEditDrafts())) { showProjectFailure("Project not saved", new Error("Apply or cancel the cell draft and pending formula edits before saving.")); render(); return; }
    const requestedName = window.prompt(
      "Save As a new browser project (existing destinations are never overwritten):",
      `${bootstrap?.title.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replaceAll(/^-|-$/g, "") || "project"}.roproj`,
    );
    if (requestedName === null) return;
    busy = true;
    notice = null;
    render();
    try {
      const expectedRevision = store.snapshot().table.revision;
      const project = await client.exportProject(expectedRevision);
      const presentation = JSON.stringify(tracker.view);
      await host.publish(requestedName, project.bytes, presentation);
      activeProject = {name: requestedName.trim(), bytes: project.bytes.slice(0), presentation};
      savedView = presentation;
      durability.published(project.revision);
      syncBeforeUnloadGuard();
      let refreshWarning = "";
      try {
        await refreshSavedProjects(requestedName.trim());
      } catch (error) {
        refreshWarning = ` The project list could not refresh: ${
          error instanceof Error ? error.message : String(error)
        }`;
      }
      notice = {
        tone: "success",
        title: "Save As complete",
        message: `${requestedName.trim()} durably committed revision ${
          project.revision
        }. ${projectRepresentation(project.bytes)}${refreshWarning}`,
        diagnostics: [],
      };
    } catch (error) {
      showProjectFailure("Project not saved", error);
    } finally {
      busy = false;
      render();
    }
  };

  const newTracker = async (): Promise<void> => {
    if (busy || !client.newTracker || !confirmDiscardDirtyOccurrence("New Tracker")) return;
    busy = true; notice = null; render();
    try { installOpenedOccurrence(await client.newTracker()); durability.install(store?.snapshot().table.revision ?? "", false); }
    catch (error) { showProjectFailure("Tracker not created", error); }
    finally { busy = false; syncBeforeUnloadGuard(); render(); }
  };

  const newBudget = async (): Promise<void> => {
    if (busy || !client.newBudget || !confirmDiscardDirtyOccurrence("New Budget")) return;
    busy = true; notice = null; render();
    try {
      const opened = await client.newBudget();
      installOpenedOccurrence(opened);
      tracker.view.budgetViews = defaultBudgetViews(opened.bootstrap.collections.map(c => c.id));
      tracker.view.budgetViews.views.forEach(view => { view.name = humanize(opened.bootstrap.collections.find(c => c.id === view.collection)?.key ?? "Budget"); });
      await refreshBudgetTables(opened.bootstrap.revision);
      durability.install(opened.bootstrap.revision, false);
    }
    catch (error) { showProjectFailure("Budget not created", error); }
    finally { busy = false; syncBeforeUnloadGuard(); render(); }
  };

  const save = async (): Promise<void> => {
    if (activeProject === null) { await saveAs(); return; }
    if (!store || busy) return;
    if (tracker.pending || (tracker.view.budgetViews && hasEditDrafts())) { showProjectFailure("Project not saved", new Error("Apply or cancel the cell draft and pending formula edits before saving.")); render(); return; }
    busy = true; notice = null; render();
    try {
      if (!host.update) throw new Error("This browser host does not support Save; use Save As.");
      const project = await client.exportProject(store.snapshot().table.revision);
      const presentation = JSON.stringify(tracker.view);
      await host.update(activeProject.name, project.bytes, activeProject.bytes, presentation, activeProject.presentation);
      activeProject = {...activeProject, bytes: project.bytes.slice(0), presentation};
      savedView = presentation; durability.published(project.revision);
      notice = {tone: "success", title: "Save complete", message: `${activeProject.name} saved in this browser. ${projectRepresentation(project.bytes)}`, diagnostics: []};
    } catch (error) { showProjectFailure("Project not saved", error); }
    finally { busy = false; syncBeforeUnloadGuard(); render(); }
  };

  const closeOccurrence = async (): Promise<void> => {
    if (busy) return;
    if (!confirmDiscardDirtyOccurrence("Close")) return;
    busy = true;
    notice = null;
    render();
    try {
      await client.closeProject();
      pendingTextBuffers.clear();
      pendingBooleanBuffers.clear();
      pendingDateBuffers.clear();
      pendingFormulaBuffers.clear();
      pendingNumberBuffers.clear(); budgetToolsDraft = {}; budgetTables = [];
      tracker.reset(); savedView = JSON.stringify(tracker.view); activeProject = null;
      bootstrap = null;
      store = null;
      selectedCollection = "";
      occurrenceClosed = true;
      durability.close();
      syncBeforeUnloadGuard();
      notice = {
        tone: "success",
        title: "Project closed",
        message: "The Rust resident occurrence was destroyed. Durable projects are unchanged.",
        diagnostics: [],
      };
    } catch (error) {
      showProjectFailure("Project not closed", error);
    } finally {
      busy = false;
      render();
    }
  };

  const bindInteractions = (): void => {
    root.querySelector("[data-new-tracker]")?.addEventListener("click", () => { void newTracker(); });
    root.querySelector("[data-new-budget]")?.addEventListener("click", () => { void newBudget(); });
    root.querySelector("[data-save-project]")?.addEventListener("click", () => { void save(); });
    root.querySelectorAll<HTMLFormElement>("[data-edit-form]").forEach((form) => {
      const draftControl = form.querySelector<HTMLTextAreaElement>("textarea");
      const draftBoolean = form.querySelector<HTMLInputElement>('input[type="checkbox"]');
      const draftDate = form.querySelector<HTMLInputElement>('input[type="date"]');
      const draftNumber = form.querySelector<HTMLInputElement>('input[type="number"]');
      const draftEntity = decodeOpaqueAttribute(form.dataset.entity);
      const draftField = decodeOpaqueAttribute(form.dataset.field);
      if (draftNumber && draftEntity !== undefined && draftField !== undefined) {
        draftNumber.addEventListener("input", () => {
          const key = textBufferKey({entity: draftEntity, field: draftField});
          if (draftNumber.value === draftNumber.dataset.initialNumber) pendingNumberBuffers.delete(key);
          else pendingNumberBuffers.set(key, draftNumber.value);
          reflectUnsavedState();
        });
      }
      if (draftControl !== null && draftEntity !== undefined && draftField !== undefined) {
        const recordDraft = (): void => {
          const key = textBufferKey({ entity: draftEntity, field: draftField });
          if (draftControl.value === draftControl.dataset.initialNormalized) {
            pendingTextBuffers.delete(key);
          } else {
            pendingTextBuffers.set(key, draftControl.value);
          }
          reflectUnsavedState();
        };
        draftControl.addEventListener("input", recordDraft);
      }
      if (draftBoolean !== null && draftEntity !== undefined && draftField !== undefined) {
        const recordDraft = (): void => {
          const key = textBufferKey({ entity: draftEntity, field: draftField });
          if (String(draftBoolean.checked) === draftBoolean.dataset.initialChecked) {
            pendingBooleanBuffers.delete(key);
          } else {
            pendingBooleanBuffers.set(key, draftBoolean.checked);
          }
          reflectUnsavedState();
        };
        draftBoolean.addEventListener("change", recordDraft);
      }
      if (draftDate !== null && draftEntity !== undefined && draftField !== undefined) {
        const recordDraft = (): void => {
          const key = textBufferKey({ entity: draftEntity, field: draftField });
          if (draftDate.value === draftDate.dataset.initialDate) {
            pendingDateBuffers.delete(key);
          } else {
            pendingDateBuffers.set(key, draftDate.value);
          }
          reflectUnsavedState();
        };
        draftDate.addEventListener("input", recordDraft);
      }
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const control = form.querySelector<HTMLInputElement | HTMLTextAreaElement>(
          "input, textarea",
        );
        const entity = decodeOpaqueAttribute(form.dataset.entity);
        const field = decodeOpaqueAttribute(form.dataset.field);
        if (control === null || entity === undefined || field === undefined) return;
        switch (form.dataset.editKind) {
          case "number":
            void commitNumber({ entity, field }, control.value);
            break;
          case "text": {
            const initialText =
              control instanceof HTMLTextAreaElement
                ? decodeOpaqueAttribute(control.dataset.initialText)
                : undefined;
            const initialNormalized =
              control instanceof HTMLTextAreaElement
                ? control.dataset.initialNormalized
                : undefined;
            let value = control.value;
            try {
              if (initialText !== undefined && initialNormalized !== undefined && control instanceof HTMLTextAreaElement) {
                value = reconcileTextEdit(initialText, initialNormalized, control.value);
              }
            } catch (error) {
              notice = { tone: "error", title: "Text edit not applied", message: error instanceof Error ? error.message : "Text edit reconciliation failed.", diagnostics: [] };
              render();
              return;
            }
            const textKey = textBufferKey({ entity, field });
            if (control.value === initialNormalized) {
              pendingTextBuffers.delete(textKey);
            } else {
              pendingTextBuffers.set(textKey, control.value);
            }
            reflectUnsavedState();
            void commitText({ entity, field }, value);
            break;
          }
          case "boolean": {
            if (!(control instanceof HTMLInputElement)) return;
            const booleanKey = textBufferKey({ entity, field });
            if (String(control.checked) === control.dataset.initialChecked) {
              pendingBooleanBuffers.delete(booleanKey);
            } else {
              pendingBooleanBuffers.set(booleanKey, control.checked);
            }
            reflectUnsavedState();
            void commitBoolean({ entity, field }, control.checked);
            break;
          }
          case "date": {
            if (!(control instanceof HTMLInputElement)) return;
            const dateKey = textBufferKey({ entity, field });
            if (control.value === control.dataset.initialDate) {
              pendingDateBuffers.delete(dateKey);
            } else {
              pendingDateBuffers.set(dateKey, control.value);
            }
            reflectUnsavedState();
            void commitDate({ entity, field }, control.value);
            break;
          }
        }
      });
    });
    root.querySelectorAll<HTMLFormElement>("[data-formula-form]").forEach((form) => {
      const input = form.querySelector<HTMLInputElement>("input");
      const entity = decodeOpaqueAttribute(form.dataset.entity);
      const field = decodeOpaqueAttribute(form.dataset.field);
      if (input !== null && entity !== undefined && field !== undefined) {
        input.addEventListener("input", () => {
          const key = textBufferKey({ entity, field });
          if (input.value === input.dataset.initialFormula) {
            pendingFormulaBuffers.delete(key);
          } else {
            pendingFormulaBuffers.set(key, input.value);
          }
          reflectUnsavedState();
        });
      }
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const source = input?.value;
        if (entity !== undefined && field !== undefined && source !== undefined) {
          void commitFormula({ entity, field }, source);
        }
      });
    });
    root.querySelectorAll<HTMLButtonElement>("[data-format-cycle]").forEach((button) => {
      button.addEventListener("click", () => {
        const entity = decodeOpaqueAttribute(button.dataset.entity);
        const field = decodeOpaqueAttribute(button.dataset.field);
        if (entity === undefined || field === undefined) return;
        const key = cellKey(entity, field);
        const order: NumberFormat[] = ["number", "currency-jpy", "percentage", "currency-usd"];
        const current = tracker.view.formats[key] ?? "number";
        tracker.view.formats[key] = order[(order.indexOf(current) + 1) % order.length] ?? "number";
        reflectUnsavedState();
        render();
      });
    });
    root
      .querySelector<HTMLSelectElement>("[data-collection-select]")
      ?.addEventListener("change", (event) => {
        const select = event.currentTarget;
        if (select instanceof HTMLSelectElement) {
          void selectCollection(select.value);
        }
      });
    root.querySelector<HTMLButtonElement>("[data-open-project]")?.addEventListener("click", () => {
      void openSavedProject();
    });
    root.querySelector<HTMLButtonElement>("[data-save-as]")?.addEventListener("click", () => {
      void saveAs();
    });
    root.querySelector<HTMLButtonElement>("[data-close-project]")?.addEventListener("click", () => {
      void closeOccurrence();
    });
    root
      .querySelector<HTMLSelectElement>("[data-saved-project-select]")
      ?.addEventListener("change", (event) => {
        const select = event.currentTarget;
        if (select instanceof HTMLSelectElement) selectedSavedProject = select.value;
      });
    root
      .querySelector<HTMLInputElement>("[data-import-project]")
      ?.addEventListener("change", (event) => {
        const input = event.currentTarget;
        if (input instanceof HTMLInputElement && input.files !== null) {
          void importProjectDirectory(input);
        }
      });
  };

  const hydrateDraftControls = (): void => {
    root.querySelectorAll<HTMLInputElement>("input[data-initial-number]").forEach(input => {
      const entity = decodeOpaqueAttribute(input.form?.dataset.entity);
      const field = decodeOpaqueAttribute(input.form?.dataset.field);
      if (entity !== undefined && field !== undefined) input.value = pendingNumberBuffers.get(textBufferKey({entity, field})) ?? input.dataset.initialNumber ?? "";
    });
    root.querySelectorAll<HTMLTextAreaElement>("textarea[data-initial-text]").forEach(
      (textarea) => {
        const initialText = decodeOpaqueAttribute(textarea.dataset.initialText);
        if (initialText !== undefined) {
          const entity = decodeOpaqueAttribute(textarea.form?.dataset.entity);
          const field = decodeOpaqueAttribute(textarea.form?.dataset.field);
          textarea.value =
            entity !== undefined && field !== undefined
              ? (pendingTextBuffers.get(textBufferKey({ entity, field })) ?? initialText)
              : initialText;
          textarea.dataset.initialNormalized = normalizeLineEndings(initialText);
        }
      },
    );
    root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]').forEach((checkbox) => {
      const entity = decodeOpaqueAttribute(checkbox.form?.dataset.entity);
      const field = decodeOpaqueAttribute(checkbox.form?.dataset.field);
      if (entity !== undefined && field !== undefined) {
        const pending = pendingBooleanBuffers.get(textBufferKey({ entity, field }));
        if (pending !== undefined) checkbox.checked = pending;
      }
    });
    root.querySelectorAll<HTMLInputElement>('input[type="date"][data-initial-date]').forEach(
      (dateInput) => {
        const entity = decodeOpaqueAttribute(dateInput.form?.dataset.entity);
        const field = decodeOpaqueAttribute(dateInput.form?.dataset.field);
        if (entity !== undefined && field !== undefined) {
          dateInput.value =
            pendingDateBuffers.get(textBufferKey({ entity, field })) ??
            dateInput.dataset.initialDate ??
            "";
        }
      },
    );
    root.querySelectorAll<HTMLInputElement>("input[data-initial-formula]").forEach((input) => {
      const entity = decodeOpaqueAttribute(input.form?.dataset.entity);
      const field = decodeOpaqueAttribute(input.form?.dataset.field);
      if (entity !== undefined && field !== undefined) {
        input.value = pendingFormulaBuffers.get(textBufferKey({ entity, field })) ?? input.dataset.initialFormula ?? "";
      }
    });
  };

  render();
  const ready = (async () => {
    try {
      const candidate = await client.bootstrap();
      await installOccurrence(candidate, false);
      try {
        await refreshSavedProjects();
      } catch (error) {
        showProjectFailure("Browser persistence unavailable", error);
      }
    } catch (error) {
      showFailure(error, false);
      startupFailure =
        error instanceof DesignerRuntimeError
          ? error.failure.message
          : error instanceof Error
            ? error.message
            : String(error);
    } finally {
      render();
    }
  })();

  return {
    ready,
    destroy: () => {
      destroyed = true;
      syncBeforeUnloadGuard();
      root.replaceChildren();
      void client.close();
    },
  };
}

function textBufferKey(target: FieldTarget): string {
  return JSON.stringify([target.entity, target.field]);
}

function loadingMarkup(): string {
  return `
    <main class="loading-shell" aria-live="polite">
      <div class="loading-mark" aria-hidden="true">T</div>
      <p>Starting the Rust workspace…</p>
    </main>
  `;
}

function startupFailureMarkup(message: string): string {
  return `
    <main class="loading-shell" role="alert">
      <div class="loading-mark" aria-hidden="true">!</div>
      <p>${escapeHtml(message)}</p>
    </main>
  `;
}

function designerMarkup(
  bootstrap: BootstrapProjection,
  table: TableProjection,
  currentness: "current" | "refreshing" | "refresh_failed",
  selectedCollection: string,
  notice: Notice | null,
  busy: boolean,
  dirty: boolean,
  durableRevision: string | null,
  savedProjects: SavedProjectSummary[],
  selectedSavedProject: string,
  view: TrackerView,
): string {
  const isTracker = table.tracker_profile === true;
  const statusLabel = {
    current: isTracker ? "Up to date" : "Semantic current",
    refreshing: "Refreshing affected fields",
    refresh_failed: "Refresh incomplete",
  }[currentness];
  return `
    <div class="designer-shell">
      <header class="workspace-header">
        <div class="brand-lockup">
          <span class="brand-mark" aria-hidden="true">T</span>
          <div><strong>Tachiko</strong><span>Designer</span></div>
        </div>
        <div class="workspace-title">
          <p class="eyebrow">${isTracker ? "Project workspace" : "Semantic project workspace"}</p>
          <h1>${escapeHtml(bootstrap.title)}</h1>
        </div>
        <div class="revision-chip" data-currentness="${currentness}">
          <span>${statusLabel}</span>
          <code ${isTracker ? 'class="tracker-revision"' : ""} data-testid="revision">${escapeHtml(table.revision)}</code>
        </div>
        <div class="workspace-actions">
          <div class="durability-chip" data-testid="durability" data-dirty="${String(dirty)}">
            <span>${dirty ? "Unsaved changes" : "Saved"}</span>
            <code>${isTracker ? "Stored in this browser after Save" : durableRevision === null ? "No durable revision" : escapeHtml(durableRevision)}</code>
          </div>
          <label class="saved-project-picker">
            <span>Browser projects</span>
            <select data-saved-project-select aria-label="Saved project" ${busy ? "disabled" : ""}>
              ${savedProjectOptions(savedProjects, selectedSavedProject)}
            </select>
          </label>
          <div class="project-buttons">
            <label class="project-button" data-import-label>
              Open .roproj
              <input type="file" data-import-project webkitdirectory multiple ${
                busy ? "disabled" : ""
              } />
            </label>
            <button type="button" data-open-project ${
              busy || selectedSavedProject === "" ? "disabled" : ""
            }>Open</button>
            <button type="button" data-new-tracker ${busy ? "disabled" : ""}>New Tracker</button>
            <button type="button" data-new-budget ${busy ? "disabled" : ""}>New Budget</button>
            <button type="button" data-save-project ${busy ? "disabled" : ""}>Save</button>
            <button type="button" data-save-as ${busy ? "disabled" : ""}>Save As</button>
            <button type="button" data-close-project ${busy ? "disabled" : ""}>Close</button>
          </div>
        </div>
      </header>

      <main class="workspace-layout">
        <aside class="collection-rail" aria-label="Workspace collections">
          <label for="collection-select">Collection</label>
          <select id="collection-select" data-collection-select ${busy ? "disabled" : ""}>
            ${bootstrap.collections
              .map(
                (collection) => `
                  <option value="${escapeHtml(collection.key)}" ${
                    collection.key === selectedCollection ? "selected" : ""
                  }>
                    ${escapeHtml(humanize(collection.key))} · ${String(collection.entity_count)}
                  </option>`,
              )
              .join("")}
          </select>
        </aside>

        <section class="table-workbench" aria-labelledby="table-title">
          <div class="table-heading">
            <div>
              <p class="eyebrow">Bounded semantic projection</p>
              <h2 id="table-title">${escapeHtml(humanize(table.collection.key))}</h2>
            </div>
            <span>${String(table.rows.length)} ${table.rows.length === 1 ? "entity" : "entities"}</span>
          </div>

          <ol class="calculation-thread" aria-label="Edit publication path">
            <li><span>1</span><strong>Stored value</strong><small>Editable scalar</small></li>
            <li><span>2</span><strong>Rust authority</strong><small>Expected revision</small></li>
            <li><span>3</span><strong>Formula refresh</strong><small>Affected fields only</small></li>
          </ol>

          ${noticeMarkup(notice)}

          <div class="table-scroll">
            <table>
              <thead>
                <tr>
                  <th scope="col">Entity</th>
                  ${table.columns
                    .map(
                      (column) => `<th scope="col"><span>${escapeHtml(
                        humanize(column.key),
                      )}</span><small>${escapeHtml(column.field_type)}</small></th>`,
                    )
                    .join("")}
                </tr>
              </thead>
              <tbody>
                ${table.rows
                  .map((row) => rowMarkup(row, table, busy || currentness !== "current", view))
                  .join("")}
              </tbody>
            </table>
          </div>
          <p class="table-footnote">Human-readable keys are shown here; edits target stable semantic IDs.</p>
        </section>
      </main>
    </div>
  `;
}

function closedMarkup(
  savedProjects: SavedProjectSummary[],
  selectedSavedProject: string,
  busy: boolean,
  notice: Notice | null,
): string {
  return `
    <main class="closed-shell">
      <div class="loading-mark" aria-hidden="true">T</div>
      <p class="eyebrow">Resident occurrence destroyed</p>
      <h1>No project open</h1>
      <p>Choose a durable browser project to create a fresh Rust-authoritative occurrence.</p>
      ${noticeMarkup(notice)}
      <label class="saved-project-picker">
        <span>Browser projects</span>
        <select data-saved-project-select aria-label="Saved project" ${busy ? "disabled" : ""}>
          ${savedProjectOptions(savedProjects, selectedSavedProject)}
        </select>
      </label>
      <label class="project-button" data-import-label>
        Open .roproj folder
        <input type="file" data-import-project webkitdirectory multiple ${
          busy ? "disabled" : ""
        } />
      </label>
      <button type="button" data-new-tracker ${busy ? "disabled" : ""}>New Tracker</button>
      <button type="button" data-new-budget ${busy ? "disabled" : ""}>New Budget</button>
      <button type="button" data-open-project ${
        busy || selectedSavedProject === "" ? "disabled" : ""
      }>Open project</button>
    </main>
  `;
}

function savedProjectOptions(
  projects: SavedProjectSummary[],
  selected: string,
): string {
  if (projects.length === 0) return '<option value="">No saved projects</option>';
  return projects
    .map(
      (project) => `<option value="${escapeHtml(project.name)}" ${
        project.name === selected ? "selected" : ""
      }>${escapeHtml(project.name)} · ${formatBytes(project.byte_length)}</option>`,
    )
    .join("");
}

function rowMarkup(
  row: TableProjection["rows"][number],
  table: TableProjection,
  busy: boolean,
  view: TrackerView,
): string {
  const fields = new Map(row.fields.map((field) => [field.target.field, field]));
  return `
    <tr>
      <th scope="row">
        <strong>${escapeHtml(humanize(row.key))}</strong>
        <code>${escapeHtml(row.id)}</code>
      </th>
      ${table.columns
        .map((column) =>
          fieldMarkup(fields.get(column.id), row.key, column.key, busy, view),
        )
        .join("")}
    </tr>
  `;
}

function fieldMarkup(
  field: FieldProjection | undefined,
  entityKey: string,
  fieldKey: string,
  busy: boolean,
  view: TrackerView,
): string {
  if (field === undefined) return '<td class="empty-cell">—</td>';
  const key = `${field.target.entity}.${field.target.field}`;
  const diagnostics = field.diagnostics
    .map((diagnostic) => `<small class="field-error">${escapeHtml(diagnostic.message)}</small>`)
    .join("");
  if (field.editable_scalar === "number" && field.stored?.kind === "number") {
    const format = view.formats[cellKey(field.target.entity, field.target.field)] ?? "number";
    return `
      <td data-field="${escapeHtml(key)}" class="stored-cell">
        <form data-edit-form data-entity="${encodeOpaqueAttribute(
          field.target.entity,
        )}" data-field="${encodeOpaqueAttribute(field.target.field)}" data-edit-kind="number">
          <input
            type="number"
            data-initial-number="${String(field.stored.value)}"
            step="any"
            value="${String(field.stored.value)}"
            aria-label="${escapeHtml(humanize(fieldKey))} for ${escapeHtml(
              humanize(entityKey),
            )}"
            ${busy ? "disabled" : ""}
          />
          <button type="submit" ${busy ? "disabled" : ""}>Apply</button>
        </form>
        <button type="button" data-format-cycle data-entity="${encodeOpaqueAttribute(field.target.entity)}" data-field="${encodeOpaqueAttribute(field.target.field)}" ${busy ? "disabled" : ""}>${escapeHtml(formatLabel(format))}</button>
        <output data-formatted-number>${escapeHtml(formatNumber(field.stored.value, format))}</output>
        <small class="value-kind">Stored · Number · ${escapeHtml(formatLabel(format))}</small>
        ${diagnostics}
      </td>
    `;
  }
  if (field.editable_scalar === "text" && field.stored?.kind === "text") {
    return `
      <td data-field="${escapeHtml(key)}" class="stored-cell">
        <form data-edit-form data-entity="${encodeOpaqueAttribute(
          field.target.entity,
        )}" data-field="${encodeOpaqueAttribute(field.target.field)}" data-edit-kind="text">
          <textarea
            data-initial-text="${encodeOpaqueAttribute(field.stored.value)}"
            aria-label="${escapeHtml(humanize(fieldKey))} for ${escapeHtml(
              humanize(entityKey),
            )}"
            ${busy ? "disabled" : ""}
          ></textarea>
          <button type="submit" ${busy ? "disabled" : ""}>Apply</button>
        </form>
        <small class="value-kind">Stored · Text</small>
        ${diagnostics}
      </td>
    `;
  }
  if (field.editable_scalar === "boolean" && field.stored?.kind === "boolean") {
    return `
      <td data-field="${escapeHtml(key)}" class="stored-cell">
        <form data-edit-form data-entity="${encodeOpaqueAttribute(
          field.target.entity,
        )}" data-field="${encodeOpaqueAttribute(field.target.field)}" data-edit-kind="boolean">
          <input
            type="checkbox"
            data-initial-checked="${String(field.stored.value)}"
            ${field.stored.value ? "checked" : ""}
            aria-label="${escapeHtml(humanize(fieldKey))} for ${escapeHtml(
              humanize(entityKey),
            )}"
            ${busy ? "disabled" : ""}
          />
          <button type="submit" ${busy ? "disabled" : ""}>Apply</button>
        </form>
        <small class="value-kind">Stored · Boolean</small>
        ${diagnostics}
      </td>
    `;
  }
  if (field.editable_scalar === "date" && field.stored?.kind === "date") {
    return `
      <td data-field="${escapeHtml(key)}" class="stored-cell">
        <form data-edit-form data-entity="${encodeOpaqueAttribute(
          field.target.entity,
        )}" data-field="${encodeOpaqueAttribute(field.target.field)}" data-edit-kind="date">
          <input
            type="date"
            data-initial-date="${escapeHtml(field.stored.value)}"
            value="${escapeHtml(field.stored.value)}"
            aria-label="${escapeHtml(humanize(fieldKey))} for ${escapeHtml(
              humanize(entityKey),
            )}"
            ${busy ? "disabled" : ""}
          />
          <button type="submit" ${busy ? "disabled" : ""}>Apply</button>
        </form>
        <small class="value-kind">Stored · Date</small>
        ${diagnostics}
      </td>
    `;
  }
  if (field.formula !== null) {
    const format = view.formats[cellKey(field.target.entity, field.target.field)] ?? "number";
    return `
      <td data-field="${escapeHtml(key)}" class="formula-cell">
        <output>${escapeHtml(calculationValue(field, format))}</output>
        <span class="formula-badge">ƒ Calculated</span>
        <form data-formula-form data-entity="${encodeOpaqueAttribute(field.target.entity)}" data-field="${encodeOpaqueAttribute(field.target.field)}">
          <input data-initial-formula="${escapeHtml(field.formula.source)}" value="${escapeHtml(field.formula.source)}" aria-label="Formula for ${escapeHtml(humanize(fieldKey))} for ${escapeHtml(humanize(entityKey))}" ${busy ? "disabled" : ""} />
          <button type="submit" ${busy ? "disabled" : ""}>Apply formula</button>
        </form>
        <button type="button" data-format-cycle data-entity="${encodeOpaqueAttribute(field.target.entity)}" data-field="${encodeOpaqueAttribute(field.target.field)}" ${busy ? "disabled" : ""}>${escapeHtml(formatLabel(format))}</button>
        ${diagnostics}
      </td>
    `;
  }
  return `
    <td data-field="${escapeHtml(key)}" class="stored-cell readonly">
      <span>${escapeHtml(storedValue(field))}</span>
      <small class="value-kind">Stored</small>
      ${diagnostics}
    </td>
  `;
}

function noticeMarkup(notice: Notice | null): string {
  if (notice === null) return '<div class="notice-slot" aria-live="polite"></div>';
  return `
    <div class="notice ${notice.tone}" role="${notice.tone === "error" ? "alert" : "status"}">
      <strong>${escapeHtml(notice.title)}</strong>
      <span>${escapeHtml(notice.message)}</span>
      ${notice.diagnostics
        .map(
          (diagnostic) => `<small><code>${escapeHtml(diagnostic.code)}</code> ${escapeHtml(
            diagnostic.message,
          )}</small>`,
        )
        .join("")}
    </div>
  `;
}

function calculationValue(field: FieldProjection, format: NumberFormat): string {
  if (field.calculated?.status === "value") return formatNumber(field.calculated.value, format);
  if (field.calculated?.status === "failure") return field.calculated.message;
  return "Unavailable";
}

function projectRepresentation(bytes: ArrayBuffer): string {
  return new TextDecoder().decode(bytes.slice(0, 8)) === "TWDPROJ2"
    ? "Storage: direct-ro/v2 with browser-only view settings; not a portable .roproj package."
    : "Storage: .roproj/v1 with browser-only view settings.";
}

function storedValue(field: FieldProjection): string {
  const stored = field.stored;
  if (stored === null) return "—";
  switch (stored.kind) {
    case "number":
      return formatNumber(stored.value);
    case "text":
      return stored.value;
    case "boolean":
      return stored.value ? "True" : "False";
    case "date":
      return stored.value;
    case "reference":
      return `→ ${stored.entity}`;
  }
}

function humanize(value: string): string {
  return value
    .split("_")
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

function formatNumber(value: number, format: NumberFormat = "number"): string {
  switch (format) {
    case "percentage": return new Intl.NumberFormat("en-US", { style: "percent", maximumFractionDigits: 2 }).format(value);
    case "currency-jpy": return new Intl.NumberFormat("ja-JP", { style: "currency", currency: "JPY", maximumFractionDigits: 0 }).format(value);
    case "currency-usd": return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(value);
    case "number": return new Intl.NumberFormat("en-US", { maximumFractionDigits: 4 }).format(value);
  }
}

function formatLabel(format: NumberFormat): string {
  return { number: "Number", percentage: "Percentage", "currency-jpy": "JPY", "currency-usd": "USD" }[format];
}

function formatBytes(value: number): string {
  if (value < 1_024) return `${String(value)} B`;
  return `${(value / 1_024).toFixed(1)} KiB`;
}

function escapeHtml(value: string): string {
  return value.replaceAll(/[&<>"']/g, (character) => {
    const replacements: Record<string, string> = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#039;",
    };
    return replacements[character] ?? character;
  });
}

function encodeOpaqueAttribute(value: string): string {
  return escapeHtml(JSON.stringify(value));
}

function decodeOpaqueAttribute(value: string | undefined): string | undefined {
  if (value === undefined) return undefined;
  try {
    const decoded: unknown = JSON.parse(value);
    return typeof decoded === "string" ? decoded : undefined;
  } catch {
    return undefined;
  }
}
