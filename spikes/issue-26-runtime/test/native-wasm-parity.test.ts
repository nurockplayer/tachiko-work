import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import readline from "node:readline";
import test from "node:test";
import { fileURLToPath } from "node:url";

import type {
  RuntimeCommand,
  WireReply,
  WireRequest,
  WireResult,
} from "../worker/protocol.ts";
import { RuntimeClient } from "../worker/runtime-client.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const spikeRoot = path.resolve(here, "..");
const wasmPath = path.join(
  spikeRoot,
  "target/wasm32-unknown-unknown/release/tachiko_issue_26_runtime_spike.wasm",
);
const nativePath = path.join(spikeRoot, "target/release/native-driver");

type Fixture = {
  entity_count: number;
  resident_mutation: RuntimeCommand;
  ours_mutation: RuntimeCommand;
  theirs_mutation: RuntimeCommand;
};

class NativeDriver {
  #process = spawn(nativePath, [], { stdio: ["pipe", "pipe", "inherit"] });
  #lines = readline.createInterface({ input: this.#process.stdout })[
    Symbol.asyncIterator
  ]();

  async request(request: WireRequest): Promise<WireResult> {
    this.#process.stdin.write(`${JSON.stringify(request)}\n`);
    const line = await this.#lines.next();
    if (line.done) {
      throw new Error("native driver ended before replying");
    }
    const reply = JSON.parse(line.value) as WireReply;
    if (!reply.ok) {
      throw new Error(reply.error);
    }
    return reply.result;
  }

  async close(): Promise<void> {
    this.#process.stdin.end();
    await new Promise<void>((resolve, reject) => {
      this.#process.once("exit", (code) => {
        if (code === 0) resolve();
        else reject(new Error(`native driver exited with ${code}`));
      });
    });
  }
}

test("native and Worker/WASM return byte-identical deterministic operation records", async () => {
  const fixture = JSON.parse(
    await readFile(path.join(spikeRoot, "fixtures/parity-commands.json"), "utf8"),
  ) as Fixture;
  const native = new NativeDriver();
  const wasm = await RuntimeClient.spawn(wasmPath);

  const compare = async (request: WireRequest): Promise<WireResult> => {
    const [nativeResult, wasmResult] = await Promise.all([
      native.request(request),
      wasm.wireRequest(request),
    ]);
    assert.equal(JSON.stringify(wasmResult), JSON.stringify(nativeResult));
    return nativeResult;
  };

  try {
    const generated = await compare({
      type: "generate_synthetic",
      entity_count: fixture.entity_count,
    });
    assert.equal(generated.type, "generated");
    if (generated.type !== "generated") throw new Error("expected generated result");
    const base = generated.document;

    await compare({ type: "open", document: base });
    await compare({ type: "execute", command: { type: "overview" } });
    await compare({ type: "execute", command: { type: "calculate" } });
    await compare({ type: "execute", command: fixture.resident_mutation });
    await compare({ type: "snapshot" });

    const ours = await compare({
      type: "execute_snapshot",
      document: base,
      command: fixture.ours_mutation,
    });
    const theirs = await compare({
      type: "execute_snapshot",
      document: base,
      command: fixture.theirs_mutation,
    });
    assert.equal(ours.type, "snapshot_execution");
    assert.equal(theirs.type, "snapshot_execution");
    if (ours.type !== "snapshot_execution" || theirs.type !== "snapshot_execution") {
      throw new Error("expected branch snapshot results");
    }

    await compare({ type: "open", document: ours.document });
    const merged = await compare({
      type: "execute",
      command: { type: "merge", base, theirs: theirs.document },
    });
    assert.equal(merged.type, "command");
    if (merged.type !== "command") throw new Error("expected merge command result");
    assert.equal(merged.response.result.type, "merge");
    if (merged.response.result.type !== "merge") {
      throw new Error("expected merge result");
    }
    assert.equal(merged.response.result.merged, true);
    assert.equal(merged.response.result.change_count, 4);
    await compare({ type: "execute", command: { type: "calculate" } });
  } finally {
    await Promise.all([native.close(), wasm.close()]);
  }
});
