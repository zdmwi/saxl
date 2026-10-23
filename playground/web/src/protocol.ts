// Messages between the page and the compiler worker.

import type { LocatedDiagnostic, Scene } from "@saxl/viewer";

/** The IR, kept loosely typed: the playground only reads names from it. */
export interface Ir {
  elements: { name: { text: string }; kind: string }[];
  regions: { infra: { name: { text: string } }[] }[];
  scenarios: { name: { text: string } }[];
}

export interface CompileResult {
  diagnostics: LocatedDiagnostic[];
  ir: Ir | null;
  scenes: Scene[] | null;
}

export type Request =
  | { kind: "compile"; seq: number; source: string }
  | { kind: "explain"; seq: number; code: string };

export type Response =
  | { kind: "compile"; seq: number; result: CompileResult }
  | { kind: "explain"; seq: number; markdown: string | null };

/** Talks to the worker. Compile results that arrive after a newer request
 * was sent are dropped. */
export class Compiler {
  private worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  private seq = 0;
  private latestCompile = 0;
  private pending = new Map<number, (markdown: string | null) => void>();

  constructor(onResult: (result: CompileResult) => void) {
    this.worker.onmessage = (e: MessageEvent<Response>) => {
      const msg = e.data;
      if (msg.kind === "compile") {
        if (msg.seq === this.latestCompile) onResult(msg.result);
      } else {
        this.pending.get(msg.seq)?.(msg.markdown);
        this.pending.delete(msg.seq);
      }
    };
  }

  compile(source: string) {
    this.latestCompile = ++this.seq;
    this.post({ kind: "compile", seq: this.latestCompile, source });
  }

  explain(code: string): Promise<string | null> {
    const seq = ++this.seq;
    return new Promise((resolve) => {
      this.pending.set(seq, resolve);
      this.post({ kind: "explain", seq, code });
    });
  }

  private post(req: Request) {
    this.worker.postMessage(req);
  }
}
