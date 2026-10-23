// Converting between the compiler's positions and the editor's. The compiler
// reports byte offsets (UTF-8) and 1-based line/column in UTF-16 units; the
// editor indexes text in UTF-16 units from 0.

import type { Text } from "@codemirror/state";

/** UTF-16 offset of a UTF-8 byte offset in `text`. */
export function byteToUtf16(text: string, byte: number): number {
  let bytes = 0;
  let i = 0;
  while (i < text.length && bytes < byte) {
    const code = text.codePointAt(i) ?? 0;
    bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4;
    i += code >= 0x10000 ? 2 : 1;
  }
  return i;
}

/** Editor offset of a 1-based line and UTF-16 column. */
export function lineColToOffset(doc: Text, line: number, col: number): number {
  if (line > doc.lines) return doc.length;
  const l = doc.line(Math.max(1, line));
  return Math.min(l.to, l.from + Math.max(0, col - 1));
}
