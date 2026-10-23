// The tower: a three.js axonometric view. The deployment floor sits at the
// bottom; containers hang above it in tiers, each on dashed leaders from its
// replicas; relations are pipes. Geometry comes from the scene; this module
// draws it, runs hover focus, and plays scenarios back as tokens along the
// pipes.

import {
  BackSide,
  Box3 as Bounds,
  BoxGeometry,
  BufferGeometry,
  CanvasTexture,
  CircleGeometry,
  ConeGeometry,
  CurvePath,
  CylinderGeometry,
  DoubleSide,
  EdgesGeometry,
  ExtrudeGeometry,
  Group,
  InstancedMesh,
  Line,
  LineBasicMaterial,
  LineCurve3,
  LineDashedMaterial,
  LineLoop,
  LineSegments,
  type Material,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  MOUSE,
  type Object3D,
  OrthographicCamera,
  PlaneGeometry,
  QuadraticBezierCurve3,
  Quaternion,
  Raycaster,
  Scene as ThreeScene,
  ShaderMaterial,
  Shape,
  SphereGeometry,
  Sprite,
  SpriteMaterial,
  SRGBColorSpace,
  TOUCH,
  TubeGeometry,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Box3 } from "./generated/Box3";
import type { Pipe } from "./generated/Pipe";
import type { Point3 } from "./generated/Point3";
import type { Scene } from "./generated/Scene";
import type { Span } from "./generated/Span";
import type { Timeline } from "./generated/Timeline";
import type { Tower } from "./generated/Tower";
import type { TimelineStep } from "./generated/TimelineStep";
import type { Marker } from "./markers";

/** Seconds per playback slot. */
const SLOT = 0.95;
/** The lowest the explode control brings the tiers to the floor. */
const GAP_MIN = 2.2;
/** Labels sit this far toward the camera from their anchor, so the element
 * they name never clips them; in an orthographic view this moves nothing on
 * screen. */
const LIFT = 1.2;
/** Faded elements keep this much opacity. */
const DIM = 0.12;
/** Label textures: pixels per world unit. */
const PX = 160;
const SANS = "system-ui, -apple-system, 'Segoe UI', sans-serif";
const MONO = "ui-monospace, Menlo, Consolas, monospace";

export interface TowerHandle {
  state(): TowerState;
  play(scenario?: string): void;
  /** Focuses an element, or clears the focus. */
  select(id: string | null): void;
  step(delta: number): void;
  setMarkers(markers: Map<string, Marker[]>): void;
  destroy(): void;
}

export interface TowerOptions {
  onSelect(id: string, source?: Span): void;
  markers: Map<string, Marker[]>;
  /** State from an earlier tower, kept when scenes are replaced. */
  restore?: TowerState;
}

/** What survives replacing the scene: camera, playback, and toggles. */
export interface TowerState {
  position: [number, number, number];
  target: [number, number, number];
  zoom: number;
  framed: boolean;
  scenario?: string;
  t: number;
  gap: number;
  hidden: string[];
}

type Layer = "containers" | "floor" | "labels";
const LAYERS: [Layer, string][] = [
  ["containers", "Containers"],
  ["floor", "Deployment floor"],
  ["labels", "Labels"],
];

// ---------- Palette ----------

interface Tone {
  lit: Vector3;
  shade: Vector3;
}

interface Palette {
  neutral: Tone;
  accent: Tone;
  plate: Tone;
  ink: string;
  thin: string;
  accentInk: string;
  muted: string;
  zone: string;
  tierTint: string;
  dark: string;
  card: string;
  cardLine: string;
  fg: string;
}

/** Colors come from CSS custom properties, so the tower follows the
 * viewer's light and dark themes. */
function palette(el: HTMLElement): Palette {
  const css = getComputedStyle(el);
  const get = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  const vec = (name: string, fallback: string) => {
    const hex = get(name, fallback).replace("#", "");
    const n = Number.parseInt(hex.length === 3 ? [...hex].map((c) => c + c).join("") : hex, 16);
    return new Vector3(((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255);
  };
  return {
    neutral: { lit: vec("--saxl-axo-lit", "#ffffff"), shade: vec("--saxl-axo-shade", "#e1e1e7") },
    accent: { lit: vec("--saxl-axo-accent-lit", "#fcefea"), shade: vec("--saxl-axo-accent-shade", "#edbfaf") },
    plate: { lit: vec("--saxl-axo-lit", "#ffffff"), shade: vec("--saxl-axo-plate-shade", "#d8d8de") },
    ink: get("--saxl-axo-ink", "#2e3040"),
    thin: get("--saxl-axo-thin", "#9a9cab"),
    accentInk: get("--saxl-axo-accent", "#b5472b"),
    muted: get("--saxl-axo-muted", "#8a8d9e"),
    zone: get("--saxl-axo-zone", "#f7f7f9"),
    tierTint: get("--saxl-axo-tier", "#dcdde4"),
    dark: get("--saxl-axo-opening", "#dcdce2"),
    card: get("--saxl-axo-card", "#ffffff"),
    cardLine: get("--saxl-axo-card-line", "#d4d4da"),
    fg: get("--saxl-fg", "#2b2d3a"),
  };
}

// ---------- Materials ----------

/** Faces are toned by their view-space normal against a light fixed to the
 * camera, so tops read lightest and the two visible sides stay distinct at
 * any angle. Output is written as given (sRGB in, sRGB out). */
function faceMaterial(tone: Tone, opacity = 1): ShaderMaterial {
  return new ShaderMaterial({
    uniforms: { uLit: { value: tone.lit }, uShade: { value: tone.shade }, uOpacity: { value: opacity } },
    vertexShader: `varying vec3 vN;
      void main() { vN = normalize(normalMatrix * normal); gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
    fragmentShader: `uniform vec3 uLit; uniform vec3 uShade; uniform float uOpacity; varying vec3 vN;
      void main() {
        float t = 0.945 + dot(normalize(vN), vec3(-0.05, 0.088, 0.0));
        float s = clamp((t - 0.874) / 0.126, 0.0, 1.0);
        gl_FragColor = vec4(mix(uShade, uLit, s), uOpacity);
      }`,
    transparent: opacity < 1,
    depthWrite: opacity >= 1,
    polygonOffset: true,
    polygonOffsetFactor: 1,
    polygonOffsetUnits: 1,
  });
}

/** Silhouette for round shapes: back faces pushed out along the normal by a
 * screen-constant width (`width.value` is updated from the zoom every frame). */
function hullMaterial(color: string, width: { value: number }): ShaderMaterial {
  const c = new Vector3(...hexRgb(color));
  return new ShaderMaterial({
    uniforms: { uT: width, uColor: { value: c } },
    vertexShader: `uniform float uT; void main() { gl_Position = projectionMatrix * modelViewMatrix * vec4(position + normal * uT, 1.0); }`,
    fragmentShader: `uniform vec3 uColor; void main() { gl_FragColor = vec4(uColor, 1.0); }`,
    side: BackSide,
  });
}

function hexRgb(color: string): [number, number, number] {
  const hex = color.replace("#", "");
  const n = Number.parseInt(hex.length === 3 ? [...hex].map((c) => c + c).join("") : hex, 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

/** A material role on a mesh: dimming and scenario highlights swap the
 * material by role. */
type Role = "face" | "hull" | "edge" | "inner" | "flat";

interface Theme {
  face: Material;
  hull: Material;
  edge: Material;
  inner: Material;
}

const v3 = (p: Point3) => new Vector3(p.x, p.y, p.z);

function tag<T extends Object3D>(o: T, role: Role): T {
  o.userData.role = role;
  return o;
}

function h<K extends keyof HTMLElementTagNameMap>(tagName: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tagName);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
}

function easeInOut(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
}

const circle = (r: number, n = 48) =>
  Array.from({ length: n }, (_, i) => {
    const a = (i / n) * Math.PI * 2;
    return [Math.cos(a) * r, Math.sin(a) * r] as const;
  });

/** Polyline with rounded corners: one smooth tube, one outline. */
function bentPath(points: Vector3[], r: number): CurvePath<Vector3> {
  const path = new CurvePath<Vector3>();
  let from = points[0];
  for (let i = 1; i < points.length - 1; i++) {
    const [a, c, b] = [points[i - 1], points[i], points[i + 1]];
    const p1 = c.clone().addScaledVector(a.clone().sub(c).normalize(), Math.min(r, c.distanceTo(a) / 2));
    const p2 = c.clone().addScaledVector(b.clone().sub(c).normalize(), Math.min(r, c.distanceTo(b) / 2));
    path.add(new LineCurve3(from, p1));
    path.add(new QuadraticBezierCurve3(p1, c.clone(), p2));
    from = p2;
  }
  path.add(new LineCurve3(from, points[points.length - 1]));
  return path;
}

const lengthOf = (p: Vector3[]) => p.slice(1).reduce((s, q, i) => s + q.distanceTo(p[i]), 0);

function pointAt(p: Vector3[], d: number): Vector3 {
  for (let i = 0; i < p.length - 1; i++) {
    const l = p[i].distanceTo(p[i + 1]);
    if (d <= l) return p[i].clone().lerp(p[i + 1], l === 0 ? 1 : d / l);
    d -= l;
  }
  return p[p.length - 1].clone();
}

/** Person silhouette, feet at the origin, about 1.45 tall. */
function personShape(): Shape[] {
  const p = new Shape();
  p.moveTo(-0.09, 0);
  p.lineTo(-0.13, 0.62);
  p.lineTo(-0.2, 0.66);
  p.lineTo(-0.2, 1.08);
  p.quadraticCurveTo(-0.2, 1.16, -0.12, 1.16);
  p.lineTo(0.12, 1.16);
  p.quadraticCurveTo(0.2, 1.16, 0.2, 1.08);
  p.lineTo(0.2, 0.66);
  p.lineTo(0.13, 0.62);
  p.lineTo(0.09, 0);
  p.lineTo(0.02, 0);
  p.lineTo(0, 0.5);
  p.lineTo(-0.02, 0);
  p.closePath();
  const head = new Shape();
  head.absarc(0, 1.3, 0.12, 0, Math.PI * 2, false);
  return [p, head];
}

export function mountTower(stage: HTMLElement, scene: Scene, opts: TowerOptions): TowerHandle {
  if (!scene.tower) throw new Error("scene has no tower");
  const tower: Tower = scene.tower;
  const colors = palette(stage);
  const restore = opts.restore;

  const renderer = new WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.outputColorSpace = SRGBColorSpace;
  const canvas = renderer.domElement;
  canvas.className = "saxl-canvas";
  stage.append(canvas);

  const world = new ThreeScene();
  const floor = new Group();
  const upper = new Group();
  const leaders = new Group();
  world.add(floor, upper, leaders);

  // ---------- Themes ----------
  const outline = { value: 0.03 };
  const pipeOutline = { value: 0.02 };
  const lineMat = (color: string) => new LineBasicMaterial({ color });
  const themes = {
    neutral: { face: faceMaterial(colors.neutral), hull: hullMaterial(colors.ink, outline), edge: lineMat(colors.ink), inner: lineMat(colors.thin) },
    accent: { face: faceMaterial(colors.accent), hull: hullMaterial(colors.accentInk, outline), edge: lineMat(colors.accentInk), inner: lineMat(colors.accentInk) },
    ghost: {
      face: faceMaterial(colors.neutral, 0.5),
      hull: hullMaterial(colors.ink, outline),
      edge: new LineDashedMaterial({ color: colors.muted, dashSize: 0.12, gapSize: 0.09 }),
      inner: lineMat(colors.thin),
    },
    plate: { face: faceMaterial(colors.plate), hull: hullMaterial(colors.ink, outline), edge: lineMat(colors.ink), inner: lineMat(colors.thin) },
    // A hook: see-through like a ghost, with accent dashes, since it is code
    // the library calls but does not own.
    socket: {
      face: faceMaterial(colors.neutral, 0.5),
      hull: hullMaterial(colors.accentInk, outline),
      edge: new LineDashedMaterial({ color: colors.accentInk, dashSize: 0.12, gapSize: 0.09 }),
      inner: lineMat(colors.accentInk),
    },
  } satisfies Record<string, Theme>;
  const pipeThemes = {
    neutral: { ...themes.neutral, hull: hullMaterial(colors.ink, pipeOutline) },
    accent: { ...themes.accent, hull: hullMaterial(colors.accentInk, pipeOutline) },
  } satisfies Record<string, Theme>;

  // ---------- Primitives ----------
  function edges(mesh: Mesh, theme: Theme, angle = 20) {
    const e = tag(new LineSegments(new EdgesGeometry(mesh.geometry, angle), theme.edge), "edge");
    if ((theme.edge as LineDashedMaterial).isLineDashedMaterial) e.computeLineDistances();
    mesh.add(e);
  }
  function loop(parent: Object3D, points: Vector3[], mat: Material, role: Role = "inner") {
    const l = tag(new LineLoop(new BufferGeometry().setFromPoints(points), mat), role);
    parent.add(l);
    return l;
  }
  function solid(parent: Object3D, geometry: BufferGeometry, pos: Vector3 | null, theme: Theme): Mesh {
    const m = tag(new Mesh(geometry, theme.face), "face");
    if (pos) m.position.copy(pos);
    m.add(tag(new Mesh(geometry, theme.hull), "hull"));
    parent.add(m);
    return m;
  }
  function box(parent: Object3D, center: Vector3, size: Vector3, theme: Theme, inset = 0): Mesh {
    const m = tag(new Mesh(new BoxGeometry(size.x, size.y, size.z), theme.face), "face");
    m.position.copy(center);
    edges(m, theme);
    if (inset > 0) {
      const [a, b, top] = [size.x / 2 - inset, size.z / 2 - inset, size.y / 2 + 0.003];
      loop(m, [[-a, -b], [a, -b], [a, b], [-a, b]].map(([p, q]) => new Vector3(p, top, q)), theme.inner);
    }
    parent.add(m);
    return m;
  }
  function cylinder(parent: Object3D, center: Vector3, r: number, height: number, theme: Theme, bands: number[] = [], ring = 0): Mesh {
    const g = new CylinderGeometry(r, r, height, 64);
    const m = solid(parent, g, center, theme);
    edges(m, theme);
    for (const k of bands) loop(m, circle(r * 1.004, 64).map(([p, q]) => new Vector3(p, -height / 2 + height * k, q)), theme.inner);
    if (ring) loop(m, circle(r * ring, 64).map(([p, q]) => new Vector3(p, height / 2 + 0.003, q)), theme.inner);
    return m;
  }
  /** A queue: a pipe along x with seams, open mouth at +x. */
  function queuePipe(parent: Object3D, center: Vector3, len: number, r: number, theme: Theme): Mesh {
    const m = solid(parent, new CylinderGeometry(r, r, len, 64).rotateZ(Math.PI / 2), center, theme);
    edges(m, theme);
    for (let x = 0.6; x < len - 0.05; x += 0.6) loop(m, circle(r * 1.004, 48).map(([p, q]) => new Vector3(x - len / 2, p, q)), theme.inner);
    const mouth = tag(new Mesh(new CircleGeometry(r * 0.62, 48).rotateY(Math.PI / 2), new MeshBasicMaterial({ color: colors.dark })), "flat");
    mouth.position.x = len / 2 + 0.004;
    m.add(mouth);
    loop(m, circle(r * 0.62, 48).map(([p, q]) => new Vector3(len / 2 + 0.006, p, q)), theme.inner);
    return m;
  }
  /** An actor: a flat scale figure at a terminal on a floating base. Pipes
   * plug into the terminal's +x face. */
  function actor(parent: Object3D, b: Box3) {
    const base = b.center.y - b.size.y / 2;
    const theme = themes.neutral;
    cylinder(parent, new Vector3(b.center.x - 0.05, base - 0.03, b.center.z), 0.75, 0.06, theme);
    const geo = new ExtrudeGeometry(personShape(), { depth: 0.05, bevelEnabled: false }).translate(0, 0, -0.025).rotateY(Math.PI / 4);
    const fig = tag(new Mesh(geo, theme.face), "face");
    fig.position.set(b.center.x - 0.35, base, b.center.z + 0.05);
    edges(fig, theme, 30);
    parent.add(fig);
    return box(parent, new Vector3(b.center.x + 0.15, base + 0.425, b.center.z), new Vector3(0.3, 0.85, 0.56), theme, 0.06);
  }
  function patch(parent: Object3D, x0: number, z0: number, x1: number, z1: number, y: number, color: string, opacity = 1) {
    const m = tag(
      new Mesh(
        new PlaneGeometry(x1 - x0, z1 - z0).rotateX(-Math.PI / 2),
        new MeshBasicMaterial({ color, transparent: opacity < 1, opacity, depthWrite: opacity >= 1, side: DoubleSide }),
      ),
      "flat",
    );
    m.position.set((x0 + x1) / 2, y, (z0 + z1) / 2);
    parent.add(m);
    return m;
  }
  /** A dotted line: small spheres, so it reads from any angle. */
  function dotted(parent: Object3D, a: Vector3, b: Vector3, step: number, color: string, r: number) {
    const n = Math.max(1, Math.floor(a.distanceTo(b) / step));
    const mesh = new InstancedMesh(new SphereGeometry(r, 8, 6), new MeshBasicMaterial({ color }), n + 1);
    for (let i = 0; i <= n; i++) mesh.setMatrixAt(i, new Matrix4().makeTranslation(a.clone().lerp(b, i / n)));
    parent.add(tag(mesh, "flat"));
  }
  /** Text painted flat on a floor, reading along +x. */
  function floorText(parent: Object3D, at: Vector3, text: string, size = 0.4) {
    const c = document.createElement("canvas");
    const ctx = c.getContext("2d");
    if (!ctx) return;
    const px = 64;
    const font = `500 ${px}px ${MONO}`;
    ctx.font = font;
    ctx.letterSpacing = `${px * 0.25}px`;
    c.width = Math.ceil(ctx.measureText(text).width) + 8;
    c.height = Math.ceil(px * 1.3);
    ctx.font = font;
    ctx.letterSpacing = `${px * 0.25}px`;
    ctx.fillStyle = colors.muted;
    ctx.fillText(text, 2, px);
    const tex = new CanvasTexture(c);
    tex.colorSpace = SRGBColorSpace;
    tex.anisotropy = 8;
    const [w, hh] = [(size * c.width) / px, (size * c.height) / px];
    const m = new Mesh(new PlaneGeometry(w, hh).rotateX(-Math.PI / 2), new MeshBasicMaterial({ map: tex, transparent: true, depthWrite: false }));
    m.position.set(at.x + w / 2, at.y + 0.012, at.z - hh / 2 + size * 0.3);
    parent.add(tag(m, "flat"));
  }

  // ---------- Labels: depth-tested sprites ----------
  const floating: { sprite: Sprite; anchor: Vector3; base: Vector3; kind: "label" | "marker" }[] = [];
  const measureCtx = document.createElement("canvas").getContext("2d");
  const measure = (font: string, text: string, spacing = 0) => {
    if (!measureCtx) return text.length * 10;
    measureCtx.font = font;
    measureCtx.letterSpacing = `${spacing}px`;
    return measureCtx.measureText(text).width;
  };
  function sprite(parent: Object3D, anchor: Vector3, w: number, hh: number, paint: (ctx: CanvasRenderingContext2D) => void, center: [number, number], kind: "label" | "marker" = "label", onTop = false) {
    const c = document.createElement("canvas");
    c.width = Math.ceil(w);
    c.height = Math.ceil(hh);
    const ctx = c.getContext("2d");
    if (ctx) paint(ctx);
    const tex = new CanvasTexture(c);
    tex.colorSpace = SRGBColorSpace;
    tex.anisotropy = 4;
    const s = new Sprite(new SpriteMaterial({ map: tex, transparent: true, depthWrite: false, depthTest: !onTop }));
    if (onTop) s.renderOrder = 10;
    s.scale.set(c.width / PX, c.height / PX, 1);
    s.center.set(...center);
    parent.add(s);
    floating.push({ sprite: s, anchor, base: s.scale.clone(), kind });
    return s;
  }
  function text(ctx: CanvasRenderingContext2D, str: string, x: number, y: number, font: string, color: string, spacing = 0, align: CanvasTextAlign = "center") {
    ctx.font = font;
    ctx.letterSpacing = `${spacing}px`;
    ctx.fillStyle = color;
    ctx.textAlign = align;
    ctx.fillText(str, x, y);
  }
  function frame(ctx: CanvasRenderingContext2D, w: number, hh: number, r: number, fill: string, stroke: string, lw: number) {
    ctx.beginPath();
    ctx.roundRect(lw / 2, lw / 2, w - lw, hh - lw, r);
    ctx.fillStyle = fill;
    ctx.fill();
    ctx.lineWidth = lw;
    ctx.strokeStyle = stroke;
    ctx.stroke();
  }
  /** Name card: bold name over a spaced mono subtitle, bottom edge at the anchor. */
  function card(parent: Object3D, anchor: Vector3, title: string, sub: string | null, small: boolean) {
    const tS = (small ? 0.34 : 0.42) * PX;
    const sS = 0.27 * PX;
    const sp = sS * 0.14;
    const tF = `600 ${tS}px ${SANS}`;
    const sF = `400 ${sS}px ${MONO}`;
    const w = Math.max(measure(tF, title), sub ? measure(sF, sub, sp) : 0) + 0.6 * PX;
    const hh = tS + (sub ? sS + 0.08 * PX : 0) + 0.34 * PX;
    return sprite(parent, anchor, w, hh, (ctx) => {
      frame(ctx, w, hh, 0.06 * PX, colors.card, colors.cardLine, 0.025 * PX);
      text(ctx, title, w / 2, 0.13 * PX + tS * 0.9, tF, colors.fg);
      if (sub) text(ctx, sub, w / 2 + sp / 2, 0.13 * PX + tS + 0.08 * PX + sS * 0.85, sF, colors.muted, sp);
    }, [0.5, 0]);
  }
  function pill(parent: Object3D, anchor: Vector3, str: string) {
    const fS = 0.26 * PX;
    const F = `500 ${fS}px ${MONO}`;
    const w = measure(F, str, fS * 0.06) + 0.36 * PX;
    const hh = fS + 0.16 * PX;
    return sprite(parent, anchor, w, hh, (ctx) => {
      frame(ctx, w, hh, hh / 2, colors.card, colors.cardLine, 0.025 * PX);
      text(ctx, str, w / 2, hh / 2 + fS * 0.34, F, colors.muted, fS * 0.06);
    }, [0.5, 0.5]);
  }

  // ---------- Floor ----------
  for (const s of tower.slabs) {
    const region = s.sublabel === "region";
    const c = v3(s.center);
    const size = v3(s.size);
    if (region) {
      box(floor, c, size, themes.plate);
      continue;
    }
    // A group zone: tinted floor with a low curb.
    const [x0, x1, z0, z1] = [c.x - size.x / 2, c.x + size.x / 2, c.z - size.z / 2, c.z + size.z / 2];
    patch(floor, x0, z0, x1, z1, 0.004, colors.zone);
    const [cw, ch] = [0.15, size.y];
    box(floor, new Vector3(x0 + cw / 2, ch / 2, c.z), new Vector3(cw, ch, size.z), themes.neutral);
    box(floor, new Vector3(x1 - cw / 2, ch / 2, c.z), new Vector3(cw, ch, size.z), themes.neutral);
    box(floor, new Vector3(c.x, ch / 2, z0 + cw / 2), new Vector3(size.x, ch, cw), themes.neutral);
    box(floor, new Vector3(c.x, ch / 2, z1 - cw / 2), new Vector3(size.x, ch, cw), themes.neutral);
  }
  for (const l of tower.labels) floorText(floor, v3(l.position), l.text, 0.42);

  // Per-element groups, so focus can fade everything else.
  const focusKey = (o: Object3D) => o.userData.focus as { el?: string; edge?: string; from?: string; to?: string } | undefined;
  const elementGroups = new Map<string, Group>();
  const podGroups = new Map<string, Group>();
  const groupFor = (map: Map<string, Group>, parent: Object3D, el: string) => {
    let g = map.get(el);
    if (!g) {
      g = new Group();
      g.userData.focus = { el };
      parent.add(g);
      map.set(el, g);
    }
    return g;
  };
  for (const p of tower.pods) {
    const g = groupFor(podGroups, floor, p.element);
    const b = p.geometry;
    box(g, v3(b.center), v3(b.size), themes.neutral, p.managed ? 0.15 : 0.1);
  }
  for (const t of tower.tags) pill(groupFor(podGroups, floor, t.element), v3(t.position), t.text);
  const boxes = new Map<string, Box3>();
  const pickable: Object3D[] = [];
  for (const b of tower.infra) {
    const m = box(floor, v3(b.center), v3(b.size), themes.neutral, 0.15);
    m.userData.id = b.id;
    boxes.set(b.id, b);
    pickable.push(m);
    card(floor, new Vector3(b.center.x, b.center.y + b.size.y / 2 + 0.2, b.center.z), b.label ?? b.id, b.sublabel, true);
  }

  // ---------- Suspended containers ----------
  for (const b of [...tower.containers, ...tower.outside]) {
    const g = groupFor(elementGroups, upper, b.id);
    const c = v3(b.center);
    let hit: Mesh;
    switch (b.kind) {
      case "store":
        hit = cylinder(g, c, b.size.x / 2, b.size.y, themes.neutral, [0.35, 0.68], 0.7);
        break;
      case "queue":
        hit = queuePipe(g, c, b.size.x, b.size.y / 2, themes.neutral);
        break;
      case "actor":
      case "consumer":
        hit = actor(g, b);
        break;
      case "external":
      case "package":
        hit = box(g, c, v3(b.size), themes.ghost, 0.12);
        break;
      case "hook":
        hit = box(g, c, v3(b.size), themes.socket, 0.12);
        break;
      default:
        hit = box(g, c, v3(b.size), themes.neutral, 0.18);
    }
    hit.userData.id = b.id;
    g.traverse((o) => {
      if (o instanceof Mesh && o.userData.role === "face") o.userData.id = b.id;
    });
    boxes.set(b.id, b);
    pickable.push(g);
    const small = ["actor", "external", "consumer", "package", "hook"].includes(b.kind);
    card(g, new Vector3(b.center.x, b.center.y + b.size.y / 2 + 0.2, b.center.z), b.label ?? b.id, b.sublabel, small);
  }

  // Tier outlines: a see-through tint and a light dotted edge.
  for (const t of tower.tiers) {
    const [a, b] = [v3(t.min), v3(t.max)];
    patch(upper, a.x, a.z, b.x, b.z, a.y, colors.tierTint, 0.25);
    const corners = [new Vector3(a.x, a.y, a.z), new Vector3(b.x, a.y, a.z), new Vector3(b.x, a.y, b.z), new Vector3(a.x, a.y, b.z)];
    for (let i = 0; i < 4; i++) dotted(upper, corners[i], corners[(i + 1) % 4], 0.22, colors.thin, 0.035);
  }

  // System or library boundary: a dotted box.
  {
    const [a, b] = [v3(tower.boundary.min), v3(tower.boundary.max)];
    const xz = [[a.x, a.z], [b.x, a.z], [b.x, b.z], [a.x, b.z]];
    for (let i = 0; i < 4; i++) {
      const [p, q] = [xz[i], xz[(i + 1) % 4]];
      dotted(upper, new Vector3(p[0], a.y, p[1]), new Vector3(q[0], a.y, q[1]), 0.32, colors.muted, 0.045);
      dotted(upper, new Vector3(p[0], b.y, p[1]), new Vector3(q[0], b.y, q[1]), 0.32, colors.muted, 0.045);
      dotted(upper, new Vector3(p[0], a.y, p[1]), new Vector3(p[0], b.y, p[1]), 0.32, colors.muted, 0.045);
    }
    floorText(upper, new Vector3(a.x + 0.4, a.y, b.z - 0.3), `${tower.boundary.label.toUpperCase()} · ${tower.boundary.kind === "library" ? "LIBRARY" : "SYSTEM"} BOUNDARY`);
  }

  // ---------- Pipes ----------
  const pipeGroups = new Map<string, Group>();
  const trunkGroups: Group[] = [];
  const pipesById = new Map(tower.pipes.map((p) => [p.id, p] as const));
  function drawPipe(parent: Object3D, points: Vector3[], theme: Theme, r: number, isAsync: boolean, ends: [boolean, boolean], cone: boolean) {
    const ext = points.map((p) => p.clone());
    const n = ext.length;
    if (n < 2) return;
    // Ends that meet a container run into it, so the tube never stops short.
    if (ends[0]) ext[0].addScaledVector(ext[0].clone().sub(ext[1]).normalize(), 0.2);
    if (ends[1]) ext[n - 1].addScaledVector(ext[n - 1].clone().sub(ext[n - 2]).normalize(), 0.2);
    const curve = bentPath(ext, 0.45);
    const len = curve.getLength();
    solid(parent, new TubeGeometry(curve, Math.max(8, Math.ceil(len * 16)), r, 16, false), null, theme);
    if (isAsync) {
      for (let d = 0.5; d < len - 0.4; d += 0.55) {
        const u = d / len;
        const ring = loop(parent, circle(r * 1.02, 24).map(([p, q]) => new Vector3(p, q, 0)), theme.edge, "edge");
        ring.position.copy(curve.getPointAt(u));
        ring.quaternion.setFromUnitVectors(new Vector3(0, 0, 1), curve.getTangentAt(u));
      }
    }
    const up = new Vector3(0, 1, 0);
    [[points[0], points[1]], [points[n - 1], points[n - 2]]].forEach(([p, q], k) => {
      if (!ends[k]) return;
      const dir = p.clone().sub(q).normalize();
      const g = new CylinderGeometry(r + 0.07, r + 0.07, 0.1, 24).applyQuaternion(new Quaternion().setFromUnitVectors(up, dir));
      solid(parent, g, p.clone().addScaledVector(dir, -0.04), theme);
    });
    if (!cone) return;
    // Direction: on the last straight run long enough to hold the cone clear of bends.
    let j = n - 1;
    while (j > 1 && points[j].distanceTo(points[j - 1]) < 1.2) j--;
    const dir = points[j].clone().sub(points[j - 1]).normalize();
    const g = new ConeGeometry(0.17, 0.3, 24).applyQuaternion(new Quaternion().setFromUnitVectors(up, dir));
    solid(parent, g, points[j].clone().addScaledVector(dir, -0.65), theme);
  }
  function buildPipes(highlight: Set<string>) {
    for (const g of [...pipeGroups.values(), ...trunkGroups]) {
      g.removeFromParent();
      g.traverse((o) => {
        if (o instanceof Mesh || o instanceof Line) o.geometry.dispose();
      });
    }
    pipeGroups.clear();
    trunkGroups.length = 0;
    for (const p of tower.pipes) {
      const g = new Group();
      g.userData.focus = { edge: p.id, from: p.from, to: p.to };
      const theme = highlight.has(p.id) ? pipeThemes.accent : pipeThemes.neutral;
      const branch = p.trunk !== null;
      const points = p.points.map(v3);
      drawPipe(g, points, theme, p.radius, p.async, [!branch, true], true);
      if (branch && points.length) solid(g, new SphereGeometry(p.radius + 0.09, 20, 12), points[0], pipeThemes.neutral); // tee
      upper.add(g);
      pipeGroups.set(p.id, g);
    }
    for (const t of tower.trunks) {
      const g = new Group();
      g.userData.focus = { el: t.hub };
      g.userData.pipes = t.pipes;
      drawPipe(g, t.stub.map(v3), pipeThemes.neutral, t.radius, false, [true, false], false);
      drawPipe(g, t.run.map(v3), pipeThemes.neutral, t.radius, false, [false, false], false);
      for (const p of t.run) solid(g, new SphereGeometry(t.radius, 20, 12), v3(p), pipeThemes.neutral); // capped ends
      upper.add(g);
      trunkGroups.push(g);
    }
  }

  // ---------- Leaders and explode ----------
  const level = tower.level;
  let gap = Math.min(level, Math.max(GAP_MIN, restore?.gap ?? level));
  const leaderMat = new LineDashedMaterial({ color: colors.thin, dashSize: 0.14, gapSize: 0.14 });
  function setGap(next: number) {
    gap = next;
    upper.position.y = gap - level;
    for (const c of [...leaders.children]) {
      leaders.remove(c);
      if (c instanceof Line) c.geometry.dispose();
    }
    for (const l of tower.leaders) {
      const to = v3(l.to);
      to.y += gap - level;
      const line = new Line(new BufferGeometry().setFromPoints([v3(l.from), to]), leaderMat);
      line.computeLineDistances();
      line.userData.el = l.element;
      leaders.add(line);
    }
    applyFocus();
  }

  // ---------- Markers ----------
  let markers = opts.markers;
  const markerGroup = new Group();
  upper.add(markerGroup);
  function drawMarkers() {
    for (let i = floating.length - 1; i >= 0; i--) if (floating[i].kind === "marker") floating.splice(i, 1);
    markerGroup.clear();
    for (const [id, list] of markers) {
      const b = boxes.get(id);
      if (!b || list.length === 0) continue;
      const error = list.some((m) => m.severity === "error");
      const d = 0.5 * PX;
      const parent = tower.infra.some((x) => x.id === id) ? floor : markerGroup;
      const at = new Vector3(b.center.x + b.size.x / 2, b.center.y + b.size.y / 2 + 0.15, b.center.z);
      sprite(parent, at, d, d, (ctx) => {
        ctx.beginPath();
        ctx.arc(d / 2, d / 2, d / 2 - 2, 0, Math.PI * 2);
        ctx.fillStyle = error ? colors.accentInk : "#f5b742";
        ctx.fill();
        text(ctx, "!", d / 2, d / 2 + 0.11 * PX, `700 ${0.32 * PX}px ${SANS}`, error ? "#fff" : "#151a22");
      }, [0.5, 0.5], "marker", true);
    }
  }

  // ---------- Focus and dimming ----------
  const dimCache = new Map<Material, Material>();
  function dimOf(m: Material): Material {
    let d = dimCache.get(m);
    if (d) return d;
    d = m.clone();
    if (m instanceof ShaderMaterial && m.uniforms.uOpacity) (d as ShaderMaterial).uniforms = { ...m.uniforms, uOpacity: { value: DIM } };
    else d.opacity = DIM * m.opacity;
    d.transparent = true;
    d.depthWrite = false;
    dimCache.set(m, d);
    return d;
  }
  function setDim(obj: Object3D, dim: boolean) {
    obj.traverse((o) => {
      if (o instanceof Sprite) {
        o.material.opacity = dim ? 0.15 : 1;
        return;
      }
      const m = (o as Mesh).material as Material | undefined;
      if (!m || Array.isArray(m)) return;
      if (o.userData.role === "hull") {
        o.visible = !dim;
        return;
      }
      o.userData.mat0 ??= m;
      (o as Mesh).material = dim ? dimOf(o.userData.mat0) : o.userData.mat0;
    });
  }
  type Focus = { el?: string; edge?: string; from?: string; to?: string } | null;
  let hovered: Focus = null;
  let pinned: Focus = null;
  let scenarioFocus: { els: Set<string>; edges: Set<string> } | null = null;
  function applyFocus() {
    const f = pinned ?? hovered;
    let els: Set<string> | null = null;
    let edgesOn: Set<string> | null = null;
    if (f?.el) {
      els = new Set([f.el]);
      edgesOn = new Set();
      for (const p of tower.pipes) {
        if (p.from === f.el || p.to === f.el) {
          els.add(p.from);
          els.add(p.to);
          edgesOn.add(p.id);
        }
      }
    } else if (f?.edge) {
      els = new Set([f.from ?? "", f.to ?? ""]);
      edgesOn = new Set([f.edge]);
    } else if (scenarioFocus) {
      ({ els, edges: edgesOn } = scenarioFocus);
    }
    const on = (s: Set<string> | null, k: string) => s === null || s.has(k);
    for (const [k, g] of elementGroups) setDim(g, !on(els, k));
    for (const [k, g] of podGroups) setDim(g, !on(els, k));
    for (const [k, g] of pipeGroups) setDim(g, !on(edgesOn, k));
    for (const g of trunkGroups) setDim(g, edgesOn !== null && !(g.userData.pipes as string[]).some((k) => edgesOn?.has(k)));
    for (const l of leaders.children) setDim(l, !on(els, l.userData.el));
  }

  // ---------- Camera ----------
  const camera = new OrthographicCamera(-1, 1, 1, -1, 0.1, 4000);
  const iso = new Vector3(1, 1, 1).normalize();
  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = true;
  controls.minZoom = 0.3;
  controls.maxZoom = 8;
  controls.screenSpacePanning = true;
  controls.mouseButtons = { LEFT: MOUSE.ROTATE, MIDDLE: MOUSE.DOLLY, RIGHT: MOUSE.ROTATE };
  controls.touches = { ONE: TOUCH.ROTATE, TWO: TOUCH.DOLLY_ROTATE };

  // Fit: project the drawing's bounds onto the iso view's screen axes.
  buildPipes(new Set());
  setGap(level);
  const bounds = new Vector3();
  const centre = new Vector3();
  {
    const pts: Vector3[] = [];
    world.updateMatrixWorld(true);
    world.traverse((o) => {
      if (!(o instanceof Mesh) || o instanceof InstancedMesh) return;
      o.geometry.computeBoundingBox();
      const bb = o.geometry.boundingBox;
      if (!bb) return;
      for (const x of [bb.min.x, bb.max.x]) for (const y of [bb.min.y, bb.max.y]) for (const z of [bb.min.z, bb.max.z]) pts.push(new Vector3(x, y, z).applyMatrix4(o.matrixWorld));
    });
    const lo = pts.reduce((a, p) => a.min(p), new Vector3(Infinity, Infinity, Infinity));
    const hi = pts.reduce((a, p) => a.max(p), new Vector3(-Infinity, -Infinity, -Infinity));
    centre.addVectors(lo, hi).multiplyScalar(0.5);
    const right = new Vector3(1, 0, -1).normalize();
    const up = new Vector3(-1, 2, -1).normalize();
    for (const p of pts) {
      const d = p.clone().sub(centre);
      bounds.x = Math.max(bounds.x, Math.abs(d.dot(right)));
      bounds.y = Math.max(bounds.y, Math.abs(d.dot(up)));
    }
    bounds.multiplyScalar(1.06);
  }
  const home = () => {
    controls.target.copy(centre);
    camera.position.copy(centre).addScaledVector(iso, 200);
    camera.zoom = 1;
    camera.updateProjectionMatrix();
  };
  let framed = restore?.framed ?? false;
  home();

  // ---------- Labels and markers after the fit, so they don't widen it ----------
  drawMarkers();

  // ---------- Panel ----------
  const shown: Record<Layer, boolean> = { containers: true, floor: true, labels: true };
  for (const key of restore?.hidden ?? []) if (key in shown) shown[key as Layer] = false;
  const panel = h("div", "saxl-tower-panel");
  const view = h("fieldset");
  view.append(h("legend", undefined, "View"));
  const isoButton = h("button", undefined, "Iso view");
  isoButton.type = "button";
  const explodeLabel = h("label");
  const explode = h("input");
  explode.type = "range";
  explode.min = String(GAP_MIN);
  explode.max = String(level);
  explode.step = "0.01";
  explode.value = String(gap);
  explode.setAttribute("aria-label", "Explode");
  explodeLabel.append("Explode ", explode);
  view.append(isoButton, explodeLabel);
  for (const [key, label] of LAYERS) {
    const l = h("label");
    const box = h("input");
    box.type = "checkbox";
    box.checked = shown[key];
    box.addEventListener("change", () => {
      shown[key] = box.checked;
      applyVisibility();
    });
    l.append(box, ` ${label}`);
    view.append(l);
  }
  panel.append(view);

  const timelines = scene.timelines;
  let timeline: Timeline | undefined = timelines.find((tl) => tl.scenario === restore?.scenario) ?? timelines[0];
  let t = 0;
  let playing = false;
  const slots = (tl?: Timeline) => (tl ? Math.max(0, ...tl.steps.map((s) => s.index + 1)) : 0);
  const play = h("fieldset");
  play.append(h("legend", undefined, "Scenario"));
  const select = h("select");
  select.setAttribute("aria-label", "Scenario");
  for (const tl of timelines) {
    const option = h("option", undefined, tl.scenario);
    option.value = tl.scenario;
    select.append(option);
  }
  const playButton = h("button", undefined, "Play");
  playButton.type = "button";
  const back = h("button", undefined, "◀");
  back.type = "button";
  back.setAttribute("aria-label", "Previous step");
  const forward = h("button", undefined, "▶");
  forward.type = "button";
  forward.setAttribute("aria-label", "Next step");
  const scrub = h("input");
  scrub.type = "range";
  scrub.min = "0";
  scrub.step = "0.01";
  scrub.setAttribute("aria-label", "Playback position");
  const caption = h("div", "saxl-step");
  const row = h("div", "saxl-row");
  row.append(back, playButton, forward);
  play.append(select, row, scrub, caption);
  if (timelines.length > 0) panel.append(play);
  stage.append(panel);

  const hoverCard = h("div", "saxl-card");
  hoverCard.hidden = true;
  stage.append(hoverCard);

  function applyVisibility() {
    upper.visible = shown.containers;
    floor.visible = shown.floor;
    leaders.visible = shown.containers && shown.floor;
    for (const f of floating) if (f.kind === "label") f.sprite.visible = shown.labels;
  }

  // ---------- Playback ----------
  // A step follows the pipe of its relation; a reply runs back along the
  // call's pipe. While a scenario plays, its pipes turn accent and the rest
  // of the system fades.
  const tokenGeo = new SphereGeometry(0.22, 24, 12);
  const tokens = new Group();
  upper.add(tokens);
  function stepPath(step: TimelineStep): Vector3[] {
    const forwardPipe = pipesById.get(`${step.from}>${step.to}`);
    if (forwardPipe) return forwardPipe.route.map(v3);
    const backPipe = pipesById.get(`${step.to}>${step.from}`);
    if (backPipe) return backPipe.route.map(v3).reverse();
    const [a, b] = [boxes.get(step.from), boxes.get(step.to)];
    return a && b ? [v3(a.center), v3(b.center)] : [];
  }
  const pipeOf = (step: TimelineStep): Pipe | undefined => pipesById.get(`${step.from}>${step.to}`) ?? pipesById.get(`${step.to}>${step.from}`);
  let highlighted = "";
  function drawTokens() {
    tokens.clear();
    const active = timeline !== undefined && (playing || t > 0);
    // Highlight: rebuild pipes in the scenario's colors when that changes.
    const key = active && timeline ? timeline.scenario : "";
    if (key !== highlighted) {
      highlighted = key;
      const ids = new Set<string>();
      const els = new Set<string>();
      if (active && timeline) {
        for (const s of timeline.steps) {
          const p = pipeOf(s);
          if (p) ids.add(p.id);
          els.add(s.from);
          els.add(s.to);
        }
      }
      buildPipes(ids);
      scenarioFocus = active ? { els, edges: ids } : null;
      applyFocus();
    }
    caption.textContent = "";
    if (!active || !timeline) return;
    const slot = Math.floor(t);
    const labels: string[] = [];
    for (const step of timeline.steps) {
      if (step.index !== slot) continue;
      const path = stepPath(step);
      const f = easeInOut(Math.min(1, t - slot));
      if (path.length >= 2) {
        const reply = step.arrow === "reply";
        const m = new Mesh(tokenGeo, reply ? themes.neutral.face : themes.accent.face);
        m.add(new Mesh(tokenGeo, themes.accent.hull));
        m.position.copy(pointAt(path, f * lengthOf(path)));
        tokens.add(m);
      }
      labels.push(`${step.from} → ${step.to}${step.label ? `: ${step.label}` : ""}`);
    }
    caption.textContent = labels.length ? `${slot + 1}/${slots(timeline)} ${labels.join(" · ")}` : "";
  }
  function syncControls() {
    scrub.max = String(slots(timeline));
    scrub.value = String(t);
    playButton.textContent = playing ? "Pause" : t >= slots(timeline) && t > 0 ? "Replay" : "Play";
    if (timeline) select.value = timeline.scenario;
    stage.dataset.playback = `${timeline?.scenario ?? ""}@${t.toFixed(2)}`;
  }
  function setTime(next: number) {
    t = Math.min(slots(timeline), Math.max(0, next));
    drawTokens();
    syncControls();
  }
  playButton.addEventListener("click", () => {
    if (!playing && t >= slots(timeline)) t = 0;
    playing = !playing;
    drawTokens();
    syncControls();
  });
  back.addEventListener("click", () => handle.step(-1));
  forward.addEventListener("click", () => handle.step(1));
  scrub.addEventListener("input", () => {
    playing = false;
    setTime(Number(scrub.value));
  });
  select.addEventListener("change", () => {
    timeline = timelines.find((tl) => tl.scenario === select.value);
    playing = false;
    setTime(0);
  });

  // ---------- View controls ----------
  let tween: { t0: number; p0: Vector3; q0: Vector3; z0: number } | null = null;
  isoButton.addEventListener("click", () => {
    tween = { t0: performance.now(), p0: camera.position.clone(), q0: controls.target.clone(), z0: camera.zoom };
  });
  explode.addEventListener("input", () => setGap(Number(explode.value)));
  controls.addEventListener("start", () => {
    framed = true;
  });

  // ---------- Picking ----------
  const raycaster = new Raycaster();
  raycaster.params.Line = { threshold: 0.05 };
  const pointer = new Vector2();
  const ray = (e: PointerEvent | MouseEvent) => {
    const r = canvas.getBoundingClientRect();
    pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    raycaster.setFromCamera(pointer, camera);
  };
  /** The focusable thing under the pointer: an element (or its card), a
   * pipe, or a trunk. */
  const pick = (e: PointerEvent | MouseEvent): Focus => {
    ray(e);
    const targets = [...(upper.visible ? [...elementGroups.values(), ...pipeGroups.values(), ...trunkGroups] : []), ...(floor.visible ? pickable.filter((o) => o.parent === floor) : [])];
    for (const hit of raycaster.intersectObjects(targets, true)) {
      if (!hit.object.visible) continue;
      let o: Object3D | null = hit.object;
      while (o && !focusKey(o) && !o.userData.id) o = o.parent;
      if (!o) continue;
      return focusKey(o) ?? { el: o.userData.id as string };
    }
    return null;
  };
  /** Whether a press starts on the drawing: geometry, a label, or anywhere
   * inside the system boundary. Presses there rotate; elsewhere they pan. */
  const onModel = (e: PointerEvent) => {
    ray(e);
    if (upper.visible) {
      const [a, b] = [v3(tower.boundary.min), v3(tower.boundary.max)];
      a.y += upper.position.y;
      b.y += upper.position.y;
      if (raycaster.ray.intersectsBox(new Bounds(a, b))) return true;
    }
    return raycaster.intersectObjects([floor, upper, leaders], true).some((x) => x.object.visible && (x.object instanceof Mesh || x.object instanceof Sprite));
  };
  const same = (a: Focus, b: Focus) => (a?.el ?? a?.edge ?? null) === (b?.el ?? b?.edge ?? null);
  const describe = (f: Focus) => {
    if (f?.edge) {
      const p = pipesById.get(f.edge);
      return p ? { k: p.async ? "async relation" : "relation", b: `${p.from} → ${p.to}`, sub: null as string | null, id: null } : null;
    }
    const b = f?.el ? boxes.get(f.el) : undefined;
    return b ? { k: b.kind.replace("_", " "), b: b.label ?? b.id, sub: b.sublabel, id: b.id } : null;
  };
  const onMove = (e: PointerEvent) => {
    if (e.buttons) return;
    const f = pick(e);
    canvas.style.cursor = f ? "pointer" : "";
    if (!same(f, hovered)) {
      hovered = f;
      applyFocus();
    }
    const d = describe(f);
    if (!d) {
      hoverCard.hidden = true;
      return;
    }
    hoverCard.replaceChildren(h("span", "k", d.k), h("b", undefined, d.b));
    if (d.sub) hoverCard.append(h("div", undefined, d.sub));
    for (const m of d.id ? (markers.get(d.id) ?? []) : []) hoverCard.append(h("div", `diag ${m.severity}`, `${m.code}: ${m.message}`));
    const r = stage.getBoundingClientRect();
    hoverCard.style.left = `${e.clientX - r.left + 12}px`;
    hoverCard.style.top = `${e.clientY - r.top + 12}px`;
    hoverCard.hidden = false;
  };
  let downAt: { x: number; y: number } | null = null;
  // Choose the drag mode at press time; capture runs before OrbitControls.
  const onDownCapture = (e: PointerEvent) => {
    const on = onModel(e);
    controls.mouseButtons.LEFT = on ? MOUSE.ROTATE : MOUSE.PAN;
    controls.touches.ONE = on ? TOUCH.ROTATE : TOUCH.PAN;
    downAt = { x: e.clientX, y: e.clientY };
  };
  const onClick = (e: MouseEvent) => {
    // Ignore the click that ends a drag.
    if (downAt && Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) > 4) return;
    const f = pick(e);
    pinned = f && !same(f, pinned) ? f : null;
    applyFocus();
    if (pinned?.el) {
      const anchor = scene.anchors.find((a) => a.node === pinned?.el);
      opts.onSelect(pinned.el, anchor?.span);
    }
  };
  stage.addEventListener("pointerdown", onDownCapture, { capture: true });
  canvas.addEventListener("pointermove", onMove);
  canvas.addEventListener("click", onClick);
  canvas.addEventListener("pointerleave", () => {
    hoverCard.hidden = true;
    hovered = null;
    applyFocus();
  });

  // ---------- Resize and render loop ----------
  const resize = () => {
    const r = stage.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;
    renderer.setSize(r.width, r.height, false);
    const aspect = r.width / r.height;
    const half = Math.max(bounds.y, bounds.x / aspect);
    camera.left = -half * aspect;
    camera.right = half * aspect;
    camera.top = half;
    camera.bottom = -half;
    camera.updateProjectionMatrix();
  };
  const observer = new ResizeObserver(resize);
  observer.observe(stage);
  resize();

  let last = performance.now();
  let raf = 0;
  const frameLoop = (now: number) => {
    const dt = Math.min(0.25, (now - last) / 1000);
    last = now;
    if (tween) {
      const k = Math.min((now - tween.t0) / 700, 1);
      const e = k * k * (3 - 2 * k);
      camera.position.lerpVectors(tween.p0, centre.clone().addScaledVector(iso, 200), e);
      controls.target.lerpVectors(tween.q0, centre, e);
      camera.zoom = tween.z0 + (1 - tween.z0) * e;
      camera.updateProjectionMatrix();
      if (k === 1) tween = null;
    }
    if (playing) {
      const end = slots(timeline);
      setTime(t + dt / SLOT);
      if (t >= end) {
        playing = false;
        syncControls();
      }
    }
    controls.update();
    const unit = (camera.top - camera.bottom) / camera.zoom / Math.max(1, canvas.clientHeight);
    outline.value = 1.2 * unit;
    pipeOutline.value = 0.7 * unit;
    const toCamera = camera.position.clone().sub(controls.target).normalize().multiplyScalar(LIFT);
    for (const f of floating) f.sprite.position.copy(f.anchor).add(toCamera);
    renderer.render(world, camera);
    raf = requestAnimationFrame(frameLoop);
  };

  stage.dataset.containers = String(tower.containers.length);
  stage.dataset.outside = String(tower.outside.length);
  stage.dataset.infra = String(tower.infra.length);
  stage.dataset.scenarios = String(timelines.length);
  stage.dataset.pipes = String(tower.pipes.length);
  applyVisibility();
  if (restore) {
    if (framed) {
      camera.position.set(...restore.position);
      controls.target.set(...restore.target);
      camera.zoom = restore.zoom;
      camera.updateProjectionMatrix();
      controls.update();
    }
    t = Math.min(restore.t, slots(timeline));
    if (gap !== level) setGap(gap);
  }
  drawTokens();
  syncControls();
  raf = requestAnimationFrame(frameLoop);

  const handle: TowerHandle = {
    state() {
      return {
        position: [camera.position.x, camera.position.y, camera.position.z],
        target: [controls.target.x, controls.target.y, controls.target.z],
        zoom: camera.zoom,
        framed,
        scenario: timeline?.scenario,
        t,
        gap,
        hidden: LAYERS.filter(([key]) => !shown[key]).map(([key]) => key),
      };
    },
    play(scenario) {
      if (scenario) timeline = timelines.find((tl) => tl.scenario === scenario) ?? timeline;
      playing = true;
      setTime(0);
    },
    step(delta) {
      playing = false;
      setTime(Math.round(t) + delta);
    },
    select(id) {
      pinned = id ? { el: id } : null;
      applyFocus();
    },
    setMarkers(next) {
      markers = next;
      drawMarkers();
      applyVisibility();
    },
    destroy() {
      cancelAnimationFrame(raf);
      observer.disconnect();
      controls.dispose();
      stage.removeEventListener("pointerdown", onDownCapture, { capture: true });
      world.traverse((o) => {
        if (o instanceof Mesh || o instanceof Line || o instanceof LineSegments) o.geometry.dispose();
      });
      renderer.dispose();
      canvas.remove();
      panel.remove();
      hoverCard.remove();
      for (const key of ["containers", "outside", "infra", "scenarios", "playback", "pipes"]) delete stage.dataset[key];
    },
  };
  return handle;
}
