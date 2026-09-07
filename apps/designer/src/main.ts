import "./styles.css";

import { mountDesigner } from "./designer-app.ts";
import { BrowserProjectHost } from "./host/browser-project-host.ts";
import { registerPwaFileLaunch } from "./pwa-file-launch.ts";
import { WorkerDesignerClient } from "./runtime/worker-client.ts";

const root = document.querySelector<HTMLElement>("#app");
if (root === null) {
  throw new Error("Designer application root is missing.");
}

const designer = mountDesigner(
  root,
  new WorkerDesignerClient(
    () =>
      new Worker(new URL("./runtime/designer.worker.ts", import.meta.url), {
        type: "module",
        name: "tachiko-designer-runtime",
      }),
  ),
  new BrowserProjectHost(),
);

registerPwaFileLaunch(window, (handles) => designer.openLocalDocumentHandles(handles));
if ("serviceWorker" in navigator) {
  void navigator.serviceWorker.register("/service-worker.js").catch(() => undefined);
}
