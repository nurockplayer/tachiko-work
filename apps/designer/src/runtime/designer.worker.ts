/// <reference lib="webworker" />

import { startDesignerWorker } from "@tachiko-work/browser-client/runtime/worker-runtime";

startDesignerWorker("/designer_runtime.wasm");
