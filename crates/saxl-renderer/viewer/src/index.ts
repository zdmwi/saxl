// @saxl/viewer: renders saxl scenes. Used by `saxl render` HTML output and by
// the playground.

import type { Diagnostic } from "./generated/Diagnostic";
import type { Scene } from "./generated/Scene";
import type { Span } from "./generated/Span";
import type { ViewKind } from "./generated/ViewKind";
import { panZoom, type PanZoom } from "./panzoom";
import { draw, type Drawn } from "./render2d";
import { CSS } from "./style";

export type { Diagnostic, Scene, Span, ViewKind };

export interface ViewerOptions {
  /** The view shown first. Defaults to the first scene. */
  initialView?: ViewKind;
  /** Diagnostics listed under the view. */
  diagnostics?: Diagnostic[];
}

export interface ViewerHandle {
  /** Shows a view. Ignored if no scene has it. */
  show(view: ViewKind): void;
  /** Shows a scenario. Animated playback arrives with the tower view. */
  play(scenario: string): void;
  /** Steps scenario playback. Reserved for the tower view. */
  step(delta: number): void;
  setDiagnostics(diags: Diagnostic[]): void;
  /** Replaces the scenes, keeping the current view if it still exists. */
  setScenes(scenes: Scene[]): void;
  onSelect(cb: (id: string, source?: Span) => void): void;
  destroy(): void;
}

const LABELS: Record<ViewKind["kind"], string> = {
  context: "Context",
  containers: "Containers",
  runtime: "Runtime",
  data: "Data",
  deployment: "Deployment",
  tower: "Tower",
};

function sameView(a: ViewKind, b: ViewKind): boolean {
  return a.kind === b.kind && (a.kind !== "runtime" || (b.kind === "runtime" && a.scenario === b.scenario));
}

function ensureStyle(doc: Document) {
  if (doc.getElementById("saxl-viewer-style")) return;
  const style = doc.createElement("style");
  style.id = "saxl-viewer-style";
  style.textContent = CSS;
  doc.head.append(style);
}

function h<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mount(el: HTMLElement, scenes: Scene[], opts: ViewerOptions = {}): ViewerHandle {
  ensureStyle(el.ownerDocument);
  const root = h("div", "saxl-viewer");
  const bar = h("div", "saxl-bar");
  const stage = h("div", "saxl-stage");
  const card = h("div", "saxl-card");
  card.hidden = true;
  const diagPanel = h("div", "saxl-diags");
  diagPanel.hidden = true;
  root.append(bar, stage, diagPanel);
  el.replaceChildren(root);

  let current: ViewKind | null = null;
  let drawn: Drawn | null = null;
  let pz: PanZoom | null = null;
  let selected: string | null = null;
  let diagnostics: Diagnostic[] = [];
  const selectCallbacks: ((id: string, source?: Span) => void)[] = [];

  const sceneFor = (view: ViewKind) => scenes.find((s) => sameView(s.view, view));

  function renderBar() {
    bar.replaceChildren();
    const seen = new Set<string>();
    for (const scene of scenes) {
      const kind = scene.view.kind;
      if (seen.has(kind)) continue;
      seen.add(kind);
      const button = h("button", undefined, LABELS[kind]);
      button.type = "button";
      button.setAttribute("aria-pressed", String(current?.kind === kind));
      button.addEventListener("click", () => {
        const keep = current && current.kind === kind ? current : scene.view;
        show(keep);
      });
      bar.append(button);
      if (kind === "runtime") {
        const runtimes = scenes.filter((s) => s.view.kind === "runtime");
        if (runtimes.length > 1) {
          const select = h("select");
          select.setAttribute("aria-label", "Scenario");
          for (const s of runtimes) {
            if (s.view.kind !== "runtime") continue;
            const option = h("option", undefined, s.view.scenario);
            option.value = s.view.scenario;
            option.selected = current?.kind === "runtime" && current.scenario === s.view.scenario;
            select.append(option);
          }
          select.addEventListener("change", () => show({ kind: "runtime", scenario: select.value }));
          bar.append(select);
        }
      }
    }
    const toggle = h("button", "saxl-diag-toggle");
    toggle.type = "button";
    const problems = diagnostics.length;
    toggle.dataset.count = String(problems);
    toggle.textContent = `${problems} ${problems === 1 ? "problem" : "problems"}`;
    toggle.setAttribute("aria-pressed", String(!diagPanel.hidden));
    toggle.addEventListener("click", () => {
      diagPanel.hidden = !diagPanel.hidden;
      renderBar();
    });
    bar.append(toggle);
    const title = h("span", "saxl-title", current ? (sceneFor(current)?.title ?? "") : "");
    bar.append(title);
  }

  function renderDiagnostics() {
    const list = h("ul");
    for (const d of diagnostics) {
      const item = h("li", d.severity);
      item.append(h("span", "code", d.code), document.createTextNode(d.message));
      list.append(item);
    }
    diagPanel.replaceChildren(list);
  }

  function select(id: string | null) {
    if (selected && drawn) drawn.nodes.get(selected)?.classList.remove("selected");
    selected = id;
    if (id && drawn) drawn.nodes.get(id)?.classList.add("selected");
  }

  function show(view: ViewKind) {
    const scene = sceneFor(view);
    if (!scene) return;
    current = scene.view;
    pz?.destroy();
    drawn = draw(scene);
    stage.replaceChildren(drawn.svg, card);
    pz = panZoom(stage, drawn.content, scene.size.w, scene.size.h);
    pz.fit();
    for (const n of scene.nodes) {
      const g = drawn.nodes.get(n.id);
      if (!g) continue;
      g.addEventListener("pointerenter", (e) => {
        card.replaceChildren(h("span", "k", n.kind.replace("_", " ")), h("b", undefined, n.label));
        if (n.sublabel) card.append(h("div", undefined, n.sublabel));
        if (n.badges.length) card.append(h("div", "k", n.badges.join(" · ")));
        const box = stage.getBoundingClientRect();
        card.style.left = `${e.clientX - box.left + 12}px`;
        card.style.top = `${e.clientY - box.top + 12}px`;
        card.hidden = false;
      });
      g.addEventListener("pointerleave", () => {
        card.hidden = true;
      });
      g.addEventListener("click", () => {
        select(n.id);
        for (const cb of selectCallbacks) cb(n.id, n.source ?? undefined);
      });
    }
    if (selected) select(selected);
    renderBar();
  }

  const handle: ViewerHandle = {
    show,
    play(scenario) {
      show({ kind: "runtime", scenario });
    },
    step() {},
    setDiagnostics(diags) {
      diagnostics = diags;
      renderDiagnostics();
      renderBar();
    },
    setScenes(next) {
      scenes = next;
      show(current && sceneFor(current) ? current : (scenes[0]?.view ?? { kind: "context" }));
    },
    onSelect(cb) {
      selectCallbacks.push(cb);
    },
    destroy() {
      pz?.destroy();
      el.replaceChildren();
    },
  };

  handle.setDiagnostics(opts.diagnostics ?? []);
  const initial = opts.initialView && sceneFor(opts.initialView) ? opts.initialView : scenes[0]?.view;
  if (initial) show(initial);
  return handle;
}
