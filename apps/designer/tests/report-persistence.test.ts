import { expect, it } from "vitest";
import { emptyTrackerView, parseTrackerView } from "../src/tracker-model.ts";
import type { ReportChart } from "../src/report-model.ts";

const chart: ReportChart = { id: "00000000-0000-4000-8000-000000000260", collectionId: "source", entityIds: ["deleted-row"], categoryFieldId: null, series: [{ fieldId: "deleted-field", label: "Actual" }], kind: "column", title: "Report", xLabel: "", yLabel: "", legend: true };

it("retains legacy presentation and round-trips private charts without inventing source data", () => {
  expect(parseTrackerView(JSON.stringify(emptyTrackerView()))).toEqual(emptyTrackerView());
  const saved = { ...emptyTrackerView(), charts: [chart] };
  expect(parseTrackerView(JSON.stringify(saved), ["source"])).toEqual(saved);
});

it("requires admitted project authority before accepting chart sidecars", () => {
  const saved = JSON.stringify({ ...emptyTrackerView(), charts: [chart] });
  expect(() => parseTrackerView(saved)).toThrow("authoritative project collection IDs");
  expect(() => parseTrackerView(saved, ["other"])).toThrow("authoritative");
  expect(() => parseTrackerView(JSON.stringify({ ...emptyTrackerView(), charts: { charts: [chart] } }), ["source"])).toThrow();
  expect(() => parseTrackerView(JSON.stringify({ ...emptyTrackerView(), charts: [{ ...chart, version: 2 }] }), ["source"])).toThrow();
});
