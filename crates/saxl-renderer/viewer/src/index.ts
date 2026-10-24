// @saxl/viewer: renders saxl scenes. Used by `saxl render` HTML output and by
// the playground.

import type { Diagnostic } from "./generated/Diagnostic";
import type { LocatedDiagnostic } from "./generated/LocatedDiagnostic";
import type { Scene } from "./generated/Scene";
import type { Span } from "./generated/Span";
import type { ViewKind } from "./generated/ViewKind";
import { markersFor } from "./markers";
import { panZoom, type PanZoom, type PanZoomState } from "./panzoom";
import { draw, type Drawn } from "./render2d";
import { CSS } from "./style";
import { mountTower, type TowerHandle, type TowerState } from "./tower";

export type { Diagnostic, LocatedDiagnostic, Scene, Span, ViewKind };

export interface ViewerOptions {
  /** The view shown first. Defaults to the first scene. */
  initialView?: ViewKind;
  /** Diagnostics listed under the view and marked on the nodes they name. */
  diagnostics?: Diagnostic[];
}

export interface ViewerHandle {
  /** Shows a view. Ignored if no scene has it. */
  show(view: ViewKind): void;
  /** Plays a scenario in the tower, or shows its runtime view if there is no tower. */
  play(scenario: string): void;
  /** Steps tower playback by `delta` slots. */
  step(delta: number): void;
  setDiagnostics(diags: Diagnostic[]): void;
  /** Replaces the scenes, keeping the current view if it still exists. */
  setScenes(scenes: Scene[]): void;
  onSelect(cb: (id: string, source?: Span) => void): void;
  /** Highlights the element named `id` in the current view, or clears it. */
  select(id: string | null): void;
  destroy(): void;
}

/** A tab's label: the view's name from the scene title ("Modules: Quill"), so
 * a library's views get their own names. Falls back to the fixed label. */
function tabLabel(scene: Scene): string {
  const name = scene.title.split(":")[0];
  return name && scene.view.kind !== "runtime" ? name : LABELS[scene.view.kind];
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
  let tower: TowerHandle | null = null;
  // Camera state per view, kept across view switches and scene updates.
  const panStates = new Map<string, PanZoomState>();
  let towerState: TowerState | undefined;
  const viewKey = (v: ViewKind) => (v.kind === "runtime" ? `runtime:${v.scenario}` : v.kind);
  let selected: string | null = null;
  let diagnostics: Diagnostic[] = [];
  const selectCallbacks: ((id: string, source?: Span) => void)[] = [];

  const sceneFor = (view: ViewKind) => scenes.find((s) => sameView(s.view, view));
  const emitSelect = (id: string, source?: Span) => {
    for (const cb of selectCallbacks) cb(id, source);
  };

  function renderBar() {
    bar.replaceChildren();
    const seen = new Set<string>();
    for (const scene of scenes) {
      const kind = scene.view.kind;
      if (seen.has(kind)) continue;
      seen.add(kind);
      const button = h("button", undefined, tabLabel(scene));
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
          select.setAttribute("aria-label", "Runtime scenario");
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
    bar.append(h("span", "saxl-title", current ? (sceneFor(current)?.title ?? "") : ""));
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

  /** 2D nodes depicting element `id`: the node itself, or `region/id` in the deployment view. */
  function nodesFor(id: string): SVGGElement[] {
    if (!drawn) return [];
    return [...drawn.nodes].filter(([key]) => key === id || key.endsWith(`/${id}`)).map(([, g]) => g);
  }

  function select(id: string | null) {
    if (selected) for (const g of nodesFor(selected)) g.classList.remove("selected");
    selected = id;
    if (id) for (const g of nodesFor(id)) g.classList.add("selected");
    tower?.select(id);
  }

  function teardown() {
    if (pz && current) panStates.set(viewKey(current), pz.state());
    if (tower) towerState = tower.state();
    pz?.destroy();
    pz = null;
    tower?.destroy();
    tower = null;
    drawn = null;
  }

  function show(view: ViewKind) {
    const scene = sceneFor(view);
    if (!scene) return;
    teardown();
    current = scene.view;
    root.dataset.view = scene.view.kind;
    const markers = markersFor(scene, diagnostics);
    if (scene.tower) {
      stage.classList.add("is-3d");
      stage.replaceChildren();
      tower = mountTower(stage, scene, { onSelect: emitSelect, markers, restore: towerState });
      if (selected) tower.select(selected);
      renderBar();
      return;
    }
    stage.classList.remove("is-3d");
    drawn = draw(scene, markers);
    stage.replaceChildren(drawn.svg, card);
    const saved = panStates.get(viewKey(scene.view));
    pz = panZoom(stage, drawn.content, scene.size.w, scene.size.h, saved);
    if (!saved) pz.fit();
    for (const n of scene.nodes) {
      const g = drawn.nodes.get(n.id);
      if (!g) continue;
      g.addEventListener("pointerenter", (e) => {
        card.replaceChildren(h("span", "k", n.kind.replace("_", " ")), h("b", undefined, n.label));
        if (n.sublabel) card.append(h("div", undefined, n.sublabel));
        if (n.badges.length) card.append(h("div", "k", n.badges.join(" · ")));
        for (const m of markers.get(n.id) ?? []) card.append(h("div", `diag ${m.severity}`, `${m.code}: ${m.message}`));
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
        emitSelect(n.id, n.source ?? undefined);
      });
    }
    if (selected) select(selected);
    renderBar();
  }

  const handle: ViewerHandle = {
    show,
    play(scenario) {
      if (scenes.some((s) => s.view.kind === "tower")) {
        if (current?.kind !== "tower") show({ kind: "tower" });
        tower?.play(scenario);
      } else {
        show({ kind: "runtime", scenario });
      }
    },
    step(delta) {
      tower?.step(delta);
    },
    setDiagnostics(diags) {
      diagnostics = diags;
      renderDiagnostics();
      const scene = current ? sceneFor(current) : undefined;
      if (tower && scene) {
        tower.setMarkers(markersFor(scene, diagnostics));
        renderBar();
      } else if (current) show(current);
      else renderBar();
    },
    setScenes(next) {
      scenes = next;
      show(current && sceneFor(current) ? current : (scenes[0]?.view ?? { kind: "context" }));
    },
    onSelect(cb) {
      selectCallbacks.push(cb);
    },
    select,
    destroy() {
      teardown();
      el.replaceChildren();
    },
  };

  handle.setDiagnostics(opts.diagnostics ?? []);
  const initial = opts.initialView && sceneFor(opts.initialView) ? opts.initialView : scenes[0]?.view;
  if (initial) show(initial);
  return handle;
}
