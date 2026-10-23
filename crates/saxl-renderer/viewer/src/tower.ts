// The tower: a three.js view of the deployment floor, containers on pillars,
// the context shell, data as color, and scenarios played back as a thread
// (PLAN §12.4, §12.5). Geometry comes from the scene; this module draws it
// and runs playback.

import {
  AmbientLight,
  BoxGeometry,
  BufferGeometry,
  CanvasTexture,
  CatmullRomCurve3,
  Color,
  CylinderGeometry,
  DirectionalLight,
  DoubleSide,
  EdgesGeometry,
  Group,
  Line,
  LineBasicMaterial,
  LineLoop,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  type Object3D,
  PerspectiveCamera,
  Quaternion,
  Raycaster,
  Scene as ThreeScene,
  SphereGeometry,
  Sprite,
  SpriteMaterial,
  SRGBColorSpace,
  TubeGeometry,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Box3 } from "./generated/Box3";
import type { NodeKind } from "./generated/NodeKind";
import type { Point3 } from "./generated/Point3";
import type { Scene } from "./generated/Scene";
import type { Span } from "./generated/Span";
import type { Timeline } from "./generated/Timeline";
import type { TimelineStep } from "./generated/TimelineStep";
import type { Marker } from "./markers";

/** Seconds per playback slot. */
const SLOT = 0.95;
/** "Time upward" raises each slot's thread by this much. */
const RISE = 0.6;
const THREAD_RADIUS = 0.035;

export interface TowerHandle {
  state(): TowerState;
  play(scenario?: string): void;
  /** Outlines a box, or clears the outline. */
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
  framed: boolean;
  scenario?: string;
  t: number;
  rise: boolean;
  hidden: string[];
}

type Layer = "shell" | "containers" | "pillars" | "floor" | "data";
const LAYERS: [Layer, string][] = [
  ["shell", "Context shell"],
  ["containers", "Containers"],
  ["pillars", "Pillars"],
  ["floor", "Deployment floor"],
  ["data", "Data"],
];

interface Palette {
  service: Color;
  store: Color;
  event: Color;
  request: Color;
  structure: Color;
  fg: string;
  bg: string;
}

function palette(el: HTMLElement): Palette {
  const css = getComputedStyle(el);
  const get = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  return {
    service: new Color(get("--saxl-service", "#5b57d1")),
    store: new Color(get("--saxl-store", "#0f8a6c")),
    event: new Color(get("--saxl-event", "#d4552b")),
    request: new Color(get("--saxl-request", "#c98407")),
    structure: new Color(get("--saxl-structure", "#7d889a")),
    fg: get("--saxl-fg", "#151a22"),
    bg: get("--saxl-bg", "#f8f9fb"),
  };
}

function kindColor(p: Palette, kind: NodeKind): Color {
  switch (kind) {
    case "service":
      return p.service;
    case "store":
      return p.store;
    case "queue":
      return p.event;
    default:
      return p.structure;
  }
}

const v3 = (p: Point3) => new Vector3(p.x, p.y, p.z);

function easeInOut(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
}

/** A text sprite drawn on a canvas. `height` is in world units. */
function textSprite(text: string, color: string, height: number, background?: string): Sprite {
  const font = 600;
  const px = 48;
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  const fontSpec = `${font} ${px}px ui-monospace, Menlo, Consolas, monospace`;
  let width = text.length * px * 0.62 + 32;
  if (ctx) {
    ctx.font = fontSpec;
    width = ctx.measureText(text).width + 32;
  }
  canvas.width = Math.ceil(width);
  canvas.height = px + 24;
  if (ctx) {
    if (background) {
      ctx.fillStyle = background;
      ctx.beginPath();
      ctx.roundRect(0, 0, canvas.width, canvas.height, 14);
      ctx.fill();
    }
    ctx.font = fontSpec;
    ctx.fillStyle = color;
    ctx.textBaseline = "middle";
    ctx.fillText(text, 16, canvas.height / 2 + 2);
  }
  const texture = new CanvasTexture(canvas);
  texture.colorSpace = SRGBColorSpace;
  const sprite = new Sprite(new SpriteMaterial({ map: texture, depthWrite: false, transparent: true }));
  sprite.scale.set((height * canvas.width) / canvas.height, height, 1);
  return sprite;
}

/** A store is a vertical cylinder and a queue a pipe along x; anything else
 * (or anything when `shaped` is false, such as floor tiles) is a box. */
function shapeGeometry(kind: NodeKind, size: Point3, shaped: boolean): BufferGeometry {
  if (shaped && (kind === "store" || kind === "queue")) {
    const g = new CylinderGeometry(0.5, 0.5, 1, 48);
    if (kind === "queue") g.rotateZ(Math.PI / 2);
    g.scale(size.x, size.y, size.z);
    return g;
  }
  return new BoxGeometry(size.x, size.y, size.z);
}

/** Outline edges only where faces meet sharply, so a cylinder shows its rims
 * and not every side facet. */
const EDGE_ANGLE = 30;

function boxMesh(b: Box3, color: Color, opacity: number, edgeOpacity: number, shaped = false): Group {
  const g = new Group();
  const geometry = shapeGeometry(b.kind, b.size, shaped);
  const mesh = new Mesh(
    geometry,
    new MeshStandardMaterial({
      color,
      transparent: opacity < 1,
      opacity,
      depthWrite: opacity >= 1,
      roughness: 0.7,
    }),
  );
  mesh.userData.id = b.id;
  g.add(mesh);
  if (edgeOpacity > 0) {
    const edges = new LineSegments(
      new EdgesGeometry(geometry, EDGE_ANGLE),
      new LineBasicMaterial({ color, transparent: true, opacity: edgeOpacity }),
    );
    g.add(edges);
  }
  g.position.copy(v3(b.center));
  return g;
}

function cylinderBetween(a: Vector3, b: Vector3, radius: number, color: Color, opacity: number): Mesh {
  const dir = new Vector3().subVectors(b, a);
  const mesh = new Mesh(
    new CylinderGeometry(radius, radius, dir.length(), 8),
    new MeshStandardMaterial({ color, transparent: true, opacity }),
  );
  mesh.position.copy(a).addScaledVector(dir, 0.5);
  mesh.quaternion.copy(new Quaternion().setFromUnitVectors(new Vector3(0, 1, 0), dir.clone().normalize()));
  return mesh;
}

/** The point at fraction `f` of a polyline's length, and the prefix up to it. */
function prefix(points: Vector3[], f: number): Vector3[] {
  const lengths = points.slice(1).map((p, i) => p.distanceTo(points[i]));
  const total = lengths.reduce((a, b) => a + b, 0);
  let left = total * f;
  const out = [points[0].clone()];
  for (let i = 0; i < lengths.length; i++) {
    if (left >= lengths[i]) {
      out.push(points[i + 1].clone());
      left -= lengths[i];
    } else {
      out.push(points[i].clone().lerp(points[i + 1], lengths[i] === 0 ? 1 : left / lengths[i]));
      break;
    }
  }
  return out;
}

function h<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mountTower(stage: HTMLElement, scene: Scene, opts: TowerOptions): TowerHandle {
  const tower = scene.tower;
  if (!tower) throw new Error("scene has no tower");
  const colors = palette(stage);
  const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const renderer = new WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.outputColorSpace = SRGBColorSpace;
  const canvas = renderer.domElement;
  canvas.className = "saxl-canvas";
  stage.append(canvas);

  const world = new ThreeScene();
  world.add(new AmbientLight(0xffffff, 1.6));
  const sun = new DirectionalLight(0xffffff, 1.4);
  sun.position.set(6, 12, 8);
  world.add(sun);

  // Everything fits in a sphere around the ring of outside elements.
  const radius = tower.shell.radius + 3.5;
  const camera = new PerspectiveCamera(42, 1, 0.1, radius * 30);
  const viewDir = new Vector3(0.62, 0.48, 0.72).normalize();
  let framed = false;
  const frame = () => {
    const vfov = (camera.fov * Math.PI) / 180;
    const hfov = 2 * Math.atan(Math.tan(vfov / 2) * camera.aspect);
    const dist = radius / Math.sin(Math.min(vfov, hfov) / 2);
    camera.position.copy(viewDir).multiplyScalar(dist).add(new Vector3(0, 2, 0));
  };
  const controls = new OrbitControls(camera, canvas);
  controls.target.set(0, 2, 0);
  controls.enableDamping = true;
  controls.autoRotate = !reducedMotion;
  controls.autoRotateSpeed = 0.6;
  controls.addEventListener("start", () => {
    controls.autoRotate = false;
    framed = true;
  });
  controls.update();

  const layers: Record<Layer, Group> = {
    shell: new Group(),
    containers: new Group(),
    pillars: new Group(),
    floor: new Group(),
    data: new Group(),
  };
  for (const g of Object.values(layers)) world.add(g);
  // Lines that join two layers live in their own groups: each is shown only
  // while both layers it connects are shown (see applyVisibility).
  const contextLinks = new Group();
  const flows = new Group();
  world.add(contextLinks, flows);
  const shown: Record<Layer, boolean> = {
    shell: true,
    containers: true,
    pillars: true,
    floor: true,
    data: true,
  };
  for (const key of opts.restore?.hidden ?? []) if (key in shown) shown[key as Layer] = false;
  const pickable: Object3D[] = [];
  const boxes = new Map<string, Box3>();
  /** Which layer each pickable box belongs to. */
  const layerOf = new Map<string, Layer>();
  const boxShown = (id: string) => {
    const layer = layerOf.get(id);
    return layer === undefined || shown[layer];
  };

  // Floor: slabs, pods, infrastructure, captions.
  for (const s of tower.slabs) {
    const floor = s.kind === "floor";
    layers.floor.add(boxMesh(s, kindColor(colors, s.kind), floor ? 0.1 : 0.22, floor ? 0.35 : 0.5));
  }
  for (const p of tower.pods) layers.floor.add(boxMesh(p, kindColor(colors, p.kind), 0.85, 0));
  for (const l of tower.labels) {
    const sprite = textSprite(l.text, colors.fg, 0.32);
    sprite.position.copy(v3(l.position));
    sprite.center.set(0, 0);
    layers.floor.add(sprite);
  }
  for (const b of tower.infra) {
    const g = boxMesh(b, colors.structure, 0.35, 0.8);
    pickable.push(g.children[0]);
    boxes.set(b.id, b);
    layerOf.set(b.id, "floor");
    layers.floor.add(g);
    const label = textSprite(b.label ?? b.id, colors.fg, 0.3);
    label.position.set(b.center.x, b.center.y + 0.45, b.center.z);
    layers.floor.add(label);
  }

  // Containers and pillars.
  for (const b of tower.containers) {
    const g = boxMesh(b, kindColor(colors, b.kind), 0.28, 0.9, true);
    pickable.push(g.children[0]);
    boxes.set(b.id, b);
    layerOf.set(b.id, "containers");
    layers.containers.add(g);
    const label = textSprite(b.label ?? b.id, colors.fg, 0.34);
    label.position.set(b.center.x, b.center.y + b.size.y / 2 + 0.3, b.center.z);
    layers.containers.add(label);
  }
  for (const p of tower.pillars) {
    layers.pillars.add(cylinderBetween(v3(p.from), v3(p.to), 0.03, colors.structure, 0.55));
  }

  // Context shell, outside elements, and their links.
  const shellHeight = tower.shell.y_max - tower.shell.y_min;
  const shell = new Mesh(
    new CylinderGeometry(tower.shell.radius, tower.shell.radius, shellHeight, 96, 1, true),
    new MeshBasicMaterial({ color: colors.structure, transparent: true, opacity: 0.06, side: DoubleSide, depthWrite: false }),
  );
  shell.position.y = tower.shell.y_min + shellHeight / 2;
  layers.shell.add(shell);
  for (const y of [tower.shell.y_min, tower.shell.y_max]) {
    const ring = new BufferGeometry().setFromPoints(
      Array.from({ length: 96 }, (_, i) => {
        const a = (i / 96) * Math.PI * 2;
        return new Vector3(Math.cos(a) * tower.shell.radius, y, Math.sin(a) * tower.shell.radius);
      }),
    );
    layers.shell.add(new LineLoop(ring, new LineBasicMaterial({ color: colors.structure, transparent: true, opacity: 0.45 })));
  }
  for (const b of tower.outside) {
    const g = boxMesh(b, colors.structure, 0.3, 0.9);
    pickable.push(g.children[0]);
    boxes.set(b.id, b);
    layerOf.set(b.id, "shell");
    layers.shell.add(g);
    const label = textSprite(b.label ?? b.id, colors.fg, 0.34);
    label.position.set(b.center.x, b.center.y + b.size.y / 2 + 0.3, b.center.z);
    layers.shell.add(label);
  }
  for (const l of tower.context_links) {
    const line = new BufferGeometry().setFromPoints([v3(l.from), v3(l.to)]);
    contextLinks.add(new Line(line, new LineBasicMaterial({ color: colors.structure, transparent: true, opacity: 0.5 })));
  }

  // Data: fills at rest, flows in motion.
  for (const f of tower.fills) {
    const truth = f.role === "truth";
    layers.data.add(boxMesh(f.geometry, colors.store, truth ? 0.55 : 0.22, 0, true));
  }
  const particles: { mesh: Mesh; curve: CatmullRomCurve3; offset: number }[] = [];
  for (const flow of tower.flows) {
    const points = flow.points.map(v3);
    flows.add(
      new Line(new BufferGeometry().setFromPoints(points), new LineBasicMaterial({ color: colors.event, transparent: true, opacity: 0.6 })),
    );
    const curve = new CatmullRomCurve3(points, false, "centripetal", 0);
    for (let i = 0; i < 3; i++) {
      const mesh = new Mesh(new SphereGeometry(0.07, 12, 8), new MeshBasicMaterial({ color: colors.event }));
      mesh.position.copy(curve.getPointAt(i / 3));
      particles.push({ mesh, curve, offset: i / 3 });
      flows.add(mesh);
    }
  }

  // Selection outline.
  const outline = new Group();
  world.add(outline);
  let selectedId: string | null = null;
  function selectBox(id: string | null) {
    selectedId = id;
    outline.clear();
    outline.visible = id === null || boxShown(id);
    const b = id ? boxes.get(id) : undefined;
    if (!b) return;
    const pad = 0.12;
    const padded = { x: b.size.x + pad, y: b.size.y + pad, z: b.size.z + pad };
    const geometry = new EdgesGeometry(shapeGeometry(b.kind, padded, true), EDGE_ANGLE);
    const lines = new LineSegments(geometry, new LineBasicMaterial({ color: colors.request }));
    lines.position.copy(v3(b.center));
    outline.add(lines);
  }

  // Diagnostic markers.
  const markerGroup = new Group();
  world.add(markerGroup);
  let markers = opts.markers;
  function drawMarkers() {
    markerGroup.clear();
    for (const [id, list] of markers) {
      const b = boxes.get(id);
      if (!b || list.length === 0 || !boxShown(id)) continue;
      const error = list.some((m) => m.severity === "error");
      const sprite = textSprite("!", error ? "#fff" : "#151a22", 0.4, error ? "#d4552b" : "#f5b742");
      sprite.position.set(b.center.x + b.size.x / 2, b.center.y + b.size.y / 2 + 0.25, b.center.z);
      markerGroup.add(sprite);
    }
  }
  drawMarkers();

  // Controls panel.
  const panel = h("div", "saxl-tower-panel");
  const layerBox = h("fieldset");
  layerBox.append(h("legend", undefined, "Layers"));
  for (const [key, text] of LAYERS) {
    const label = h("label");
    const box = h("input");
    box.type = "checkbox";
    box.checked = shown[key];
    box.addEventListener("change", () => {
      shown[key] = box.checked;
      applyVisibility();
    });
    label.append(box, ` ${text}`);
    layerBox.append(label);
  }
  panel.append(layerBox);

  const timelines = scene.timelines;
  const restore = opts.restore;
  let timeline: Timeline | undefined =
    timelines.find((tl) => tl.scenario === restore?.scenario) ?? timelines[0];
  let t = 0;
  let playing = false;
  let rise = restore?.rise ?? false;
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
  const riseLabel = h("label");
  const riseBox = h("input");
  riseBox.type = "checkbox";
  riseLabel.append(riseBox, " Time upward");
  const row = h("div", "saxl-row");
  row.append(back, playButton, forward);
  play.append(select, row, scrub, riseLabel);
  if (timelines.length > 0) panel.append(play);
  stage.append(panel);

  const thread = new Group();
  world.add(thread);

  function syncControls() {
    scrub.max = String(slots(timeline));
    scrub.value = String(t);
    playButton.textContent = playing ? "Pause" : t >= slots(timeline) && t > 0 ? "Replay" : "Play";
    if (timeline) select.value = timeline.scenario;
    stage.dataset.playback = `${timeline?.scenario ?? ""}@${t.toFixed(2)}`;
  }

  function waypoints(step: TimelineStep): Vector3[] {
    const ids = [step.from, ...step.via, step.to];
    const points: Vector3[] = [];
    for (const id of ids) {
      const b = boxes.get(id);
      if (b) points.push(v3(b.center));
    }
    if (rise) for (const p of points) p.y += RISE * step.index;
    return points;
  }

  function drawThread() {
    for (const child of [...thread.children]) {
      thread.remove(child);
      if (child instanceof Mesh) child.geometry.dispose();
    }
    if (!timeline) return;
    for (const step of timeline.steps) {
      const f = Math.min(1, Math.max(0, t - step.index));
      if (f <= 0) continue;
      // A step is a connection too: hide it if anything it passes through is hidden.
      if (![step.from, ...step.via, step.to].every(boxShown)) continue;
      const points = waypoints(step);
      if (points.length < 2) continue;
      const part = prefix(points, easeInOut(f));
      const color = step.arrow === "async" ? colors.event : colors.request;
      const opacity = step.arrow === "reply" ? 0.45 : 0.95;
      // A reply that returns along the same path is offset slightly so it is visible.
      if (step.arrow === "reply") for (const p of part) p.y -= 0.08;
      if (part.length >= 2 && part[0].distanceTo(part[part.length - 1]) > 1e-3) {
        const curve = new CatmullRomCurve3(part, false, "catmullrom", 0);
        const tube = new Mesh(
          new TubeGeometry(curve, Math.max(8, part.length * 12), THREAD_RADIUS, 6, false),
          new MeshBasicMaterial({ color, transparent: opacity < 1, opacity }),
        );
        thread.add(tube);
      }
      if (f < 1) {
        const head = new Mesh(new SphereGeometry(0.12, 16, 12), new MeshBasicMaterial({ color }));
        head.position.copy(part[part.length - 1]);
        thread.add(head);
      }
    }
  }

  /** Shows each layer per its toggle, and each connection only while every
   * layer it joins is shown. */
  function applyVisibility() {
    for (const [key] of LAYERS) layers[key].visible = shown[key];
    layers.pillars.visible = shown.pillars && shown.floor && shown.containers;
    contextLinks.visible = shown.shell && shown.containers;
    flows.visible = shown.data && shown.containers;
    outline.visible = selectedId === null || boxShown(selectedId);
    drawMarkers();
    drawThread();
  }

  function setTime(next: number) {
    t = Math.min(slots(timeline), Math.max(0, next));
    drawThread();
    syncControls();
  }

  playButton.addEventListener("click", () => {
    if (!playing && t >= slots(timeline)) t = 0;
    playing = !playing;
    controls.autoRotate = false;
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
  riseBox.addEventListener("change", () => {
    rise = riseBox.checked;
    drawThread();
  });

  // Picking: hover cards and selection.
  const raycaster = new Raycaster();
  const pointer = new Vector2();
  const card = h("div", "saxl-card");
  card.hidden = true;
  stage.append(card);
  const pick = (e: PointerEvent | MouseEvent): string | undefined => {
    const r = canvas.getBoundingClientRect();
    pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    raycaster.setFromCamera(pointer, camera);
    const visible = pickable.filter((o) => o.parent?.parent?.visible !== false);
    return raycaster.intersectObjects(visible, false)[0]?.object.userData.id as string | undefined;
  };
  const onMove = (e: PointerEvent) => {
    const id = pick(e);
    const b = id ? boxes.get(id) : undefined;
    if (!b) {
      card.hidden = true;
      canvas.style.cursor = "";
      return;
    }
    canvas.style.cursor = "pointer";
    card.replaceChildren(h("span", "k", b.kind.replace("_", " ")), h("b", undefined, b.label ?? b.id));
    if (b.sublabel) card.append(h("div", undefined, b.sublabel));
    for (const m of markers.get(b.id) ?? []) card.append(h("div", `diag ${m.severity}`, `${m.code}: ${m.message}`));
    const r = stage.getBoundingClientRect();
    card.style.left = `${e.clientX - r.left + 12}px`;
    card.style.top = `${e.clientY - r.top + 12}px`;
    card.hidden = false;
  };
  let downAt: { x: number; y: number } | null = null;
  const onDown = (e: PointerEvent) => {
    downAt = { x: e.clientX, y: e.clientY };
  };
  const onClick = (e: MouseEvent) => {
    // Ignore the click that ends an orbit drag.
    if (downAt && Math.hypot(e.clientX - downAt.x, e.clientY - downAt.y) > 4) return;
    const id = pick(e);
    if (!id) return;
    const anchor = scene.anchors.find((a) => a.node === id);
    selectBox(id);
    opts.onSelect(id, anchor?.span);
  };
  canvas.addEventListener("pointermove", onMove);
  canvas.addEventListener("pointerdown", onDown);
  canvas.addEventListener("click", onClick);
  canvas.addEventListener("pointerleave", () => {
    card.hidden = true;
  });

  // Resize and render loop.
  const resize = () => {
    const r = stage.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;
    renderer.setSize(r.width, r.height, false);
    camera.aspect = r.width / r.height;
    camera.updateProjectionMatrix();
    // Re-frame on resize until the user moves the camera.
    if (!framed) frame();
  };
  const observer = new ResizeObserver(resize);
  observer.observe(stage);
  resize();

  let last = performance.now();
  let raf = 0;
  const loop = (now: number) => {
    const dt = Math.min(0.25, (now - last) / 1000);
    last = now;
    if (playing) {
      const end = slots(timeline);
      setTime(t + dt / SLOT);
      if (t >= end) {
        playing = false;
        syncControls();
      }
    }
    if (!reducedMotion) {
      const phase = now / 4000;
      for (const p of particles) p.mesh.position.copy(p.curve.getPointAt((phase + p.offset) % 1));
    }
    controls.update();
    renderer.render(world, camera);
    raf = requestAnimationFrame(loop);
  };
  raf = requestAnimationFrame(loop);

  stage.dataset.containers = String(tower.containers.length);
  stage.dataset.outside = String(tower.outside.length);
  stage.dataset.infra = String(tower.infra.length);
  stage.dataset.scenarios = String(timelines.length);
  applyVisibility();
  if (restore) {
    framed = restore.framed;
    if (framed) {
      camera.position.set(...restore.position);
      controls.target.set(...restore.target);
      controls.autoRotate = false;
      controls.update();
    }
    riseBox.checked = rise;
    t = Math.min(restore.t, slots(timeline));
    drawThread();
  }
  syncControls();

  const handle: TowerHandle = {
    state() {
      return {
        position: [camera.position.x, camera.position.y, camera.position.z],
        target: [controls.target.x, controls.target.y, controls.target.z],
        framed,
        scenario: timeline?.scenario,
        t,
        rise,
        hidden: LAYERS.filter(([key]) => !shown[key]).map(([key]) => key),
      };
    },
    play(scenario) {
      if (scenario) timeline = timelines.find((tl) => tl.scenario === scenario) ?? timeline;
      controls.autoRotate = false;
      playing = true;
      setTime(0);
    },
    step(delta) {
      playing = false;
      controls.autoRotate = false;
      setTime(Math.round(t) + delta);
    },
    select: selectBox,
    setMarkers(next) {
      markers = next;
      drawMarkers();
    },
    destroy() {
      cancelAnimationFrame(raf);
      observer.disconnect();
      controls.dispose();
      world.traverse((o) => {
        if (o instanceof Mesh || o instanceof Line || o instanceof LineSegments) o.geometry.dispose();
      });
      renderer.dispose();
      canvas.remove();
      panel.remove();
      card.remove();
      for (const key of ["containers", "outside", "infra", "scenarios", "playback"]) delete stage.dataset[key];
    },
  };
  return handle;
}
