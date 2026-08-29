import "./styles.css";

import { mountDesigner } from "./designer-app.ts";
import { WorkerDesignerClient } from "./runtime/worker-client.ts";

const root = document.querySelector<HTMLElement>("#app");
if (root === null) {
  throw new Error("Designer application root is missing.");
}

mountDesigner(root, new WorkerDesignerClient());
