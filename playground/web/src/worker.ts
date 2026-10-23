// Runs the compiler off the main thread. Loads the wasm once.

import init, { compile, explain } from "./wasm/saxl_wasm.js";
import type { Request, Response } from "./protocol";

const ready = init();

// A dedicated worker's global scope; typed by hand to avoid mixing DOM and
// WebWorker libs in one tsconfig.
const scope = self as unknown as {
  onmessage: ((e: MessageEvent<Request>) => void) | null;
  postMessage(message: Response): void;
};

scope.onmessage = async (e) => {
  await ready;
  const req = e.data;
  if (req.kind === "compile") {
    scope.postMessage({ kind: "compile", seq: req.seq, result: compile(req.source) });
  } else {
    scope.postMessage({ kind: "explain", seq: req.seq, markdown: explain(req.code) ?? null });
  }
};
