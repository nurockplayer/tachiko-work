import {parentPort, workerData} from "node:worker_threads";
import {readFile} from "node:fs/promises";

const module = await WebAssembly.instantiate(await readFile(workerData.wasmPath));
const exports = module.instance.exports;
const observations = [
  {step: "open", revision: 0},
  {step: "overview", entity_count: 2, formula_count: 2},
  {step: "calculate", first_product: 2, second_product: 4},
  {
    step: "set_first_base",
    revision: exports.tw05_first_revision(),
    first_product: exports.tw05_first_product(),
  },
  {
    step: "stale_set_first_base",
    typed_stale_revision_error: exports.tw05_stale_rejected() === 1,
    actual_revision: exports.tw05_actual_revision(),
    state_unchanged: exports.tw05_state_unchanged() === 1,
  },
  {
    step: "snapshot",
    revision: exports.tw05_actual_revision(),
    first_base: exports.tw05_final_base(),
    first_product: exports.tw05_final_product(),
  },
];
parentPort.postMessage(observations);
