// The saxl playground: edit on the left, views on the right, diagnostics and
// IR below. Compilation runs in a worker 150 ms after typing stops.

import { autocompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { type Diagnostic as LintDiagnostic, lintGutter, setDiagnostics } from "@codemirror/lint";
import { EditorView, basicSetup } from "codemirror";
import { type Diagnostic, type LocatedDiagnostic, mount, type ViewerHandle } from "@saxl/viewer";
import { compressToEncodedURIComponent, decompressFromEncodedURIComponent } from "lz-string";
import { EXAMPLES } from "./examples";
import { CONTEXTUAL, KEYWORDS, saxl, saxlHighlight } from "./lang";
import { byteToUtf16, lineColToOffset } from "./positions";
import { type CompileResult, Compiler, type Ir } from "./protocol";
import "./style.css";

const DEBOUNCE_MS = 150;
const IDENT = /[A-Za-z_][A-Za-z0-9_]*(?:-[A-Za-z0-9_]+)*/g;

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string> = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  node.append(...children);
  return node;
}

// ---- Layout ----

const examples = h("select", { "aria-label": "Example" });
EXAMPLES.forEach((ex, i) => examples.append(h("option", { value: String(i) }, ex.name)));
const share = h("button", { type: "button" }, "Share");
const status = h("span", { class: "status", "aria-live": "polite" });
const header = h(
  "header",
  { class: "top" },
  h("h1", {}, "saxl ", h("span", {}, "playground")),
  examples,
  share,
  status,
);

const paneTabs = h(
  "nav",
  { class: "pane-tabs", "aria-label": "Panes" },
  h("button", { type: "button", "data-pane": "editor", "aria-pressed": "true" }, "Editor"),
  h("button", { type: "button", "data-pane": "viewer", "aria-pressed": "false" }, "Views"),
);
const editorEl = h("section", { class: "editor-pane", "aria-label": "Editor" });
const viewerEl = h("div", { class: "viewer" });
const staleNote = h("div", { class: "stale-note" }, "Showing the last valid model. Fix the errors below to update.");
const viewerPane = h("section", { class: "viewer-pane", "aria-label": "Views", "data-state": "empty" }, viewerEl, staleNote);
const panes = h("main", { class: "panes", "data-show": "editor" }, editorEl, viewerPane);

const diagCount = h("span", { class: "count" }, "0");
const diagTab = h("button", { type: "button", role: "tab", "data-tab": "diagnostics", "aria-selected": "true" }, "Diagnostics ", diagCount);
const irTab = h("button", { type: "button", role: "tab", "data-tab": "ir", "aria-selected": "false" }, "IR");
const collapse = h("button", { type: "button", class: "collapse", "aria-label": "Collapse panel", "aria-expanded": "true" }, "▾");
const diagList = h("ul", { class: "diags" });
const diagBody = h("div", { class: "tab-body", "data-tab": "diagnostics", role: "tabpanel" }, diagList);
const irPre = h("pre", { class: "ir" });
const irBody = h("div", { class: "tab-body", "data-tab": "ir", role: "tabpanel", hidden: "" }, irPre);
const bottom = h(
  "section",
  { class: "bottom", "aria-label": "Diagnostics and IR" },
  h("div", { class: "tabs", role: "tablist" }, diagTab, irTab, collapse),
  diagBody,
  irBody,
);

const explainTitle = h("h2");
const explainBody = h("pre");
const explainDialog = h(
  "dialog",
  { class: "explain" },
  h("form", { method: "dialog" }, h("header", {}, explainTitle, h("button", { type: "submit" }, "Close")), explainBody),
);

document.getElementById("app")?.append(header, paneTabs, panes, bottom, explainDialog);

/** On narrow screens, shows the editor or the views. */
function showPane(pane: "editor" | "viewer") {
  panes.dataset.show = pane;
  for (const b of paneTabs.querySelectorAll("button")) b.setAttribute("aria-pressed", String(b.dataset.pane === pane));
}
for (const button of paneTabs.querySelectorAll("button")) {
  button.addEventListener("click", () => showPane(button.dataset.pane === "viewer" ? "viewer" : "editor"));
}

function openTab(tab: "diagnostics" | "ir") {
  diagTab.setAttribute("aria-selected", String(tab === "diagnostics"));
  irTab.setAttribute("aria-selected", String(tab === "ir"));
  diagBody.hidden = tab !== "diagnostics";
  irBody.hidden = tab !== "ir";
  setCollapsed(false);
}
function setCollapsed(collapsed: boolean) {
  bottom.classList.toggle("collapsed", collapsed);
  collapse.setAttribute("aria-expanded", String(!collapsed));
  collapse.textContent = collapsed ? "▴" : "▾";
}
diagTab.addEventListener("click", () => openTab("diagnostics"));
irTab.addEventListener("click", () => openTab("ir"));
collapse.addEventListener("click", () => setCollapsed(!bottom.classList.contains("collapsed")));

let statusTimer = 0;
function say(text: string) {
  status.textContent = text;
  clearTimeout(statusTimer);
  statusTimer = window.setTimeout(() => (status.textContent = ""), 3000);
}

// ---- Editor ----

let ir: Ir | null = null;
const declaredNames = () => {
  if (!ir) return new Map<string, string>();
  const names = new Map<string, string>();
  for (const e of ir.elements) names.set(e.name.text, e.kind);
  for (const r of ir.regions) for (const n of r.infra) names.set(n.name.text, "cdn");
  return names;
};

function complete(ctx: CompletionContext): CompletionResult | null {
  const word = ctx.matchBefore(/[A-Za-z_][A-Za-z0-9_-]*/);
  if (!word || (word.from === word.to && !ctx.explicit)) return null;
  const options = [
    ...[...KEYWORDS, ...CONTEXTUAL].map((k) => ({ label: k, type: "keyword" })),
    ...[...declaredNames()].map(([name, kind]) => ({ label: name, type: "variable", detail: kind })),
  ];
  return { from: word.from, options, validFor: /^[A-Za-z_][A-Za-z0-9_-]*$/ };
}

const dark = matchMedia("(prefers-color-scheme: dark)").matches;
const theme = EditorView.theme(
  {
    "&": { height: "100%", color: "var(--saxl-fg)", backgroundColor: "var(--saxl-bg)" },
    ".cm-scroller": { fontFamily: "ui-monospace, Menlo, Consolas, monospace", fontSize: "13px", lineHeight: "1.55" },
    ".cm-gutters": { backgroundColor: "var(--saxl-panel)", color: "var(--saxl-muted)", border: "none" },
    ".cm-activeLine": { backgroundColor: "var(--saxl-hover)" },
    ".cm-activeLineGutter": { backgroundColor: "var(--saxl-hover)" },
    "&.cm-focused .cm-cursor": { borderLeftColor: "var(--saxl-fg)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": { backgroundColor: "var(--saxl-selection)" },
  },
  { dark },
);

let timer = 0;
let lastSelected: string | null = null;
let ignoreCursor = false;

const editor = new EditorView({
  parent: editorEl,
  state: EditorState.create({
    doc: initialSource(),
    extensions: [
      basicSetup,
      saxl,
      saxlHighlight,
      theme,
      lintGutter(),
      autocompletion({ override: [complete] }),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) {
          clearTimeout(timer);
          timer = window.setTimeout(() => compiler.compile(u.state.doc.toString()), DEBOUNCE_MS);
        }
        if (u.selectionSet || u.docChanged) cursorMoved();
      }),
    ],
  }),
});

function initialSource(): string {
  const match = /^#src=(.+)$/.exec(location.hash);
  if (match) {
    const source = decompressFromEncodedURIComponent(match[1]);
    if (source) return source;
  }
  return EXAMPLES[0].source;
}

/** Highlights, in the viewer, the declared name under the cursor. */
function cursorMoved() {
  if (ignoreCursor || !viewer) return;
  const pos = editor.state.selection.main.head;
  const line = editor.state.doc.lineAt(pos);
  let word: string | null = null;
  for (const m of line.text.matchAll(IDENT)) {
    const start = line.from + (m.index ?? 0);
    if (start <= pos && pos <= start + m[0].length) word = m[0];
  }
  const target = word && declaredNames().has(word) ? word : null;
  if (target !== lastSelected) {
    lastSelected = target;
    viewer.select(target);
  }
}

function selectBytes(start: number, end: number) {
  const text = editor.state.doc.toString();
  const from = byteToUtf16(text, start);
  const to = byteToUtf16(text, end);
  ignoreCursor = true;
  editor.dispatch({ selection: { anchor: from, head: to }, scrollIntoView: true });
  ignoreCursor = false;
  showPane("editor");
}

function selectLineCol(d: LocatedDiagnostic) {
  const label = d.labels[0];
  if (!label) return;
  const doc = editor.state.doc;
  const from = lineColToOffset(doc, label.range.start.line, label.range.start.col);
  const to = lineColToOffset(doc, label.range.end.line, label.range.end.col);
  editor.dispatch({ selection: { anchor: from, head: Math.max(from, to) }, scrollIntoView: true });
  editor.focus();
  showPane("editor");
}

// ---- Compile loop ----

let viewer: ViewerHandle | null = null;

/** The viewer works with byte spans; the editor with UTF-16 positions. */
function forViewer(diags: LocatedDiagnostic[]): Diagnostic[] {
  return diags.map((d) => ({
    code: d.code,
    severity: d.severity,
    message: d.message,
    help: d.help,
    labels: d.labels.map((l) => ({
      span: { file: 0, start: l.span.start, end: l.span.end },
      message: l.message,
      primary: l.primary,
    })),
  }));
}

function renderDiagnostics(diags: LocatedDiagnostic[]) {
  diagCount.textContent = String(diags.length);
  diagList.replaceChildren();
  if (diags.length === 0) {
    diagList.append(h("li", { class: "empty" }, "No problems."));
    return;
  }
  for (const d of diags) {
    const label = d.labels[0];
    const where = label ? `${label.range.start.line}:${label.range.start.col}` : "";
    const loc = h("button", { type: "button", class: "loc", title: "Go to source" }, where);
    loc.addEventListener("click", () => selectLineCol(d));
    const explain = h("button", { type: "button", class: "explain-button" }, "Explain");
    explain.addEventListener("click", async () => {
      const markdown = await compiler.explain(d.code);
      explainTitle.textContent = d.code;
      explainBody.textContent = markdown ?? `No documentation for ${d.code}.`;
      explainDialog.showModal();
    });
    const text = h("span", { class: "message" }, d.message);
    if (label?.message) text.append(h("span", { class: "label" }, ` — ${label.message}`));
    if (d.help) text.append(h("span", { class: "help" }, `help: ${d.help}`));
    diagList.append(
      h("li", { class: d.severity }, loc, h("span", { class: "code" }, d.code), text, explain),
    );
  }
}

function onResult(result: CompileResult) {
  if (result.ir) ir = result.ir;
  const doc = editor.state.doc;
  const lint: LintDiagnostic[] = result.diagnostics.flatMap((d) => {
    const label = d.labels[0];
    if (!label) return [];
    const from = lineColToOffset(doc, label.range.start.line, label.range.start.col);
    let to = lineColToOffset(doc, label.range.end.line, label.range.end.col);
    if (to <= from) to = Math.min(doc.length, from + 1);
    return [
      {
        from,
        to,
        severity: d.severity,
        source: d.code,
        message: [d.message, label.message, d.help ? `help: ${d.help}` : ""].filter(Boolean).join("\n"),
      },
    ];
  });
  editor.dispatch(setDiagnostics(editor.state, lint));
  renderDiagnostics(result.diagnostics);
  irPre.textContent = result.ir ? JSON.stringify(result.ir, null, 2) : "The IR appears once the file parses.";

  if (result.scenes) {
    const diags = forViewer(result.diagnostics);
    if (viewer) {
      viewer.setScenes(result.scenes);
      viewer.setDiagnostics(diags);
    } else {
      viewer = mount(viewerEl, result.scenes, { initialView: { kind: "tower" }, diagnostics: diags });
      viewer.onSelect((_id, source) => {
        if (source) selectBytes(source.start, source.end);
      });
    }
    if (lastSelected) viewer.select(lastSelected);
    viewerPane.dataset.state = "fresh";
  } else {
    // Keep the last valid views, dimmed. Their spans no longer match the text,
    // so drop their markers.
    viewer?.setDiagnostics([]);
    viewerPane.dataset.state = viewer ? "stale" : "empty";
    openTab("diagnostics");
  }
}

const compiler = new Compiler(onResult);
compiler.compile(editor.state.doc.toString());

// ---- Examples and sharing ----

examples.addEventListener("change", () => {
  const ex = EXAMPLES[Number(examples.value)];
  if (!ex) return;
  editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: ex.source } });
  history.replaceState(null, "", location.pathname + location.search);
});

share.addEventListener("click", async () => {
  const url = new URL(location.href);
  url.hash = `src=${compressToEncodedURIComponent(editor.state.doc.toString())}`;
  history.replaceState(null, "", url);
  try {
    await navigator.clipboard.writeText(url.toString());
    say("Link copied");
  } catch {
    say("Link is in the address bar");
  }
});
