// Deliberate missing-product seam. No implementation authority before live Ready.
export function proposeHandoff(_compiledWorkbook, _mapping) {
  throw Object.assign(new Error("HANDOFF_NOT_IMPLEMENTED"), {
    code: "HANDOFF_NOT_IMPLEMENTED",
  });
}
