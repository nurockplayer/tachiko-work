import {
  createProjectionStore,
  type ControlProjection,
  type ProjectionStore,
} from "./projection-store.ts";
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
  let bootstrap: BootstrapProjection | null = null;
  let store: ProjectionStore | null = null;
  let selectedCollection = "";
  let notice: Notice | null = null;
  let startupFailure: string | null = null;
  let busy = false;
  let destroyed = false;
  let occurrenceClosed = false;
  let savedProjects: SavedProjectSummary[] = [];
  let selectedSavedProject = "";
  const durability = createDurabilityState();
  let beforeUnloadGuarded = false;

  const warnBeforeDirtyUnload = (event: BeforeUnloadEvent): void => {
    event.preventDefault();
    Reflect.set(event, "returnValue", true);
  };

  const syncBeforeUnloadGuard = (): void => {
    const shouldGuard = !destroyed && durability.snapshot().dirty;
    if (shouldGuard && !beforeUnloadGuarded) {
      window.addEventListener("beforeunload", warnBeforeDirtyUnload);
      beforeUnloadGuarded = true;
    } else if (!shouldGuard && beforeUnloadGuarded) {
      window.removeEventListener("beforeunload", warnBeforeDirtyUnload);
      beforeUnloadGuarded = false;
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
      snapshot.control,
      snapshot.currentness,
      selectedCollection,
      notice,
      busy,
      durability.snapshot().dirty,
      durability.snapshot().durable_revision,
      savedProjects,
      selectedSavedProject,
    );
    hydrateTextareas();
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
  ): Promise<void> => {
    if (store === null || busy || store.snapshot().currentness !== "current") return;
    busy = true;
    notice = null;
    render();
    let published = false;
    try {
      const publication = await publish(store.snapshot().table.revision);
      published = true;
      const requested = store.beginPublication(publication);
      durability.observe(publication.resulting_revision);
      syncBeforeUnloadGuard();
      render();
      const refresh = await client.queryFields(
        publication.resulting_revision,
        requested,
      );
      store.finishRefresh(refresh);
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

  const commitNumber = (target: FieldTarget, input: string): Promise<void> =>
    commitScalar(target, (expectedRevision) => client.editNumber(expectedRevision, target, input));

  const commitText = (target: FieldTarget, value: string): Promise<void> =>
    commitScalar(target, (expectedRevision) => client.editText(expectedRevision, target, value));

  const commitBoolean = (target: FieldTarget, value: boolean): Promise<void> =>
    commitScalar(target, (expectedRevision) =>
      client.editBoolean(expectedRevision, target, value),
    );

  const selectCollection = async (collection: string): Promise<void> => {
    if (bootstrap === null || store === null || busy) return;
    busy = true;
    notice = null;
    render();
    try {
      const expectedRevision = store.snapshot().table.revision;
      const [table, controlBatch] = await Promise.all([
        client.queryTable(collection),
        client.queryFields(expectedRevision, [bootstrap.control_field]),
      ]);
      if (table.revision !== expectedRevision || controlBatch.revision !== expectedRevision) {
        throw new Error("Collection query returned a different semantic revision.");
      }
      const controlField = controlBatch.fields[0];
      if (controlField?.calculated?.status !== "value") {
        throw new Error("The unrelated control projection is unavailable.");
      }
      store = createProjectionStore(table, {
        target: bootstrap.control_field,
        value: controlField.calculated.value,
        revision: expectedRevision,
      });
      selectedCollection = collection;
    } catch (error) {
      showFailure(error, false);
    } finally {
      busy = false;
      render();
    }
  };

  const installOccurrence = async (
    candidate: BootstrapProjection,
    durable: boolean,
  ): Promise<void> => {
    const [table, controlBatch] = await Promise.all([
      client.queryTable(candidate.default_collection),
      client.queryFields(candidate.revision, [candidate.control_field]),
    ]);
    if (table.revision !== candidate.revision || controlBatch.revision !== candidate.revision) {
      throw new Error("Initial projections do not share one semantic revision.");
    }
    const controlField = controlBatch.fields[0];
    if (controlField?.calculated?.status !== "value") {
      throw new Error("The unrelated control projection is unavailable.");
    }
    const nextStore = createProjectionStore(table, {
      target: candidate.control_field,
      value: controlField.calculated.value,
      revision: candidate.revision,
    });
    bootstrap = candidate;
    store = nextStore;
    selectedCollection = candidate.default_collection;
    occurrenceClosed = false;
    durability.install(candidate.revision, durable);
    syncBeforeUnloadGuard();
  };

  const installOpenedOccurrence = (opened: OpenedProjection): void => {
    const nextStore = createProjectionStore(opened.table, opened.control);
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
    !durability.snapshot().dirty ||
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
      const bytes = await host.read(selectedSavedProject);
      await installProjectBytes(bytes);
      notice = {
        tone: "success",
        title: "Project opened",
        message: `${selectedSavedProject} is current in a fresh Rust occurrence.`,
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
      await host.publish(requestedName, project.bytes);
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
        }.${refreshWarning}`,
        diagnostics: [],
      };
    } catch (error) {
      showProjectFailure("Project not saved", error);
    } finally {
      busy = false;
      render();
    }
  };

  const closeOccurrence = async (): Promise<void> => {
    if (busy) return;
    if (!confirmDiscardDirtyOccurrence("Close")) return;
    busy = true;
    notice = null;
    render();
    try {
      await client.closeProject();
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
    root.querySelectorAll<HTMLFormElement>("[data-edit-form]").forEach((form) => {
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
            const value =
              initialText !== undefined &&
              initialNormalized !== undefined &&
              control instanceof HTMLTextAreaElement
                ? preserveUneditedLineEndings(initialText, initialNormalized, control.value)
                : control.value;
            void commitText({ entity, field }, value);
            break;
          }
          case "boolean":
            if (!(control instanceof HTMLInputElement)) return;
            void commitBoolean({ entity, field }, control.checked);
            break;
        }
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

  const hydrateTextareas = (): void => {
    root.querySelectorAll<HTMLTextAreaElement>("textarea[data-initial-text]").forEach(
      (textarea) => {
        const initialText = decodeOpaqueAttribute(textarea.dataset.initialText);
        if (initialText !== undefined) {
          textarea.value = initialText;
          textarea.dataset.initialNormalized = textarea.value;
        }
      },
    );
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
  control: ControlProjection,
  currentness: "current" | "refreshing" | "refresh_failed",
  selectedCollection: string,
  notice: Notice | null,
  busy: boolean,
  dirty: boolean,
  durableRevision: string | null,
  savedProjects: SavedProjectSummary[],
  selectedSavedProject: string,
): string {
  const statusLabel = {
    current: "Semantic current",
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
          <p class="eyebrow">Game balance workspace</p>
          <h1>${escapeHtml(bootstrap.title)}</h1>
        </div>
        <div class="revision-chip" data-currentness="${currentness}">
          <span>${statusLabel}</span>
          <code data-testid="revision">${escapeHtml(table.revision)}</code>
        </div>
        <div class="workspace-actions">
          <div class="durability-chip" data-testid="durability" data-dirty="${String(dirty)}">
            <span>${dirty ? "Unsaved changes" : "Saved"}</span>
            <code>${durableRevision === null ? "No durable revision" : escapeHtml(durableRevision)}</code>
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

          <section class="control-witness" aria-labelledby="control-title">
            <p class="eyebrow">Unrelated control</p>
            <h2 id="control-title">Upgrade cost</h2>
            <output data-testid="control-value">${formatNumber(control.value)}</output>
            <p>Carried forward because this field was not invalidated.</p>
            <code>${escapeHtml(control.target.entity)}.${escapeHtml(control.target.field)}</code>
          </section>
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
                  .map((row) => rowMarkup(row, table, busy || currentness !== "current"))
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
): string {
  const fields = new Map(row.fields.map((field) => [field.target.field, field]));
  return `
    <tr>
      <th scope="row">
        <strong>${escapeHtml(humanize(row.key))}</strong>
        <code>${escapeHtml(row.id)}</code>
      </th>
      ${table.columns
        .map((column) => fieldMarkup(fields.get(column.id), row.key, busy))
        .join("")}
    </tr>
  `;
}

function fieldMarkup(
  field: FieldProjection | undefined,
  entityKey: string,
  busy: boolean,
): string {
  if (field === undefined) return '<td class="empty-cell">—</td>';
  const key = `${field.target.entity}.${field.target.field}`;
  const diagnostics = field.diagnostics
    .map((diagnostic) => `<small class="field-error">${escapeHtml(diagnostic.message)}</small>`)
    .join("");
  if (field.editable_scalar === "number" && field.stored?.kind === "number") {
    return `
      <td data-field="${escapeHtml(key)}" class="stored-cell">
        <form data-edit-form data-entity="${encodeOpaqueAttribute(
          field.target.entity,
        )}" data-field="${encodeOpaqueAttribute(field.target.field)}" data-edit-kind="number">
          <input
            type="number"
            step="any"
            value="${String(field.stored.value)}"
            aria-label="${escapeHtml(humanize(field.target.field))} for ${escapeHtml(
              humanize(entityKey),
            )}"
            ${busy ? "disabled" : ""}
          />
          <button type="submit" ${busy ? "disabled" : ""}>Apply</button>
        </form>
        <small class="value-kind">Stored · Number</small>
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
            aria-label="${escapeHtml(humanize(field.target.field))} for ${escapeHtml(
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
            ${field.stored.value ? "checked" : ""}
            aria-label="${escapeHtml(humanize(field.target.field))} for ${escapeHtml(
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
  if (field.formula !== null) {
    return `
      <td data-field="${escapeHtml(key)}" class="formula-cell">
        <output>${escapeHtml(calculationValue(field))}</output>
        <span class="formula-badge">ƒ Calculated</span>
        <code title="${escapeHtml(field.formula.source)}">${escapeHtml(
          field.formula.source,
        )}</code>
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

function calculationValue(field: FieldProjection): string {
  if (field.calculated?.status === "value") return formatNumber(field.calculated.value);
  if (field.calculated?.status === "failure") return field.calculated.message;
  return "Unavailable";
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

function formatNumber(value: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 4 }).format(value);
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

function preserveUneditedLineEndings(
  original: string,
  normalizedOriginal: string,
  edited: string,
): string {
  const originalLineEndings = original.match(/\r\n|\r|\n/g) ?? [];
  const editedLineEndingCount = (edited.match(/\n/g) ?? []).length;
  if (
    originalLineEndings.length === editedLineEndingCount &&
    normalizeLineEndings(original) === normalizedOriginal
  ) {
    let lineEndingIndex = 0;
    return edited.replace(/\n/g, () => {
      const lineEnding = originalLineEndings[lineEndingIndex++];
      return lineEnding ?? "\n";
    });
  }

  let prefixLength = 0;
  const sharedLength = Math.min(normalizedOriginal.length, edited.length);
  while (
    prefixLength < sharedLength &&
    normalizedOriginal[prefixLength] === edited[prefixLength]
  ) {
    prefixLength += 1;
  }

  let suffixLength = 0;
  while (
    suffixLength < normalizedOriginal.length - prefixLength &&
    suffixLength < edited.length - prefixLength &&
    normalizedOriginal[normalizedOriginal.length - suffixLength - 1] ===
      edited[edited.length - suffixLength - 1]
  ) {
    suffixLength += 1;
  }

  const prefixEnd = originalOffsetForNormalizedLength(original, prefixLength);
  const suffixStart = originalOffsetForNormalizedLength(
    original,
    normalizedOriginal.length - suffixLength,
  );
  return `${original.slice(0, prefixEnd)}${edited.slice(
    prefixLength,
    edited.length - suffixLength,
  )}${original.slice(suffixStart)}`;
}

function normalizeLineEndings(value: string): string {
  return value.replace(/\r\n|\r/g, "\n");
}

function originalOffsetForNormalizedLength(original: string, normalizedLength: number): number {
  let originalOffset = 0;
  let normalizedOffset = 0;
  while (originalOffset < original.length && normalizedOffset < normalizedLength) {
    if (original[originalOffset] === "\r") {
      originalOffset += original[originalOffset + 1] === "\n" ? 2 : 1;
    } else {
      originalOffset += 1;
    }
    normalizedOffset += 1;
  }
  return originalOffset;
}
