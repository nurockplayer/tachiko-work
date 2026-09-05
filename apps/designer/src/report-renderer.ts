import type { NumberFormat } from "./tracker-model.ts";
import type { ReadyReportChart } from "./report-model.ts";

const WIDTH = 1600;
const COLORS = ["#0f766e", "#c2410c", "#1d4ed8"];
const GRID = "#d6d3d1";
const INK = "#1c1917";
const MUTED = "#57534e";

const colorAt = (index: number): string => COLORS[index % COLORS.length] ?? "#0f766e";

function finite(value: number, description: string): number {
  if (!Number.isFinite(value)) throw new Error(`${description} must be finite.`);
  return value;
}

export function formatReportNumber(value: number, format: NumberFormat = "number"): string {
  finite(value, "Report chart value");
  const magnitude = Math.abs(value);
  const scientific = magnitude !== 0 && (magnitude >= 1e9 || magnitude < 1e-4);
  if (scientific) {
    if (format === "percentage") {
      const scaled = value * 100;
      return Number.isFinite(scaled) ? `${scaled.toExponential(3)}%` : `${value.toExponential(3)}×100%`;
    }
    const prefix = format === "currency-jpy" ? "¥" : format === "currency-usd" ? "$" : "";
    return `${prefix}${value.toExponential(3)}`;
  }
  switch (format) {
    case "percentage": return new Intl.NumberFormat("en-US", { style: "percent", maximumFractionDigits: 2 }).format(value);
    case "currency-jpy": return new Intl.NumberFormat("ja-JP", { style: "currency", currency: "JPY", maximumFractionDigits: 0 }).format(value);
    case "currency-usd": return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(value);
    case "number": return new Intl.NumberFormat("en-US", { maximumFractionDigits: 4 }).format(value);
  }
}

function wrapText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number, font: string): string[] {
  const width = Math.max(1, maxWidth);
  const lines: string[] = [];
  const previousFont = ctx.font;
  ctx.font = font;
  for (const paragraph of text.split("\n")) {
    let line = "";
    for (const character of Array.from(paragraph)) {
      const candidate = line + character;
      if (line !== "" && ctx.measureText(candidate).width > width) {
        lines.push(line);
        line = character;
      } else {
        line = candidate;
      }
    }
    lines.push(line);
  }
  ctx.font = previousFont;
  return lines.length === 0 ? [""] : lines;
}

function textLines(
  ctx: CanvasRenderingContext2D,
  lines: string[],
  x: number,
  y: number,
  lineHeight: number,
  align: CanvasTextAlign = "left",
): void {
  ctx.textAlign = align;
  ctx.textBaseline = "top";
  lines.forEach((line, index) => { ctx.fillText(line, x, y + index * lineHeight); });
}

function validateProjection(projection: ReadyReportChart): void {
  if (projection.labels.length === 0 || projection.series.length === 0) throw new Error("Report chart has no drawable values.");
  for (const series of projection.series) {
    if (series.values.length !== projection.labels.length) throw new Error("Report chart series length does not match categories.");
    series.values.forEach(value => finite(value, "Report chart value"));
  }
}

/** Render a ready private chart to a self-contained canvas for PNG export. */
export function renderReportChart(projection: ReadyReportChart): HTMLCanvasElement {
  validateProjection(projection);
  if (typeof document === "undefined") throw new Error("Canvas rendering requires a browser document.");
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("Canvas 2D context is unavailable.");
  const ctx = context;

  const right = 52;
  const titleFont = "700 26px system-ui, sans-serif";
  const categoryFont = "12px system-ui, sans-serif";
  const axisFont = "14px system-ui, sans-serif";
  const tickFont = "12px system-ui, sans-serif";
  const titleLines = wrapText(ctx, projection.chart.title, WIDTH - 96, titleFont);
  const maxAbs = Math.max(...projection.series.flatMap(series => series.values.map(Math.abs)));
  const domain = maxAbs === 0 ? 1 : maxAbs;
  ctx.font = tickFont;
  const tickValues = Array.from({ length: 5 }, (_, tick) => (1 - (tick / 4) * 2));
  const tickLabels = tickValues.map(value => formatReportNumber(value * domain, projection.numberFormat));
  const tickWidth = Math.max(...tickLabels.map(label => ctx.measureText(label).width));
  const left = Math.min(WIDTH - right - 240, Math.max(120, Math.ceil(tickWidth + 66)));
  const plotWidth = WIDTH - left - right;
  const categoryWidth = plotWidth / projection.labels.length;
  const categoryLines = projection.labels.map(label => wrapText(ctx, label, Math.max(24, categoryWidth - 10), categoryFont));
  const xLabelLines = wrapText(ctx, projection.chart.xLabel, Math.min(560, plotWidth), axisFont);
  const legendLines = projection.chart.legend
    ? projection.series.map(series => wrapText(ctx, series.label, 220, axisFont))
    : [];
  const legendHeight = legendLines.length === 0 ? 0 : Math.max(...legendLines.map(lines => lines.length)) * 18 + 12;
  const titleHeight = titleLines.length * 26;
  const top = 32 + titleHeight + legendHeight;
  const xCategoryHeight = Math.max(...categoryLines.map(lines => lines.length)) * 17;
  ctx.font = axisFont;
  const yLabelWidth = Math.max(...projection.chart.yLabel.split("\n").map(label => ctx.measureText(label).width));
  const plotHeight = Math.max(380, Math.ceil(yLabelWidth + 32), 520);
  const bottom = xCategoryHeight + xLabelLines.length * 18 + 66 + (projection.note === null ? 0 : 24);
  const plotBottom = top + plotHeight;
  const height = Math.ceil(plotBottom + bottom);
  canvas.width = WIDTH;
  canvas.height = height;

  ctx.fillStyle = "#fafaf9";
  ctx.fillRect(0, 0, WIDTH, height);
  ctx.fillStyle = INK;
  ctx.font = titleFont;
  textLines(ctx, titleLines, 48, 28, 26);

  if (projection.chart.legend) {
    ctx.font = axisFont;
    let legendX = 48;
    const legendY = 32 + titleHeight + 4;
    projection.series.forEach((series, index) => {
      const lines = legendLines[index] ?? [series.label];
      ctx.fillStyle = colorAt(index);
      ctx.fillRect(legendX, legendY + 3, 12, 12);
      ctx.fillStyle = INK;
      textLines(ctx, lines, legendX + 18, legendY, 18);
      legendX += 250;
    });
  }

  const yFor = (value: number): number => {
    const normalized = (value / domain + 1) / 2;
    return plotBottom - Math.min(1, Math.max(0, normalized)) * plotHeight;
  };
  const baseline = yFor(0);

  ctx.font = tickFont;
  ctx.strokeStyle = GRID;
  ctx.fillStyle = MUTED;
  ctx.lineWidth = 1;
  for (let tick = 0; tick <= 4; tick++) {
    const ratio = tick / 4;
    const y = top + ratio * plotHeight;
    const tickValue = tickValues[tick];
    if (tickValue === undefined) throw new Error("Report chart tick generation failed.");
    const value = tickValue * domain;
    ctx.beginPath();
    ctx.moveTo(left, y);
    ctx.lineTo(WIDTH - right, y);
    ctx.stroke();
    textLines(ctx, [formatReportNumber(value, projection.numberFormat)], left - 12, y - 7, 14, "right");
  }

  ctx.strokeStyle = INK;
  ctx.beginPath();
  ctx.moveTo(left, top);
  ctx.lineTo(left, plotBottom);
  ctx.lineTo(WIDTH - right, plotBottom);
  ctx.stroke();

  if (projection.chart.kind === "column") {
    const groupWidth = categoryWidth * 0.76;
    const barWidth = groupWidth / projection.series.length;
    projection.labels.forEach((_, categoryIndex) => {
      projection.series.forEach((series, seriesIndex) => {
        const value = series.values[categoryIndex];
        if (value === undefined) throw new Error("Report chart series length does not match categories.");
        const x = left + categoryIndex * categoryWidth + (categoryWidth - groupWidth) / 2 + seriesIndex * barWidth;
        const y = yFor(value);
        const barTop = Math.min(y, baseline);
        const barHeight = Math.abs(y - baseline);
        ctx.fillStyle = colorAt(seriesIndex);
        ctx.fillRect(x, barTop, Math.max(1, barWidth - 3), Math.max(1, barHeight));
      });
    });
  } else {
    projection.series.forEach((series, seriesIndex) => {
      ctx.strokeStyle = colorAt(seriesIndex);
      ctx.lineWidth = 3;
      ctx.beginPath();
      series.values.forEach((value, categoryIndex) => {
        const x = left + (categoryIndex + 0.5) * categoryWidth;
        const y = yFor(value);
        if (categoryIndex === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
      ctx.fillStyle = colorAt(seriesIndex);
      series.values.forEach((value, categoryIndex) => {
        const x = left + (categoryIndex + 0.5) * categoryWidth;
        const y = yFor(value);
        ctx.fillRect(x - 3, y - 3, 6, 6);
      });
    });
  }

  ctx.fillStyle = INK;
  ctx.font = categoryFont;
  projection.labels.forEach((_, index) => {
    const x = left + (index + 0.5) * categoryWidth;
    textLines(ctx, categoryLines[index] ?? [""], x, plotBottom + 12, 17, "center");
  });
  const xLabelY = plotBottom + 18 + xCategoryHeight + 12;
  ctx.font = axisFont;
  textLines(ctx, xLabelLines, left + plotWidth / 2, xLabelY, 18, "center");

  ctx.save();
  ctx.translate(30, top + plotHeight / 2);
  ctx.rotate(-Math.PI / 2);
  ctx.font = axisFont;
  const yAxisLines = wrapText(ctx, projection.chart.yLabel, plotHeight - 16, axisFont);
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const yAxisOffset = (yAxisLines.length - 1) * -9;
  yAxisLines.forEach((line, index) => { ctx.fillText(line, 0, yAxisOffset + index * 18); });
  ctx.restore();

  if (projection.note !== null) {
    ctx.fillStyle = MUTED;
    ctx.font = "12px system-ui, sans-serif";
    textLines(ctx, wrapText(ctx, projection.note, WIDTH - 96, tickFont), 48, height - 24, 16);
  }
  return canvas;
}
