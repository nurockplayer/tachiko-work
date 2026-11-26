import { expect, it } from "vitest";
import type { CellStyleChange, ExperimentalDesignerClient } from "../src/experimental-client.ts";
import type { DesignerRequest, TrackerCommand } from "../src/runtime/protocol.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";

it("sends the rectangular cell style command through the kit without changing its patch", async () => {
  let receive: ((event: MessageEvent) => void) | undefined;
  const sent: DesignerRequest[] = [];
  const worker = {
    addEventListener(type: string, callback: (event: MessageEvent) => void) {
      if (type === "message") receive = callback;
    },
    postMessage(message: { id: number; request: DesignerRequest }) {
      sent.push(structuredClone(message.request));
      receive?.({data: {id: message.id, status: "ok", response: {type: "published", payload: {
        base_revision: "resident/0", resulting_revision: "resident/1", schemas: [], schema_fields: [],
        entities: [], fields: [], affected_calculations: [], many: true, changed: [],
      }}}} as MessageEvent);
    },
  } as unknown as Worker;
  const client: Pick<ExperimentalDesignerClient, "setCellStyle" | "setCellStyleRange"> = new WorkerDesignerClient(() => worker);
  const changes: CellStyleChange[] = [
    {kind: "toggle_bold"}, {kind: "italic", value: true}, {kind: "fill", color: "#FFFF00"},
    {kind: "border", target: "outline", value: true},
    {kind: "border", target: "top", value: false},
    {kind: "border", target: "right", value: true},
    {kind: "border", target: "bottom", value: false},
    {kind: "border", target: "left", value: true},
    {kind: "number_format", format: {kind: {type: "percent"}, decimals: 2, thousands_separator: false}},
  ];
  const publication = await client.setCellStyle("resident/0", "rows", ["r2", "r3", "r4"], ["b", "c"], changes);
  const clear: CellStyleChange[] = [{kind: "bold", value: false}, {kind: "toggle_italic"}, {kind: "fill", color: null}, {kind: "border", target: "all", value: false}];
  await client.setCellStyle("resident/1", "rows", ["r2"], ["c"], clear);
  await client.setCellStyleRange("resident/2", "rows", 1, 5000, ["c", "b"], changes);
  expect(sent).toEqual([
    {type: "set_cell_style", expected_revision: "resident/0", collection: "rows", entities: ["r2", "r3", "r4"], fields: ["b", "c"], changes},
    {type: "set_cell_style", expected_revision: "resident/1", collection: "rows", entities: ["r2"], fields: ["c"], changes: clear},
    {type: "set_cell_style_range", expected_revision: "resident/2", collection: "rows", row: 1, row_count: 5000, fields: ["c", "b"], changes},
  ] satisfies TrackerCommand[]);
  expect(publication.resulting_revision).toBe("resident/1");
  expect(publication.many).toBe(true);
});

// @ts-expect-error Fill must explicitly specify its colour or clear it with null.
const invalidFill: CellStyleChange = {kind: "fill"};
// @ts-expect-error A toggle is a selection-wide operation, without a value.
const invalidToggle: CellStyleChange = {kind: "toggle_bold", value: true};
void invalidFill;
void invalidToggle;

// @ts-expect-error A border operation requires an explicit set/clear value.
const invalidBorder: CellStyleChange = {kind: "border", target: "outline"};
// @ts-expect-error Border targets are the four sides, outline or all.
const invalidBorderTarget: CellStyleChange = {kind: "border", target: "diagonal", value: true};
void invalidBorder;
void invalidBorderTarget;
