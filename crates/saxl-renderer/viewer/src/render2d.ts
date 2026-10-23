// Draws a 2D scene as SVG. Geometry comes from the scene unchanged; this
// module only decides how things look.

import type { EdgeStyle } from "./generated/EdgeStyle";
import type { Point2 } from "./generated/Point2";
import type { Scene } from "./generated/Scene";
import type { SceneEdge } from "./generated/SceneEdge";
import type { SceneGroup } from "./generated/SceneGroup";
import type { SceneNode } from "./generated/SceneNode";
import type { Marker } from "./markers";

const NS = "http://www.w3.org/2000/svg";
const PAD = 24;
const BADGE_ROW = 20;
const FIELD_LINE = 18;
const CORNER = 8;
/** Vertical radius of a store cylinder's top and bottom ellipses (matches Rust). */
const STORE_CAP = 8;
/** Horizontal radius of a queue pipe's end ellipses (matches Rust). */
const QUEUE_CAP = 10;

let instance = 0;

function el<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number> = {},
  text?: string,
): SVGElementTagNameMap[K] {
  const node = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
  if (text !== undefined) node.textContent = text;
  return node;
}

export interface Drawn {
  svg: SVGSVGElement;
  /** The group that pan and zoom transform. */
  content: SVGGElement;
  /** Node elements by scene node id. */
  nodes: Map<string, SVGGElement>;
}

export function draw(scene: Scene, markers: Map<string, Marker[]> = new Map()): Drawn {
  const id = `saxl${++instance}`;
  const svg = el("svg", { role: "img", "aria-label": scene.title });
  const defs = el("defs");
  for (const style of ["sync", "async", "reply"] as EdgeStyle[]) {
    const marker = el("marker", {
      id: `${id}-arrow-${style}`,
      class: `arrow-${style}`,
      viewBox: "0 0 10 10",
      refX: 9,
      refY: 5,
      markerWidth: 8,
      markerHeight: 8,
      orient: "auto-start-reverse",
    });
    marker.append(el("path", { d: "M0 0L10 5L0 10z" }));
    defs.append(marker);
  }
  svg.append(defs);
  const content = el("g");
  svg.append(content);

  const groups = [...scene.groups].sort((a, b) => a.depth - b.depth);
  for (const g of groups) content.append(group(g));
  if (scene.view.kind === "runtime") {
    for (const n of scene.nodes) {
      const x = n.rect.x + n.rect.w / 2;
      content.append(
        el("line", { class: "lifeline", x1: x, y1: n.rect.y + n.rect.h, x2: x, y2: scene.size.h - PAD }),
      );
    }
  }
  for (const e of scene.edges) content.append(...edge(e, id));
  const nodes = new Map<string, SVGGElement>();
  for (const n of scene.nodes) {
    const g = node(n, markers.get(n.id) ?? []);
    nodes.set(n.id, g);
    content.append(g);
  }
  return { svg, content, nodes };
}

function group(g: SceneGroup): SVGGElement {
  const out = el("g", { class: `group group-${g.kind}` });
  const r = g.rect;
  out.append(el("rect", { x: r.x, y: r.y, width: r.w, height: r.h, rx: 8 }));
  out.append(el("text", { class: "group-label", x: r.x + 10, y: r.y + 18 }, g.label));
  return out;
}

/** A polyline with rounded corners. */
export function roundedPath(points: Point2[]): string {
  if (points.length === 0) return "";
  let d = `M${points[0].x} ${points[0].y}`;
  for (let i = 1; i < points.length - 1; i++) {
    const [a, b, c] = [points[i - 1], points[i], points[i + 1]];
    const r = Math.min(CORNER, Math.hypot(b.x - a.x, b.y - a.y) / 2, Math.hypot(c.x - b.x, c.y - b.y) / 2);
    const p1 = toward(b, a, r);
    const p2 = toward(b, c, r);
    d += `L${p1.x} ${p1.y}Q${b.x} ${b.y} ${p2.x} ${p2.y}`;
  }
  const last = points[points.length - 1];
  return `${d}L${last.x} ${last.y}`;
}

function toward(from: Point2, to: Point2, dist: number): Point2 {
  const len = Math.hypot(to.x - from.x, to.y - from.y) || 1;
  return { x: from.x + ((to.x - from.x) / len) * dist, y: from.y + ((to.y - from.y) / len) * dist };
}

function edge(e: SceneEdge, id: string): SVGElement[] {
  const path = el("path", {
    class: `edge edge-${e.style}`,
    d: roundedPath(e.points),
    "marker-end": `url(#${id}-arrow-${e.style})`,
  });
  if (!e.label) return [path];
  const i = Math.floor((e.points.length - 1) / 2);
  const [a, b] = [e.points[i], e.points[Math.min(i + 1, e.points.length - 1)]];
  const label = el(
    "text",
    { class: "edge-label", x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 - 6, "text-anchor": "middle" },
    e.label,
  );
  return [path, label];
}

/** Outlines for the standard data shapes: a store is a vertical cylinder and a
 * queue a horizontal pipe. Returns the silhouette and the rim (the near edge
 * of the cylinder's top, or of the pipe's right end). Mirrors `svg.rs`. */
export function cylinderPaths(n: SceneNode): { body: string; rim: string } | null {
  const { x, y, w, h } = n.rect;
  if (n.kind === "store") {
    const [rx, ry, top, bottom] = [w / 2, STORE_CAP, y + STORE_CAP, y + h - STORE_CAP];
    return {
      body: `M${x} ${top}V${bottom}A${rx} ${ry} 0 0 0 ${x + w} ${bottom}V${top}A${rx} ${ry} 0 0 0 ${x} ${top}Z`,
      rim: `M${x} ${top}A${rx} ${ry} 0 0 0 ${x + w} ${top}`,
    };
  }
  if (n.kind === "queue") {
    const [rx, ry, left, right] = [QUEUE_CAP, h / 2, x + QUEUE_CAP, x + w - QUEUE_CAP];
    return {
      body: `M${left} ${y}H${right}A${rx} ${ry} 0 0 1 ${right} ${y + h}H${left}A${rx} ${ry} 0 0 1 ${left} ${y}Z`,
      rim: `M${right} ${y}A${rx} ${ry} 0 0 0 ${right} ${y + h}`,
    };
  }
  return null;
}

function node(n: SceneNode, marks: Marker[]): SVGGElement {
  const r = n.rect;
  const radius = n.kind === "actor" ? 18 : n.kind === "system" ? 12 : 6;
  const g = el("g", { class: `node node-${n.kind}`, "data-id": n.id, tabindex: 0 });
  const shape = cylinderPaths(n);
  if (shape) {
    g.append(
      el("path", { class: "base", d: shape.body }),
      el("path", { class: "tint", d: shape.body }),
      el("path", { class: "rim", d: shape.rim }),
    );
  } else {
    const box = { x: r.x, y: r.y, width: r.w, height: r.h, rx: radius };
    g.append(el("rect", { class: "base", ...box }), el("rect", { class: "tint", ...box }));
  }
  // Text sits below a cylinder's top, and on a pipe's visible face.
  const top = r.y + (n.kind === "store" ? 1.5 * STORE_CAP : 0);
  const cx = r.x + r.w / 2 - (n.kind === "queue" ? QUEUE_CAP / 2 : 0);
  const left = n.kind === "data_card";
  const tx = left ? r.x + 16 : cx;
  const anchor = left ? "start" : "middle";
  g.append(
    el(
      "text",
      { x: tx, y: top + (n.sublabel ? 22 : 25), "text-anchor": anchor, "font-weight": 600 },
      n.label,
    ),
  );
  if (n.sublabel) {
    g.append(el("text", { class: "sub", x: tx, y: top + 40, "text-anchor": anchor }, n.sublabel));
  }
  let y = top + (n.sublabel ? 56 : 40);
  if (n.badges.length > 0) {
    g.append(
      el(
        "text",
        { class: "badge", x: cx, y: y + BADGE_ROW - 8, "text-anchor": "middle" },
        n.badges.join("  "),
      ),
    );
    y += BADGE_ROW;
  }
  for (const f of n.fields) {
    y += FIELD_LINE;
    g.append(el("text", { class: "field", x: r.x + 16, y: y - 3 }, f));
  }
  if (marks.length > 0) {
    const error = marks.some((m) => m.severity === "error");
    const badge = el("g", { class: `marker${error ? " marker-error" : ""}` });
    badge.append(
      el("title", {}, marks.map((m) => `${m.code}: ${m.message}`).join("\n")),
      el("circle", { cx: r.x + r.w - 2, cy: r.y + 2, r: 9 }),
      el("text", { x: r.x + r.w - 2, y: r.y + 6.5, "text-anchor": "middle" }, "!"),
    );
    g.append(badge);
  }
  return g;
}
