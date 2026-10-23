// Pan (drag), zoom (wheel and pinch), and fit-to-view for an SVG group.

export interface PanZoom {
  fit(): void;
  destroy(): void;
}

interface View {
  x: number;
  y: number;
  k: number;
}

const MIN_K = 0.1;
const MAX_K = 4;

export function panZoom(stage: HTMLElement, target: SVGGElement, width: number, height: number): PanZoom {
  let view: View = { x: 0, y: 0, k: 1 };
  const pointers = new Map<number, { x: number; y: number }>();
  let pinch: { dist: number; k: number } | null = null;

  const apply = () => target.setAttribute("transform", `translate(${view.x} ${view.y}) scale(${view.k})`);

  const zoomAt = (px: number, py: number, k: number) => {
    k = Math.min(MAX_K, Math.max(MIN_K, k));
    view = { x: px - ((px - view.x) / view.k) * k, y: py - ((py - view.y) / view.k) * k, k };
    apply();
  };

  const local = (e: { clientX: number; clientY: number }) => {
    const box = stage.getBoundingClientRect();
    return { x: e.clientX - box.left, y: e.clientY - box.top };
  };

  const fit = () => {
    const box = stage.getBoundingClientRect();
    if (box.width === 0 || box.height === 0 || width === 0 || height === 0) return;
    const k = Math.min(box.width / width, box.height / height, 1.5);
    view = { x: (box.width - width * k) / 2, y: (box.height - height * k) / 2, k };
    apply();
  };

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const p = local(e);
    zoomAt(p.x, p.y, view.k * Math.exp(-e.deltaY * 0.0015));
  };

  const onDown = (e: PointerEvent) => {
    if ((e.target as Element).closest?.(".node")) return;
    stage.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, local(e));
    stage.classList.add("dragging");
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      pinch = { dist: Math.hypot(a.x - b.x, a.y - b.y), k: view.k };
    }
  };

  const onMove = (e: PointerEvent) => {
    const prev = pointers.get(e.pointerId);
    if (!prev) return;
    const p = local(e);
    pointers.set(e.pointerId, p);
    if (pointers.size === 2 && pinch) {
      const [a, b] = [...pointers.values()];
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      zoomAt((a.x + b.x) / 2, (a.y + b.y) / 2, (pinch.k * dist) / (pinch.dist || 1));
    } else if (pointers.size === 1) {
      view = { ...view, x: view.x + p.x - prev.x, y: view.y + p.y - prev.y };
      apply();
    }
  };

  const onUp = (e: PointerEvent) => {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = null;
    if (pointers.size === 0) stage.classList.remove("dragging");
  };

  stage.addEventListener("wheel", onWheel, { passive: false });
  stage.addEventListener("pointerdown", onDown);
  stage.addEventListener("pointermove", onMove);
  stage.addEventListener("pointerup", onUp);
  stage.addEventListener("pointercancel", onUp);
  stage.addEventListener("dblclick", fit);
  const resize = new ResizeObserver(fit);
  resize.observe(stage);

  return {
    fit,
    destroy() {
      stage.removeEventListener("wheel", onWheel);
      stage.removeEventListener("pointerdown", onDown);
      stage.removeEventListener("pointermove", onMove);
      stage.removeEventListener("pointerup", onUp);
      stage.removeEventListener("pointercancel", onUp);
      stage.removeEventListener("dblclick", fit);
      resize.disconnect();
    },
  };
}
