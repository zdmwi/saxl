// saxl highlighting for CodeMirror. Mirrors the lexer (`lexer.rs`): keywords,
// names, strings, arrows, step labels, and comments. Contextual keywords lex
// as names, so they are highlighted only where the parser reads them as
// keywords.

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
  "library",
  "consumer",
  "package",
  "api",
  "module",
  "hook",
  "publish",
];

/** Words that are keywords only in one position and names everywhere else. */
export const CONTEXTUAL = ["impl", "at", "pub", "crate", "feature", "target"];

const keywordSet = new Set(KEYWORDS);
const NAME = "[A-Za-z_]";

/** Whether `word`, spanning `before` and `after` on its line, sits where the
 * parser reads it as a keyword. */
function isContextual(word: string, before: string, after: string): boolean {
  const lead = before.trim();
  switch (word) {
    case "impl": // `implementor impl hook`
      return IDENT_ONLY.test(lead) && new RegExp(`^\\s+${NAME}`).test(after);
    case "at": // `module m "desc" at "glob"`
      return lead !== "" && /^\s+"/.test(after);
    case "pub": // `data D in m pub {`
      return /^data\s/.test(lead) && /^\s*\{/.test(after);
    case "crate": // `crate name {`
      return lead === "" && /^\s+["A-Za-z_][^{]*\{/.test(after);
    case "target": // `package "name" target t {`
      return /^(crate|package)\s/.test(lead) && new RegExp(`^\\s+${NAME}[^{]*\\{`).test(after);
    case "feature": // `module feature f`
      return lead !== "" && !/[-~]>/.test(lead) && new RegExp(`^\\s+${NAME}`).test(after);
    default:
      return false;
  }
}
const IDENT = /^[A-Za-z_][A-Za-z0-9_]*(-[A-Za-z0-9_]+)*/;
const IDENT_ONLY = /^[A-Za-z_][A-Za-z0-9_]*(-[A-Za-z0-9_]+)*$/;

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
      if (isContextual(word, stream.string.slice(0, stream.start), stream.string.slice(stream.pos))) {
        return "keyword";
      }
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
