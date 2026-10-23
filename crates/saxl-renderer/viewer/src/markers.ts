// Maps diagnostics to the nodes their primary label names, using the
// scene's anchors (every source span that names a node).

import type { Diagnostic } from "./generated/Diagnostic";
import type { Scene } from "./generated/Scene";
import type { Severity } from "./generated/Severity";
import type { Span } from "./generated/Span";

export interface Marker {
  code: string;
  severity: Severity;
  message: string;
}

function overlaps(a: Span, b: Span): boolean {
  if (b.start === b.end) return a.start <= b.start && b.start <= a.end;
  return a.start < b.end && b.start < a.end;
}

/** Markers per node id. A node is marked when one of its anchors overlaps a
 * diagnostic's primary label. */
export function markersFor(scene: Scene, diagnostics: Diagnostic[]): Map<string, Marker[]> {
  const out = new Map<string, Marker[]>();
  for (const d of diagnostics) {
    const primary = d.labels.find((l) => l.primary);
    if (!primary) continue;
    const marked = new Set<string>();
    for (const a of scene.anchors) {
      if (a.span.file === primary.span.file && overlaps(a.span, primary.span)) marked.add(a.node);
    }
    for (const id of marked) {
      const list = out.get(id) ?? [];
      list.push({ code: d.code, severity: d.severity, message: d.message });
      out.set(id, list);
    }
  }
  return out;
}
