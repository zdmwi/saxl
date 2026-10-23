// saxl highlighting for CodeMirror. Mirrors the lexer (PLAN §4.2): keywords,
// names, strings, arrows, step labels, and comments.

import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";

export const KEYWORDS = [
  "system",
  "actor",
  "external",
  "service",
  "store",
  "queue",
  "data",
  "in",
  "cached",
  "via",
  "deploy",
  "region",
  "cdn",
  "cluster",
  "managed",
  "scenario",
  "parallel",
  "allow",
];

const keywordSet = new Set(KEYWORDS);
const IDENT = /^[A-Za-z_][A-Za-z0-9_]*(-[A-Za-z0-9_]+)*/;

export const saxl = StreamLanguage.define<Record<string, never>>({
  name: "saxl",
  startState: () => ({}),
  token(stream) {
    if (stream.eatSpace()) return null;
    const next = stream.peek();
    if (next === "#") {
      stream.skipToEnd();
      return "comment";
    }
    if (next === ":") {
      // A step label runs to the end of the line, `#` included.
      stream.skipToEnd();
      return "string.special";
    }
    if (next === '"') {
      stream.next();
      stream.eatWhile((c) => c !== '"');
      stream.eat('"');
      return "string";
    }
    if (stream.match("-->") || stream.match("->") || stream.match("~>")) return "operator";
    if (stream.match(IDENT)) {
      const word = stream.current();
      if (keywordSet.has(word)) return "keyword";
      if (/^x[0-9]+$/.test(word)) return "number";
      if (/^[EWIP][0-9]{3}$/.test(word)) return "atom";
      return "variableName";
    }
    if (stream.match(/^[{}]/)) return "brace";
    if (stream.match(/^[,+]/)) return "punctuation";
    stream.next();
    return "invalid";
  },
  languageData: { commentTokens: { line: "#" } },
});

/** Colors from the playground's CSS tokens, so light and dark both work. */
export const saxlHighlight = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.keyword, color: "var(--saxl-service)", fontWeight: "600" },
    { tag: tags.comment, color: "var(--saxl-muted)", fontStyle: "italic" },
    { tag: tags.string, color: "var(--saxl-store)" },
    { tag: tags.special(tags.string), color: "var(--saxl-store)", fontStyle: "italic" },
    { tag: tags.operator, color: "var(--saxl-event)", fontWeight: "600" },
    { tag: [tags.number, tags.atom], color: "var(--saxl-request)" },
    { tag: tags.variableName, color: "var(--saxl-fg)" },
    { tag: tags.invalid, color: "var(--saxl-event)", textDecoration: "underline wavy" },
  ]),
);
