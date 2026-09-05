import { cellKey, type NumberFormat } from "./tracker-model.ts";
import type { FieldProjection, TableProjection } from "./runtime/protocol.ts";

const MAX_CHARTS = 8;
const MAX_ROWS = 16;
const MAX_SERIES = 3;
const MAX_LABEL_CODE_POINTS = 80;
const MAX_ID_CODE_UNITS = 256;

export type ReportChart = {
  id: string;
  collectionId: string;
  entityIds: string[];
  categoryFieldId: string | null;
  series: Array<{ fieldId: string; label: string }>;
  kind: "column" | "line";
  title: string;
  xLabel: string;
  yLabel: string;
  legend: boolean;
};

export type ReadyReportChart = {
  status: "ready";
  revision: string;
  chart: ReportChart;
  labels: string[];
  series: Array<{ label: string; values: number[] }>;
  numberFormat: NumberFormat;
  note: string | null;
};

export type UnavailableReportChart = {
  status: "unavailable";
  message: string;
};

export type ReportChartProjection = ReadyReportChart | UnavailableReportChart;

const CHART_KEYS = [
  "id",
  "collectionId",
  "entityIds",
  "categoryFieldId",
  "series",
  "kind",
  "title",
  "xLabel",
  "yLabel",
  "legend",
] as const;
const SERIES_KEYS = ["fieldId", "label"] as const;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function assertClosed(value: Record<string, unknown>, keys: readonly string[], description: string): void {
  if (Object.keys(value).some(key => !keys.includes(key))) {
    throw new Error(`Unsupported ${description} property.`);
  }
}

function assertId(value: unknown, description: string): asserts value is string {
  if (typeof value !== "string" || value.length === 0 || value.length > MAX_ID_CODE_UNITS || value.includes("\0")) {
    throw new Error(`${description} must be a non-empty ID of at most 256 code units.`);
  }
}

function assertLabel(value: unknown, description: string, required = false): asserts value is string {
  if (typeof value !== "string" || (required && (value.trim() === "" || Array.from(value).length > MAX_LABEL_CODE_POINTS))) {
    throw new Error(`${description} must be at most 80 Unicode code points${required ? " and non-blank" : ""}.`);
  }
  if (!required && Array.from(value).length > MAX_LABEL_CODE_POINTS) {
    throw new Error(`${description} must be at most 80 Unicode code points.`);
  }
}

function parseChart(value: unknown, collectionIds: readonly string[]): ReportChart {
  if (!isRecord(value)) throw new Error("Report chart must be an object.");
  assertClosed(value, CHART_KEYS, "report chart");

  const id = value.id;
  assertId(id, "Chart ID");
  if (!UUID.test(id) || id !== id.toLowerCase()) throw new Error("Chart ID must be a canonical lowercase UUID.");

  const collectionId = value.collectionId;
  assertId(collectionId, "Chart collection ID");
  if (!collectionIds.includes(collectionId)) throw new Error("Chart collection ID is not an authoritative collection.");

  if (!Array.isArray(value.entityIds) || value.entityIds.length === 0 || value.entityIds.length > MAX_ROWS) {
    throw new Error("A report chart must select between 1 and 16 rows.");
  }
  const entityIds = value.entityIds.map((entity, index) => {
    assertId(entity, `Chart row ID ${String(index + 1)}`);
    return entity;
  });
  if (new Set(entityIds).size !== entityIds.length) throw new Error("Chart row IDs must be unique.");

  const categoryFieldId = value.categoryFieldId;
  if (categoryFieldId !== null) assertId(categoryFieldId, "Category field ID");

  if (!Array.isArray(value.series) || value.series.length === 0 || value.series.length > MAX_SERIES) {
    throw new Error("A report chart must contain between 1 and 3 numeric series.");
  }
  const series = value.series.map((item, index) => {
    if (!isRecord(item)) throw new Error(`Report series ${String(index + 1)} must be an object.`);
    assertClosed(item, SERIES_KEYS, "report series");
    const fieldId = item.fieldId;
    assertId(fieldId, `Report series ${String(index + 1)} field ID`);
    const label = item.label;
    assertLabel(label, `Report series ${String(index + 1)} label`);
    return { fieldId, label };
  });
  const fieldIds = series.map(item => item.fieldId);
  if (new Set(fieldIds).size !== fieldIds.length) throw new Error("Report series field IDs must be unique.");
  if (categoryFieldId !== null && fieldIds.includes(categoryFieldId)) {
    throw new Error("Category and numeric series field IDs must be distinct.");
  }

  if (value.kind !== "column" && value.kind !== "line") throw new Error("Report chart kind must be column or line.");
  const title = value.title;
  assertLabel(title, "Report chart title", true);
  const xLabel = value.xLabel;
  const yLabel = value.yLabel;
  assertLabel(xLabel, "Categorical axis label");
  assertLabel(yLabel, "Numeric axis label");
  if (typeof value.legend !== "boolean") throw new Error("Report chart legend must be boolean.");

  return {
    id,
    collectionId,
    entityIds,
    categoryFieldId,
    series,
    kind: value.kind,
    title,
    xLabel,
    yLabel,
    legend: value.legend,
  };
}

/** Parse the private chart list. */
export function parseReportCharts(input: unknown, collectionIds: readonly string[]): ReportChart[] {
  if (!Array.isArray(input) || input.length > MAX_CHARTS) {
    throw new Error("A project may contain at most 8 report charts.");
  }
  const charts = input.map(value => parseChart(value, collectionIds));
  if (new Set(charts.map(chart => chart.id)).size !== charts.length) throw new Error("Report chart IDs must be unique.");
  return charts;
}

function numericValue(field: FieldProjection | undefined): number | undefined {
  if (field === undefined || field.diagnostics.length > 0) return undefined;
  if (field.formula !== null) {
    return field.calculated?.status === "value" && Number.isFinite(field.calculated.value)
      ? field.calculated.value
      : undefined;
  }
  return field.stored?.kind === "number" && Number.isFinite(field.stored.value) ? field.stored.value : undefined;
}

function categoryValue(field: FieldProjection | undefined): string | undefined {
  if (field === undefined || field.diagnostics.length > 0) return undefined;
  if (field.formula !== null) {
    const value = field.calculated?.status === "value" && Number.isFinite(field.calculated.value)
      ? field.calculated.value
      : undefined;
    return value === undefined ? undefined : String(value);
  }
  const stored = field.stored;
  if (stored === null || stored.kind === "reference") return undefined;
  if (stored.kind === "number" && !Number.isFinite(stored.value)) return undefined;
  return String(stored.value);
}

function unavailable(message: string): UnavailableReportChart {
  return { status: "unavailable", message };
}

/** Project the saved bindings against one current authoritative table query. */
export function projectReportChart(
  chart: ReportChart,
  table: TableProjection,
  formats: Record<string, NumberFormat>,
): ReportChartProjection {
  if (table.collection.id !== chart.collectionId) {
    return unavailable("Report chart collection is unavailable in the current table.");
  }
  const rows = chart.entityIds.map(id => table.rows.find(row => row.id === id));
  if (rows.some(row => row === undefined)) return unavailable("A selected report chart row is unavailable.");

  const presentRows = rows.filter((row): row is TableProjection["rows"][number] => row !== undefined);
  const rowFields = presentRows.map(row => new Map(row.fields.map(field => [field.target.field, field])));
  const categoryFieldId = chart.categoryFieldId;
  if (categoryFieldId !== null && !table.columns.some(column => column.id === categoryFieldId)) {
    return unavailable("A report chart category field is unavailable.");
  }
  const labels = categoryFieldId === null
    ? chart.entityIds.map((_, index) => `Row ${String(index + 1)}`)
    : rowFields.map(fields => categoryValue(fields.get(categoryFieldId)));
  if (labels.some(label => label === undefined)) return unavailable("A report chart category value is unavailable.");
  const resolvedLabels = labels as string[];
  if (resolvedLabels.some(label => Array.from(label).length > MAX_LABEL_CODE_POINTS)) {
    return unavailable("A report chart category label exceeds 80 Unicode code points.");
  }

  const projectedSeries: Array<{ label: string; values: number[] }> = [];
  const discoveredFormats = new Set<NumberFormat>();
  for (const configured of chart.series) {
    const values: number[] = [];
    for (let index = 0; index < rowFields.length; index++) {
      const column = table.columns.find(candidate => candidate.id === configured.fieldId);
      if (column === undefined || column.field_type !== "number") {
        return unavailable(`Numeric series '${configured.label}' has an unavailable field.`);
      }
      const value = numericValue(rowFields[index]?.get(configured.fieldId));
      if (value === undefined) return unavailable(`Numeric series '${configured.label}' has an unavailable value.`);
      values.push(value);
      const entityId = chart.entityIds[index];
      if (entityId === undefined) return unavailable("Report chart row order is invalid.");
      const format = formats[cellKey(entityId, configured.fieldId)] ?? "number";
      if (["number", "percentage", "currency-jpy", "currency-usd"].includes(format)) discoveredFormats.add(format);
    }
    projectedSeries.push({ label: configured.label, values });
  }

  const mixed = discoveredFormats.size > 1;
  return {
    status: "ready",
    revision: table.revision,
    chart,
    labels: resolvedLabels,
    series: projectedSeries,
    numberFormat: mixed ? "number" : discoveredFormats.values().next().value ?? "number",
    note: mixed ? "Mixed number formats are displayed as Number without conversion." : null,
  };
}
