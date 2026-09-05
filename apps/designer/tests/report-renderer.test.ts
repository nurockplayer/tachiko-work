// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";

import { formatReportNumber, renderReportChart } from "../src/report-renderer.ts";
import type { ReadyReportChart } from "../src/report-model.ts";

const ready = (kind: "column" | "line" = "line"): ReadyReportChart => ({
  status: "ready",
  revision: "resident/8",
  chart: {
    id: "00000000-0000-4000-8000-000000000260",
    collectionId: "people",
    entityIds: ["a", "b", "c"],
    categoryFieldId: null,
    series: [{ fieldId: "value", label: "Value" }],
    kind,
    title: "Extreme values",
    xLabel: "Subjects",
    yLabel: "Amount",
    legend: true,
  },
  labels: ["A", "B", "A very long category label that wraps safely"],
  series: [{ label: "Value", values: [Number.MAX_VALUE, Number.MIN_VALUE, -Number.MAX_VALUE] }],
  numberFormat: "number",
  note: null,
});

describe("report chart Canvas renderer", () => {
  it("formats tiny and huge finite numbers without rounded zero or vast labels", () => {
    expect(formatReportNumber(Number.MIN_VALUE)).toContain("e-");
    expect(formatReportNumber(Number.MAX_VALUE)).toContain("e+");
    expect(formatReportNumber(0.00001)).toContain("e-");
    expect(formatReportNumber(0, "currency-jpy")).toMatch(/[¥￥]/);
    expect(formatReportNumber(0.125, "percentage")).toBe("12.5%");
  });

  it("keeps every plotted coordinate finite and draws wrapped labels for column and line charts", () => {
    const coordinates: number[] = [];
    const text: string[] = [];
    const context = {
      measureText: (value: string) => ({ width: Array.from(value).length * 8 }),
      fillText: (value: string, x: number, y: number) => { text.push(value); coordinates.push(x, y); },
      fillRect: (...args: number[]) => { void args; },
      beginPath: () => undefined,
      moveTo: (x: number, y: number) => { coordinates.push(x, y); },
      lineTo: (x: number, y: number) => { coordinates.push(x, y); },
      stroke: () => undefined,
      save: () => undefined,
      restore: () => undefined,
      translate: (x: number, y: number) => { coordinates.push(x, y); },
      rotate: (angle: number) => { void angle; },
      fillStyle: "",
      strokeStyle: "",
      lineWidth: 1,
      font: "",
      textAlign: "left" as CanvasTextAlign,
      textBaseline: "top" as CanvasTextBaseline,
    } as unknown as CanvasRenderingContext2D;
    const spy = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => context);
    try {
      const lineCanvas = renderReportChart(ready("line"));
      const columnCanvas = renderReportChart(ready("column"));
      expect(lineCanvas.width).toBe(1600);
      expect(columnCanvas.height).toBeGreaterThan(500);
      expect(coordinates.every(Number.isFinite)).toBe(true);
      expect(text.join(" ")).toContain("A very long category");
      expect(text.some(value => value.includes("e+"))).toBe(true);
    } finally {
      spy.mockRestore();
    }
  });

  it("allocates enough height for eighty-wide CJK labels measured with each matching font", () => {
    const wide = ready("line");
    const label = "界".repeat(80);
    wide.chart.title = label;
    wide.chart.xLabel = label;
    wide.chart.yLabel = label;
    wide.chart.series[0]!.label = label;
    wide.labels = [label, label, label];
    let currentFont = "";
    let drewText = false;
    const drawn: Array<{ value: string; x: number; y: number; font: string }> = [];
    const context = {
      get font() { return currentFont; },
      set font(value: string) { currentFont = value; },
      measureText: (value: string) => {
        const glyphWidth = currentFont.includes("26px") ? 14 : currentFont.includes("14px") ? 12 : 10;
        return { width: Array.from(value).length * glyphWidth };
      },
      fillText: (value: string, x: number, y: number) => { drewText = true; drawn.push({ value, x, y, font: currentFont }); },
      fillRect: vi.fn(),
      beginPath: vi.fn(),
      moveTo: vi.fn(),
      lineTo: vi.fn(),
      stroke: vi.fn(),
      save: vi.fn(),
      restore: vi.fn(),
      translate: vi.fn(),
      rotate: vi.fn(),
      fillStyle: "",
      strokeStyle: "",
      lineWidth: 1,
      textAlign: "left" as CanvasTextAlign,
      textBaseline: "top" as CanvasTextBaseline,
    } as unknown as CanvasRenderingContext2D;
    const spy = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => context);
    try {
      const canvas = renderReportChart(wide);
      expect(canvas.height).toBeGreaterThan(1200);
      expect(drewText).toBe(true);
      expect(drawn.filter(item => item.value.length > 0).every(item => Number.isFinite(item.x) && Number.isFinite(item.y))).toBe(true);
    } finally {
      spy.mockRestore();
    }
  });

  it("measures extreme currency, USD and percentage ticks before choosing the plot margin", () => {
    let currentFont = "";
    const drawn: Array<{ value: string; x: number; font: string }> = [];
    const context = {
      get font() { return currentFont; },
      set font(value: string) { currentFont = value; },
      measureText: (value: string) => ({ width: Array.from(value).length * (currentFont.includes("14px") ? 12 : 10) }),
      fillText: (value: string, x: number) => { drawn.push({ value, x, font: currentFont }); },
      fillRect: (...args: number[]) => { void args; },
      beginPath: () => undefined,
      moveTo: () => undefined,
      lineTo: () => undefined,
      stroke: () => undefined,
      save: () => undefined,
      restore: () => undefined,
      translate: () => undefined,
      rotate: () => undefined,
      fillStyle: "",
      strokeStyle: "",
      lineWidth: 1,
      textAlign: "left" as CanvasTextAlign,
      textBaseline: "top" as CanvasTextBaseline,
    } as unknown as CanvasRenderingContext2D;
    const spy = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => context);
    try {
      for (const format of ["currency-jpy", "currency-usd", "percentage"] as const) {
        drawn.length = 0;
        renderReportChart({ ...ready("line"), numberFormat: format });
        const extremeTicks = drawn.filter(item => item.value.includes("e+") || item.value.includes("×100%"));
        expect(extremeTicks.length).toBeGreaterThan(0);
        expect(extremeTicks.every(item => item.x - Array.from(item.value).length * 10 >= 0)).toBe(true);
      }
    } finally {
      spy.mockRestore();
    }
  });
});
