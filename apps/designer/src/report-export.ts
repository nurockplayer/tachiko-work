import { projectReportChart, type ReportChart } from "./report-model.ts";
import { renderReportChart } from "./report-renderer.ts";
import type { TableProjection } from "./runtime/protocol.ts";
import type { NumberFormat } from "./tracker-model.ts";

export type ReportExportState = {
  occurrence: symbol;
  alive: boolean;
  current: boolean;
  hasDrafts: boolean;
  table: TableProjection | null;
  charts: readonly ReportChart[];
  formats: Record<string, NumberFormat>;
};

/** Encode and download in one currentness-checked continuation. */
export async function downloadCurrentReport(
  chartId: string,
  readState: () => ReportExportState,
): Promise<void> {
  const initial = readState();
  const selected = initial.charts.find(chart => chart.id === chartId);
  if (!initial.alive || !initial.current || initial.hasDrafts || !initial.table || !selected) {
    throw new Error("Apply or cancel pending edits and wait for current chart data before exporting.");
  }
  const chart = structuredClone(selected);
  const formats = structuredClone(initial.formats);
  const occurrence = initial.occurrence;
  const revision = initial.table.revision;
  const collection = initial.table.collection.id;
  const projection = projectReportChart(chart, initial.table, formats);
  if (projection.status !== "ready") throw new Error(projection.message);
  const canvas = renderReportChart(projection);
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(result => {
      if (!result || result.size === 0 || result.type !== "image/png") {
        reject(new Error("The browser could not encode this chart as PNG."));
      } else resolve(result);
    }, "image/png");
  });
  const current = readState();
  if (!current.alive || !current.current || current.hasDrafts ||
      current.occurrence !== occurrence ||
      current.table?.revision !== revision || current.table.collection.id !== collection ||
      JSON.stringify(current.charts.find(value => value.id === chartId)) !== JSON.stringify(chart) ||
      JSON.stringify(current.formats) !== JSON.stringify(formats)) {
    throw new Error("The chart changed while PNG was being prepared. Export again from current data.");
  }
  // There is no await between the final check and the actual download action.
  const url = URL.createObjectURL(blob);
  try {
    const anchor = document.createElement("a");
    anchor.href = url;
    const name = Array.from(chart.title.replaceAll(/[^\p{L}\p{N}]+/gu, "-").replaceAll(/^-|-$/g, "")).slice(0, 80).join("") || "report";
    anchor.download = `${name}-static.png`;
    anchor.click();
  } finally {
    setTimeout(() => { URL.revokeObjectURL(url); }, 0);
  }
}
