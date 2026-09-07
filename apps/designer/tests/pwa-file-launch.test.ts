import { describe, expect, it, vi } from "vitest";

import {
  registerPwaFileLaunch,
  type PwaLaunchParams,
  type PwaLaunchQueue,
} from "../src/pwa-file-launch.ts";

function params(): PwaLaunchParams {
  return {
    files: [{
      kind: "file",
      name: "game.ro",
      getFile: async () => new File(["opaque Rust input"], "game.ro"),
    }],
  };
}

describe("installed PWA file launch registration", () => {
  it("does nothing when File Handling is unavailable", () => {
    expect(registerPwaFileLaunch({}, vi.fn())).toBe(false);
  });

  it("treats an empty ordinary launch as a no-op", async () => {
    let consumer: ((next: PwaLaunchParams) => void | Promise<void>) | undefined;
    const queue: PwaLaunchQueue = {
      setConsumer(next) { consumer = next; },
    };
    const consume = vi.fn(async () => undefined);

    expect(registerPwaFileLaunch({ launchQueue: queue }, consume)).toBe(true);
    if (consumer === undefined) throw new Error("launch consumer was not registered");
    await consumer({ files: [] });

    expect(consume).not.toHaveBeenCalled();
  });

  it("registers one consumer and ignores a repeated delivery of the same launch", async () => {
    let consumer: ((next: PwaLaunchParams) => void | Promise<void>) | undefined;
    const queue: PwaLaunchQueue = {
      setConsumer(next) { consumer = next; },
    };
    const consume = vi.fn(async () => undefined);

    expect(registerPwaFileLaunch({ launchQueue: queue }, consume)).toBe(true);
    if (consumer === undefined) throw new Error("launch consumer was not registered");
    const launch = params();
    await consumer(launch);
    await consumer(launch);

    expect(consume).toHaveBeenCalledOnce();
    expect(consume).toHaveBeenCalledWith(launch.files);
  });
});
