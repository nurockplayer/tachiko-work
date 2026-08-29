import {
  createProjectionStore,
  type ControlProjection,
  type ProjectionStore,
} from "./projection-store.ts";
import {
  DesignerRuntimeError,
  type DesignerClient,
} from "./runtime/client.ts";
import type {
  BootstrapProjection,
  DiagnosticProjection,
  FieldProjection,
  FieldTarget,
  TableProjection,
} from "./runtime/protocol.ts";

type Notice = {
  tone: "error" | "success";
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
): MountedDesigner {
  let bootstrap: BootstrapProjection | null = null;
  let store: ProjectionStore | null = null;
  let selectedCollection = "";
  let notice: Notice | null = null;
  let startupFailure: string | null = null;
  let busy = false;
  let destroyed = false;

  const render = (): void => {
    if (destroyed) return;
    if (bootstrap === null || store === null) {
      root.innerHTML =
        startupFailure === null ? loadingMarkup() : startupFailureMarkup(startupFailure);
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
    );
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
      message: failure.message,
      diagnostics: failure.diagnostics,
    };
    if (published) {
      store?.failRefresh(failure.message);
    }
  };

  const commitNumber = async (
    target: FieldTarget,
    input: string,
  ): Promise<void> => {
    if (store === null || busy) return;
    busy = true;
    notice = null;
    render();
    let published = false;
    try {
      const publication = await client.editNumber(
        store.snapshot().table.revision,
        target,
        input,
      );
      published = true;
      const requested = store.beginPublication(publication);
      render();
      const refresh = await client.queryFields(
        publication.resulting_revision,
        requested,
      );
      store.finishRefresh(refresh);
      notice = {
        tone: "success",
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

  const selectCollection = async (collection: string): Promise<void> => {
    if (bootstrap === null || store === null || busy) return;
    busy = true;
    notice = null;
    render();
    try {
      const table = await client.queryTable(collection);
      const control = store.snapshot().control;
      if (table.revision !== control.revision) {
        throw new Error("Collection query returned a different semantic revision.");
      }
      store = createProjectionStore(table, control);
      selectedCollection = collection;
    } catch (error) {
      showFailure(error, false);
    } finally {
      busy = false;
      render();
    }
  };

  const bindInteractions = (): void => {
    root.querySelectorAll<HTMLFormElement>("[data-edit-form]").forEach((form) => {
      form.addEventListener("submit", (event) => {
        event.preventDefault();
        const input = form.querySelector<HTMLInputElement>("input");
        const entity = form.dataset.entity;
        const field = form.dataset.field;
        if (input === null || entity === undefined || field === undefined) return;
        void commitNumber({ entity, field }, input.value);
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
  };

  render();
  const ready = (async () => {
    try {
      bootstrap = await client.bootstrap();
      selectedCollection = bootstrap.default_collection;
      const [table, controlBatch] = await Promise.all([
        client.queryTable(selectedCollection),
        client.queryFields(bootstrap.revision, [bootstrap.control_field]),
      ]);
      if (table.revision !== bootstrap.revision || controlBatch.revision !== bootstrap.revision) {
        throw new Error("Initial projections do not share one semantic revision.");
      }
      const controlField = controlBatch.fields[0];
      if (controlField?.calculated?.status !== "value") {
        throw new Error("The unrelated control projection is unavailable.");
      }
      store = createProjectionStore(table, {
        target: bootstrap.control_field,
        value: controlField.calculated.value,
        revision: bootstrap.revision,
      });
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
            <li><span>1</span><strong>Stored value</strong><small>Editable Number</small></li>
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
                ${table.rows.map((row) => rowMarkup(row, table, busy)).join("")}
              </tbody>
            </table>
          </div>
          <p class="table-footnote">Human-readable keys are shown here; edits target stable semantic IDs.</p>
        </section>
      </main>
    </div>
  `;
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
  if (field.editable_number && field.stored?.kind === "number") {
    return `
      <td data-field="${escapeHtml(key)}" class="stored-cell">
        <form data-edit-form data-entity="${escapeHtml(
          field.target.entity,
        )}" data-field="${escapeHtml(field.target.field)}">
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
      <strong>${notice.tone === "error" ? "Edit not published" : "Publication complete"}</strong>
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
