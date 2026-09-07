// Steward-owned trusted authoring fixture, not an arbitrary module loader.
// Direct component calls are the public equivalent of open-sheet JSX authoring.
export const SOURCE_ROWS = [
  { label: "January", revenue: 1200, cost: 400 },
  { label: "February", revenue: 1600, cost: 800 },
  { label: "March", revenue: 2000, cost: 1000 },
];

export function compileModel(core, options = {}) {
  const { Workbook, Sheet, Table, col, compile, sub, mul, ref } = core;
  const rows = options.rows ?? SOURCE_ROWS;
  const columns = options.columns ?? ["label", "revenue", "cost", "gross", "net"];
  const formula = {
    gross: (row) => sub(row.cell("revenue"), row.cell("cost")),
    net: (row) => mul(row.cell("gross"), sub(1, ref("assumptions").get("rate"))),
    ...options.formulas,
  };
  const sheets = [
    Sheet({
      name: options.assumptionsName ?? "Assumptions",
      origin: options.origin ?? { r: 0, c: 0 },
      children: Table({
        name: "assumptions", kind: "keyValue",
        data: [{ key: "rate", label: "Tax rate", value: options.rate ?? 0.25,
          format: "0.00%" }],
      }),
    }),
    Sheet({
      name: options.planName ?? "P&L",
      origin: options.origin ?? { r: 0, c: 0 },
      children: Table({
        name: "plan", data: rows,
        ...(options.appendable ? { appendable: true } : {}),
        columns: columns.map((key) => col(key, {
          ...(formula[key] ? { formula: formula[key] } : {}),
          ...(key === "label" ? {} : { format: "#,##0.00" }),
        })),
      }),
    }),
  ];
  return compile(Workbook({ children: options.reverseSheets ? sheets.reverse() : sheets }));
}
