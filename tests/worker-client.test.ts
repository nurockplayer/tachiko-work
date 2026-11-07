
describe("Published changed-cell wire validation", () => {
  it("refuses a changed cell with bad presence", async () => {
    let receive: ((event: MessageEvent) => void) | undefined;
    const requests: { id: number }[] = [];
    vi.stubGlobal("Worker", class {
      addEventListener(type: string, callback: (event: MessageEvent) => void) {
        if (type === "message") receive = callback;
      }
      postMessage(request: { id: number }) { requests.push(structuredClone(request)); }
      terminate() {}
    });
    const client = createExperimentalDesignerClient();
    const target = {entity: "row", field: "note"};
    const pending = client.clearField("resident/0", target);
    const publication = {
      base_revision: "resident/0",
      resulting_revision: "resident/1",
      schemas: [],
      schema_fields: [],
      entities: [],
      fields: [target],
      affected_calculations: [],
      changed: [{
        target,
        address: "items.row.note",
        stored: null,
        formula: null,
        calculated: null,
        diagnostics: [],
        editable_scalar: null,
      }],
    };
    receive!(new MessageEvent("message", {
      data: {id: requests.at(-1)!.id, status: "ok", response: {type: "published", payload: publication}},
    }));
    await expect(pending).rejects.toThrow("non-blank field payload");
    client.close();
  });
});
