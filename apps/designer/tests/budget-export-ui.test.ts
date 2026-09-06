import { afterEach, describe, expect, it, vi } from "vitest";

import { mountDesigner } from "../src/designer-app.ts";
import type { DesignerClient } from "../src/runtime/client.ts";
import type {
  NativeBudgetExportPresentation,
  SpreadsheetExport,
  SpreadsheetFormat,
} from "../src/runtime/interop-protocol.ts";
import type { DesignerProjectHost } from "../src/host/browser-project-host.ts";
import type {
  BootstrapProjection,
  FieldBatchProjection,
  FieldProjection,
  OpenedProjection,
  ProjectExport,
  PublicationProjection,
  TableProjection,
} from "../src/runtime/protocol.ts";

let destroy: (() => void) | undefined;

afterEach(() => {
  destroy?.();
  destroy = undefined;
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const bootstrap: BootstrapProjection = {
  title: "Monthly Budget",
  revision: "resident/0",
  default_collection: "budget_items",
  collections: [
    { id: "budget_items", key: "budget_items", entity_count: 2 },
    { id: "budget_summary", key: "budget_summary", entity_count: 1 },
  ],
};

function numberField(entity: string, field: string, value: number): FieldProjection {
  return {
    target: { entity, field },
    address: `${entity}.${field}`,
    stored: { kind: "number", value },
    formula: null,
    calculated: null,
    diagnostics: [],
    editable_scalar: "number",
  };
}

function budgetTable(collection: "budget_items" | "budget_summary", revision: string): TableProjection {
  if (collection === "budget_summary") {
    return {
      revision,
      collection: { id: collection, key: collection, entity_count: 1 },
      columns: [{ id: "actual_total", key: "actual_total", field_type: "number" }],
      rows: [{ id: "monthly_summary", key: "monthly_summary", fields: [numberField("monthly_summary", "actual_total", 1360)] }],
    };
  }
  return {
    revision,
    collection: { id: collection, key: collection, entity_count: 2 },
    columns: [
      { id: "actual", key: "actual", field_type: "number" },
      { id: "variance", key: "variance", field_type: "number" },
    ],
    rows: [
      {
        id: "rent",
        key: "rent",
        fields: [numberField("rent", "actual", 1200), numberField("rent", "variance", 0)],
      },
      {
        id: "utilities",
        key: "utilities",
        fields: [numberField("utilities", "actual", 160), numberField("utilities", "variance", -20)],
      },
    ],
  };
}

function opened(revision = "resident/0"): OpenedProjection {
  return {
    bootstrap: structuredClone({ ...bootstrap, revision }),
    table: budgetTable("budget_items", revision),
  };
}

function setup() {
  let revision = "resident/0";
  const queryTable = vi.fn(async (collection: string): Promise<TableProjection> =>
    budgetTable(collection === "budget_summary" ? "budget_summary" : "budget_items", revision),
  );
  const exportNativeBudgetSpreadsheet = vi.fn(async (
    expectedRevision: string,
    presentation: NativeBudgetExportPresentation,
    format: SpreadsheetFormat,
  ): Promise<SpreadsheetExport> => {
    void presentation;
    void format;
    return {
      revision: expectedRevision,
      bytes: new Uint8Array([0x50, 0x4b, 0x03, 0x04]).buffer,
      ledger: [{
        category: "lossy_on_export",
        code: "native_budget_session_presentation",
        location: "Budget",
        message: "Charts and browser-only view presentation are not exported.",
        blocking: false,
      }],
    };
  });
  const editNumber = vi.fn(async (expectedRevision: string): Promise<PublicationProjection> => {
    revision = "resident/1";
    return {
      base_revision: expectedRevision,
      resulting_revision: revision,
      entities: [],
      fields: [],
      affected_calculations: [],
    };
  });
  const exportProject = vi.fn(async (expectedRevision: string): Promise<ProjectExport> => ({
    revision: expectedRevision,
    bytes: new ArrayBuffer(8),
  }));
  const queryFields = vi.fn(async (expectedRevision: string, fields: FieldProjection["target"][]): Promise<FieldBatchProjection> => {
    void fields;
    return { revision: expectedRevision, fields: [] };
  });
  const publish = vi.fn(async () => {});
  const client: DesignerClient = {
    bootstrap: async () => structuredClone(bootstrap),
    newBudget: async () => opened(revision),
    openProject: async () => opened(revision),
    exportProject,
    closeProject: async () => {},
    queryTable,
    queryFields,
    editNumber,
    editText: async () => ({ base_revision: revision, resulting_revision: revision, entities: [], fields: [], affected_calculations: [] }),
    editBoolean: async () => ({ base_revision: revision, resulting_revision: revision, entities: [], fields: [], affected_calculations: [] }),
    editDate: async () => ({ base_revision: revision, resulting_revision: revision, entities: [], fields: [], affected_calculations: [] }),
    exportNativeBudgetSpreadsheet,
    close: () => {},
  };
  const host: DesignerProjectHost = {
    list: async () => [],
    read: async () => new ArrayBuffer(8),
    publish,
  };
  const root = document.createElement("div");
  document.body.append(root);
  vi.stubGlobal("confirm", vi.fn(() => true));
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:budget-export");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
  const app = mountDesigner(root, client, host);
  destroy = () => { app.destroy(); };
  return { app, root, client, exportNativeBudgetSpreadsheet, editNumber, exportProject, publish };
}

async function openBudget(root: HTMLElement): Promise<void> {
  root.querySelector<HTMLButtonElement>("[data-new-budget]")?.click();
  await vi.waitFor(() => {
    expect(root.querySelector<HTMLButtonElement>('button[data-budget-export="xlsx"]')).not.toBeNull();
  });
}

function button(root: HTMLElement, text: string): HTMLButtonElement {
  const control = [...root.querySelectorAll<HTMLButtonElement>("button")].find(item => item.textContent === text);
  if (!control) throw new Error(`Missing button '${text}'.`);
  return control;
}

describe("native Budget export UI", () => {
  it("composes stable identities and discloses format-specific loss before download", async () => {
    const { app, root, exportNativeBudgetSpreadsheet, exportProject, publish } = setup();
    await app.ready;
    await openBudget(root);

    root.querySelector<HTMLButtonElement>('button[data-budget-export="xlsx"]')?.click();
    await vi.waitFor(() => { expect(exportNativeBudgetSpreadsheet).toHaveBeenCalledOnce(); });
    const [revision, presentation, format] = exportNativeBudgetSpreadsheet.mock.calls[0] ?? [];
    expect(revision).toBe("resident/0");
    expect(format).toBe("xlsx");
    expect((presentation as NativeBudgetExportPresentation).version).toBe(1);
    expect((presentation as NativeBudgetExportPresentation).views.map(view => view.collection_id)).toEqual([
      "budget_items",
      "budget_summary",
    ]);
    expect((presentation as NativeBudgetExportPresentation).collections.map(collection => collection.collection_id)).toEqual([
      "budget_items",
      "budget_summary",
    ]);
    expect((presentation as NativeBudgetExportPresentation).collections.every(collection =>
      collection.rows.every(row => row.styles.length > 0),
    )).toBe(true);

    const review = root.querySelector('[aria-label="Export compatibility review"]');
    expect(review?.textContent).toContain("budget_xlsx_charts_not_preserved");
    expect(review?.textContent).toContain("native_budget_session_presentation");
    expect(review?.textContent).toMatch(/chart/i);
    expect(button(root, "Acknowledge losses and download XLSX")).toBeTruthy();

    button(root, "Acknowledge losses and download XLSX").click();
    expect(exportProject).not.toHaveBeenCalled();
    expect(publish).not.toHaveBeenCalled();

    root.querySelector<HTMLButtonElement>('button[data-budget-export="csv"]')?.click();
    await vi.waitFor(() => { expect(exportNativeBudgetSpreadsheet).toHaveBeenCalledTimes(2); });
    const csvReview = root.querySelector('[aria-label="Export compatibility review"]');
    expect(csvReview?.textContent).toContain("budget_csv_calculated_values");
    expect(csvReview?.textContent).toMatch(/calculated|values/i);
    expect(csvReview?.textContent).toMatch(/formula/i);
    expect(csvReview?.textContent).toMatch(/collection|sheet/i);
  });

  it("lets a published scalar change invalidate a prepared review", async () => {
    const { app, root, exportNativeBudgetSpreadsheet, editNumber } = setup();
    await app.ready;
    await openBudget(root);
    root.querySelector<HTMLButtonElement>('button[data-budget-export="xlsx"]')?.click();
    await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });

    const actual = root.querySelector<HTMLInputElement>('input[aria-label="Actual for Utilities"]');
    if (actual?.form === null || actual === null) throw new Error("Budget scalar control is required");
    actual.value = "201";
    actual.form.requestSubmit();
    await vi.waitFor(() => { expect(root.querySelector('[data-testid="revision"]')?.textContent).toContain("resident/1"); });
    expect(editNumber).toHaveBeenCalledWith("resident/0", { entity: "utilities", field: "actual" }, "201");
    expect(exportNativeBudgetSpreadsheet).toHaveBeenCalledOnce();
    expect(root.querySelector('[aria-label="Export compatibility review"]')).toBeNull();
  });

  it("does not resurrect a prepared review after a refused scalar edit", async () => {
    const { app, root, exportNativeBudgetSpreadsheet, editNumber } = setup();
    await app.ready;
    await openBudget(root);
    root.querySelector<HTMLButtonElement>('button[data-budget-export="xlsx"]')?.click();
    await vi.waitFor(() => { expect(root.querySelector('[aria-label="Export compatibility review"]')).not.toBeNull(); });

    editNumber.mockRejectedValueOnce(new Error("The requested revision is stale."));
    const actual = root.querySelector<HTMLInputElement>('input[aria-label="Actual for Utilities"]');
    if (actual?.form === null || actual === null) throw new Error("Budget scalar control is required");
    actual.value = "201";
    actual.form.requestSubmit();
    await vi.waitFor(() => { expect(root.textContent).toContain("The requested revision is stale."); });
    expect(exportNativeBudgetSpreadsheet).toHaveBeenCalledOnce();
    expect(root.querySelector('[aria-label="Export compatibility review"]')).toBeNull();
  });
});
