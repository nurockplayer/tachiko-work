import type { LocalDocumentHandle } from "./host/local-document-ingress.ts";

export type PwaLaunchParams = {
  files: readonly LocalDocumentHandle[];
};

export type PwaLaunchQueue = {
  setConsumer(consumer: (params: PwaLaunchParams) => void | Promise<void>): void;
};

/**
 * Register the one installed-PWA launch consumer when the browser exposes it.
 * Replaying the same launch object is host transport duplication, not a second
 * document-open request.
 */
export function registerPwaFileLaunch(
  target: object,
  consume: (files: readonly LocalDocumentHandle[]) => Promise<void>,
): boolean {
  const queue = Reflect.get(target, "launchQueue") as PwaLaunchQueue | undefined;
  if (queue === undefined) return false;

  const delivered = new WeakSet<PwaLaunchParams>();
  queue.setConsumer(async (params) => {
    if (delivered.has(params)) return;
    delivered.add(params);
    if (params.files.length === 0) return;
    await consume(params.files);
  });
  return true;
}
