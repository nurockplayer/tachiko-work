import { parseReportCharts, projectReportChart, type ReadyReportChart, type ReportChart, type ReportChartProjection } from "./report-model.ts";
import { formatReportNumber, renderReportChart } from "./report-renderer.ts";
import type { NumberFormat } from "./tracker-model.ts";
import type { TableProjection } from "./runtime/protocol.ts";

export type ReportPanelState = {
  draft: { chart: ReportChart; creating: boolean } | null;
};

export type ReportPanelOptions = {
  table: TableProjection;
  charts: ReportChart[];
  formats: Record<string, NumberFormat>;
  collectionIds: readonly string[];
  current: boolean;
  busy: boolean;
  state: ReportPanelState;
  onChartsChange: (charts: ReportChart[]) => void;
  onDraftChange: () => void;
  rerender: () => void;
  onExport: (chart: ReportChart) => Promise<void>;
};

function element<K extends keyof HTMLElementTagNameMap>(tag: K, text = ""): HTMLElementTagNameMap[K] {
  const result = document.createElement(tag);
  result.textContent = text;
  return result;
}

function appendText(parent: Element, tag: keyof HTMLElementTagNameMap, text: string): HTMLElement {
  const child = element(tag as never, text);
  parent.append(child);
  return child;
}

function fieldLabel(column: TableProjection["columns"][number]): string {
  const key = column.key.trim();
  return key.split("_").map(part => part.slice(0, 1).toUpperCase() + part.slice(1)).join(" ") || "Unnamed field";
}

function labelled(parent: HTMLElement, name: string, control: HTMLElement): void {
  const wrapper = element("label", name);
  wrapper.className = "report-control-label";
  control.setAttribute("aria-label", name);
  wrapper.append(control);
  parent.append(wrapper);
}

function textInput(parent: HTMLElement, name: string, value: string): HTMLInputElement {
  const control = element("input");
  control.type = "text";
  control.value = value;
  labelled(parent, name, control);
  return control;
}

function selectInput(parent: HTMLElement, name: string, options: Array<{ value: string; label: string }>, value: string): HTMLSelectElement {
  const control = element("select");
  for (const option of options) {
    const item = element("option", option.label);
    item.value = option.value;
    control.append(item);
  }
  control.value = value;
  labelled(parent, name, control);
  return control;
}

function chartId(): string { return crypto.randomUUID(); }

function textValue(row: TableProjection["rows"][number]): string | null {
  for (const field of row.fields) {
    if (field.stored?.kind === "text" && field.stored.value.trim()) return field.stored.value;
  }
  return null;
}

function rowLabel(table: TableProjection, row: TableProjection["rows"][number], index: number): string {
  const value = textValue(row);
  return value === null ? `Row ${String(index + 1)}` : `${value} (Row ${String(index + 1)})`;
}

function sourceOptions(table: TableProjection): Array<{ value: string; label: string }> {
  return [{ value: table.collection.id, label: table.collection.key.trim() || "Current table" }];
}

function numericColumns(table: TableProjection): TableProjection["columns"] {
  return table.columns.filter(column => column.field_type === "number");
}

function categoryColumns(table: TableProjection): TableProjection["columns"] {
  return table.columns.filter(column => !["reference", "ref"].includes(column.field_type.toLowerCase()));
}

function cloneChart(chart: ReportChart): ReportChart {
  return {
    ...chart,
    entityIds: [...chart.entityIds],
    series: chart.series.map(item => ({ ...item })),
  };
}

function reasonText(value: unknown): string {
  const reason = typeof value === "string" && value.trim() ? value.trim() : "The source values are unavailable for this chart.";
  // Projection diagnostics are user-facing prose. Strip common opaque IDs if a
  // lower layer accidentally includes one; stable bindings must never become UI text.
  return reason
    .replace(/Numeric series field '[^']+' is unavailable\./g, "A numeric series field is unavailable.")
    .replace(/\b[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/gi, "an unavailable source")
    .replace(/\b(?:stable|entity|field|collection)[-_][A-Za-z0-9_-]+\b/g, "an unavailable source");
}

function renderReadyCard(parent: HTMLElement, chart: ReportChart, projection: ReadyReportChart, options: ReportPanelOptions): void {
  const card = element("article");
  card.className = "report-card";
  const title = appendText(card, "h4", chart.title);
  title.className = "report-card-title";
  const preview = element("div");
  preview.className = "report-preview";
  let canvas: HTMLCanvasElement;
  try {
    canvas = renderReportChart(projection);
  } catch (error) {
    renderUnavailableCard(parent, chart, `Chart rendering is unavailable: ${reasonText(error instanceof Error ? error.message : error)}`);
    return;
  }
  canvas.setAttribute("role", "img");
  canvas.setAttribute("aria-label", chart.title);
  preview.append(canvas);
  card.append(preview);
  const labels = projection.labels;
  const series = projection.series.map(item => ({ label: item.label, values: item.values, format: projection.numberFormat }));
  const data = element("table");
  data.className = "report-data-table";
  const caption = element("caption", `${chart.title} data`);
  data.append(caption);
  const head = element("thead");
  const heading = element("tr");
  heading.append(element("th", chart.xLabel.trim() || "Row"));
  for (const item of series) heading.append(element("th", item.label));
  head.append(heading);
  data.append(head);
  const body = element("tbody");
  for (let index = 0; index < labels.length; index += 1) {
    const row = element("tr");
    row.append(element("th", labels[index] ?? `Row ${String(index + 1)}`));
    for (const item of series) {
      const raw = item.values[index];
      const value = typeof raw === "number" && Number.isFinite(raw)
        ? formatReportNumber(raw, item.format)
        : "Unavailable";
      row.append(element("td", value));
    }
    body.append(row);
  }
  data.append(body);
  card.append(data);
  const exportButton = element("button", "Download PNG");
  exportButton.type = "button";
  exportButton.disabled = options.busy || !options.current || options.state.draft !== null;
  exportButton.addEventListener("click", () => {
    if (exportButton.disabled) return;
    exportButton.disabled = true;
    void options.onExport(chart).catch((error: unknown) => {
      card.append(element("p", `PNG export unavailable: ${reasonText(error instanceof Error ? error.message : error)}`));
    }).finally(() => { exportButton.disabled = options.busy || !options.current || options.state.draft !== null; });
  });
  card.append(exportButton);
  parent.append(card);
}

function renderUnavailableCard(parent: HTMLElement, chart: ReportChart, reason: unknown): void {
  const card = element("article");
  card.className = "report-card report-card-unavailable";
  card.append(element("h4", chart.title));
  card.append(element("p", `Chart unavailable: ${reasonText(reason)}`));
  const exportButton = element("button", "Download PNG");
  exportButton.type = "button";
  exportButton.disabled = true;
  card.append(exportButton);
  parent.append(card);
}

function draftDefaults(table: TableProjection, collectionId: string): ReportChart {
  const numeric = numericColumns(table);
  return {
    id: chartId(),
    collectionId,
    entityIds: table.rows.slice(0, 16).map(row => row.id),
    categoryFieldId: null,
    series: numeric.slice(0, 1).map(column => ({ fieldId: column.id, label: fieldLabel(column) })),
    kind: "column",
    title: "Report",
    xLabel: "",
    yLabel: "",
    legend: true,
  };
}

function addSeriesEditor(editor: HTMLElement, chart: ReportChart, table: TableProjection, options: ReportPanelOptions): void {
  const numeric = numericColumns(table);
  const seriesFieldOptions = numeric.map(column => ({ value: column.id, label: fieldLabel(column) }));
  const seriesSection = element("fieldset");
  seriesSection.className = "report-series-editor";
  seriesSection.append(element("legend", "Numeric series (up to 3)"));
  const redraw = (): void => {
    seriesSection.querySelectorAll(".report-series-row").forEach(row => { row.remove(); });
    seriesSection.querySelectorAll(".report-add-series").forEach(button => { button.remove(); });
    for (let index = 0; index < chart.series.length; index += 1) {
      const item = chart.series[index];
      if (!item) continue;
      const row = element("div");
      row.className = "report-series-row";
      const field = selectInput(row, `Series ${String(index + 1)} field`, seriesFieldOptions, item.fieldId);
      field.addEventListener("change", () => { item.fieldId = field.value; options.onDraftChange(); });
      const label = textInput(row, `Series ${String(index + 1)} label`, item.label);
      label.addEventListener("input", () => { item.label = label.value; options.onDraftChange(); });
      const remove = element("button", "Remove series");
      remove.type = "button";
      remove.addEventListener("click", () => { chart.series.splice(index, 1); redraw(); options.onDraftChange(); });
      row.append(remove);
      seriesSection.append(row);
    }
    const add = element("button", "Add numeric series");
    add.className = "report-add-series";
    add.type = "button";
    add.disabled = chart.series.length >= 3 || numeric.length <= chart.series.length;
    add.addEventListener("click", () => {
      const used = new Set(chart.series.map(item => item.fieldId));
      const next = numeric.find(column => !used.has(column.id));
      if (!next || chart.series.length >= 3) return;
      chart.series.push({ fieldId: next.id, label: fieldLabel(next) });
      redraw();
      options.onDraftChange();
    });
    seriesSection.append(add);
  };
  redraw();
  editor.append(seriesSection);
}

function renderEditor(parent: HTMLElement, options: ReportPanelOptions): void {
  const draft = options.state.draft;
  if (!draft) return;
  const editor = element("form");
  editor.className = "report-editor";
  editor.append(element("h3", draft.creating ? "Create report chart" : "Edit report chart"));
  const chart = draft.chart;
  const source = selectInput(editor, "Chart source", sourceOptions(options.table), chart.collectionId);
  source.disabled = true;
  editor.append(element("p", "Chart source is the current table. Switch tables to choose a different source."));
  const title = textInput(editor, "Chart title", chart.title);
  title.addEventListener("input", () => { chart.title = title.value; options.onDraftChange(); });
  const kind = selectInput(editor, "Chart type", [{ value: "column", label: "Column" }, { value: "line", label: "Line" }], chart.kind);
  kind.addEventListener("change", () => { chart.kind = kind.value as ReportChart["kind"]; options.onDraftChange(); });
  const categoryOptions = [{ value: "", label: "Row labels" }, ...categoryColumns(options.table).map(column => ({ value: column.id, label: fieldLabel(column) }))];
  const category = selectInput(editor, "Category field", categoryOptions, chart.categoryFieldId ?? "");
  category.addEventListener("change", () => { chart.categoryFieldId = category.value || null; options.onDraftChange(); });
  const xLabel = textInput(editor, "X axis label", chart.xLabel);
  xLabel.addEventListener("input", () => { chart.xLabel = xLabel.value; options.onDraftChange(); });
  const yLabel = textInput(editor, "Y axis label", chart.yLabel);
  yLabel.addEventListener("input", () => { chart.yLabel = yLabel.value; options.onDraftChange(); });
  const legend = element("input");
  legend.type = "checkbox";
  legend.checked = chart.legend;
  labelled(editor, "Show legend", legend);
  legend.addEventListener("change", () => { chart.legend = legend.checked; options.onDraftChange(); });
  if (numericColumns(options.table).length === 0) {
    editor.append(element("p", "This table has no Number columns. Choose the Budget workflow or a table with numeric columns to make a chart."));
  } else {
    addSeriesEditor(editor, chart, options.table, options);
  }
  const rows = element("fieldset");
  rows.className = "report-row-editor";
  rows.append(element("legend", "Rows (up to 16; saved order is preserved)"));
  for (const [index, row] of options.table.rows.entries()) {
    const check = element("input");
    check.type = "checkbox";
    check.checked = chart.entityIds.includes(row.id);
    check.disabled = !check.checked && chart.entityIds.length >= 16;
    check.addEventListener("change", () => {
      if (check.checked) {
        if (!chart.entityIds.includes(row.id) && chart.entityIds.length < 16) chart.entityIds.push(row.id);
        else if (chart.entityIds.length >= 16) check.checked = false;
      } else {
        const position = chart.entityIds.indexOf(row.id);
        if (position >= 0) chart.entityIds.splice(position, 1);
      }
      options.onDraftChange();
      for (const input of rows.querySelectorAll<HTMLInputElement>("input[type=checkbox]")) {
        input.disabled = !input.checked && chart.entityIds.length >= 16;
      }
    });
    const wrapper = element("label", rowLabel(options.table, row, index));
    wrapper.className = "report-row-choice";
    wrapper.prepend(check);
    rows.append(wrapper);
  }
  editor.append(rows);
  const actions = element("div");
  actions.className = "report-editor-actions";
  const apply = element("button", "Apply chart");
  apply.type = "submit";
  apply.disabled = options.busy || !options.current;
  const applyCandidate = (): void => {
    if (options.busy || !options.current) return;
    try {
      if (chart.collectionId !== options.table.collection.id) throw new Error("The chart source changed. Cancel this draft and select its source again.");
      const candidate = [...options.charts.filter(item => item.id !== chart.id), cloneChart(chart)];
      const parsed = parseReportCharts(candidate, [...options.collectionIds]);
      options.onChartsChange(parsed);
    } catch (error) {
      editor.append(element("p", `Chart could not be applied: ${reasonText(error instanceof Error ? error.message : error)}`));
    }
  };
  // Prevent a browser's native submit navigation and keep the same behaviour
  // for happy-dom, whose synthetic button click does not submit every form.
  apply.addEventListener("click", event => { event.preventDefault(); applyCandidate(); });
  const cancel = element("button", "Cancel");
  cancel.type = "button";
  cancel.addEventListener("click", () => { options.state.draft = null; options.onDraftChange(); options.rerender(); });
  actions.append(apply, cancel);
  editor.append(actions);
  editor.addEventListener("submit", event => { event.preventDefault(); applyCandidate(); });
  parent.append(editor);
}

export function mountReportPanel(host: HTMLElement, options: ReportPanelOptions): void {
  const section = element("section");
  section.className = "report-panel";
  section.setAttribute("aria-label", "Report charts");
  section.append(element("h2", "Report charts"));
  section.append(element("p", "Charts are private presentation settings, with up to 8 charts, 16 rows and 3 numeric series per chart. Static PNGs are not editable, live-linked, project backups, or preserved in XLSX downloads."));
  const hasDraft = options.state.draft !== null;
  if (hasDraft) renderEditor(section, options);
  else {
    const numeric = numericColumns(options.table);
    const controls = element("div");
    controls.className = "report-panel-actions";
    const create = element("button", "Create chart from selected source");
    create.type = "button";
    create.disabled = options.busy || !options.current || options.charts.length >= 8 || numeric.length === 0 || options.table.rows.length === 0 || options.collectionIds.length === 0 || !options.collectionIds.includes(options.table.collection.id);
    create.addEventListener("click", () => {
      if (create.disabled) return;
      options.state.draft = { chart: draftDefaults(options.table, options.table.collection.id), creating: true };
      options.onDraftChange();
      options.rerender();
    });
    controls.append(create);
    section.append(controls);
    if (numeric.length === 0) section.append(element("p", "No chart can be created because this table has no Number columns. Choose the Budget workflow or a table with numeric columns."));
    else if (options.table.rows.length === 0) section.append(element("p", "No chart can be created because this table has no rows to select."));
  }
  const visible = options.charts.filter(chart => chart.collectionId === options.table.collection.id);
  if (!hasDraft && visible.length === 0) section.append(element("p", "No saved charts for this source yet."));
  const cards = element("div");
  cards.className = "report-card-list";
  for (const chart of visible) {
    if (!options.current) {
      renderUnavailableCard(cards, chart, "The source is waiting for current data. Apply or discard pending data edits, then refresh the chart.");
    } else {
      let projection: ReportChartProjection;
      try { projection = projectReportChart(chart, options.table, options.formats); }
      catch (error) {
        renderUnavailableCard(cards, chart, error instanceof Error ? error.message : error);
        projection = { status: "unavailable", message: "Chart projection is unavailable." };
      }
      if (projection.status === "ready") renderReadyCard(cards, chart, projection, options);
      else if (projection.message !== "Chart projection is unavailable.") renderUnavailableCard(cards, chart, projection.message);
    }
    const card = cards.lastElementChild;
    if (!card) continue;
    const actions = element("div");
    actions.className = "report-card-actions";
    const edit = element("button", "Edit chart");
    edit.type = "button";
    edit.disabled = options.busy || !options.current || options.state.draft !== null;
    edit.addEventListener("click", () => { options.state.draft = { chart: cloneChart(chart), creating: false }; options.onDraftChange(); options.rerender(); });
    const remove = element("button", "Delete chart");
    remove.type = "button";
    remove.disabled = options.busy || !options.current || options.state.draft !== null;
    remove.addEventListener("click", () => { options.onChartsChange(options.charts.filter(item => item.id !== chart.id)); });
    actions.append(edit, remove);
    card.append(actions);
  }
  section.append(cards);
  host.append(section);
}
