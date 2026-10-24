// Viewer styles. Colors are CSS custom properties with light and dark values;
// node, edge, and group classes match the Rust SVG output.

export const CSS = `
.saxl-viewer{--saxl-axo-lit:#ffffff;--saxl-axo-shade:#e1e1e7;--saxl-axo-plate-shade:#d8d8de;--saxl-axo-accent-lit:#fcefea;--saxl-axo-accent-shade:#edbfaf;--saxl-axo-ink:#2e3040;--saxl-axo-thin:#9a9cab;--saxl-axo-accent:#b5472b;--saxl-axo-muted:#8a8d9e;--saxl-axo-zone:#f7f7f9;--saxl-axo-tier:#dcdde4;--saxl-axo-opening:#dcdce2;--saxl-axo-card:#ffffff;--saxl-axo-card-line:#d4d4da;--saxl-service:#5b57d1;--saxl-store:#0f8a6c;--saxl-event:#d4552b;--saxl-request:#c98407;--saxl-structure:#7d889a;--saxl-fg:#151a22;--saxl-muted:#586274;--saxl-bg:#f8f9fb;--saxl-panel:#ffffff;--saxl-line:#dde1e7}
@media (prefers-color-scheme:dark){.saxl-viewer{--saxl-axo-lit:#3a414d;--saxl-axo-shade:#252a33;--saxl-axo-plate-shade:#1e232b;--saxl-axo-accent-lit:#6b3a2d;--saxl-axo-accent-shade:#4a271e;--saxl-axo-ink:#c9cfda;--saxl-axo-thin:#6d7789;--saxl-axo-accent:#f07a52;--saxl-axo-muted:#97a1b2;--saxl-axo-zone:#1c222b;--saxl-axo-tier:#2c3440;--saxl-axo-opening:#151a21;--saxl-axo-card:#1c222b;--saxl-axo-card-line:#3a4250;--saxl-service:#8f8cf0;--saxl-store:#3fc79f;--saxl-event:#f07a52;--saxl-request:#f5b742;--saxl-structure:#6d7789;--saxl-fg:#e6e9ef;--saxl-muted:#97a1b2;--saxl-bg:#151a21;--saxl-panel:#1c222b;--saxl-line:#2c3440}}
.saxl-viewer{position:relative;display:flex;flex-direction:column;height:100%;min-height:320px;background:var(--saxl-bg);color:var(--saxl-fg);font:13px/1.4 ui-monospace,Menlo,Consolas,monospace;overflow:hidden}
.saxl-viewer *{box-sizing:border-box}
.saxl-bar{display:flex;flex-wrap:wrap;align-items:center;gap:4px;padding:8px 12px;border-bottom:1px solid var(--saxl-line);background:var(--saxl-panel)}
.saxl-bar button,.saxl-bar select{font:inherit;color:var(--saxl-fg);background:transparent;border:1px solid transparent;border-radius:6px;padding:4px 10px;cursor:pointer}
.saxl-bar button:hover{border-color:var(--saxl-line)}
.saxl-bar button[aria-pressed=true]{background:var(--saxl-bg);border-color:var(--saxl-line);font-weight:600}
.saxl-bar select{border-color:var(--saxl-line)}
.saxl-title{margin-left:auto;color:var(--saxl-muted)}
.saxl-diag-toggle[data-count="0"]{display:none}
.saxl-stage{position:relative;flex:1;min-height:0;cursor:grab;touch-action:none}
.saxl-stage.dragging{cursor:grabbing}
.saxl-stage.is-3d{cursor:auto}
.saxl-stage svg{display:block;width:100%;height:100%}
.saxl-card{position:absolute;pointer-events:none;max-width:280px;padding:8px 10px;border:1px solid var(--saxl-line);border-radius:8px;background:var(--saxl-panel);box-shadow:0 4px 16px rgb(0 0 0/.12)}
.saxl-card b{display:block}
.saxl-card .k{color:var(--saxl-muted);font-size:11px;text-transform:uppercase;letter-spacing:.04em}
.saxl-diags{max-height:35%;overflow:auto;border-top:1px solid var(--saxl-line);background:var(--saxl-panel);padding:6px 12px}
.saxl-diags[hidden]{display:none}
.saxl-diags li{list-style:none;padding:3px 0}
.saxl-diags ul{margin:0;padding:0}
.saxl-diags .code{font-weight:600;margin-right:8px}
.saxl-diags .error .code{color:var(--saxl-event)}
.saxl-diags .warning .code{color:var(--saxl-request)}
.saxl-viewer text{fill:var(--saxl-fg);font-size:14px}
.saxl-viewer .sub,.saxl-viewer .badge,.saxl-viewer .field,.saxl-viewer .edge-label,.saxl-viewer .group-label{font-size:12px}
.saxl-viewer .sub,.saxl-viewer .group-label,.saxl-viewer .badge,.saxl-viewer .edge-label{fill:var(--saxl-muted)}
.saxl-viewer .badge{font-weight:600}
.saxl-viewer .node{color:var(--saxl-structure);cursor:pointer}
.saxl-viewer .node-service{color:var(--saxl-service)}
.saxl-viewer .node-store{color:var(--saxl-store)}
.saxl-viewer .node-queue{color:var(--saxl-event)}
.saxl-viewer .node-api,.saxl-viewer .node-module,.saxl-viewer .node-library{color:var(--saxl-service)}
.saxl-viewer .node-hook{color:var(--saxl-request)}
.saxl-viewer .node .base{fill:var(--saxl-bg)}
.saxl-viewer .node .tint{fill:currentColor;fill-opacity:.1;stroke:currentColor;stroke-width:1.5}
.saxl-viewer .node:hover .tint{fill-opacity:.18}
.saxl-viewer .node .rim{fill:none;stroke:currentColor;stroke-width:1.5}
.saxl-viewer .node.selected .tint,.saxl-viewer .node.selected .rim{stroke-width:3}
.saxl-viewer .node-external .tint,.saxl-viewer .node-infra .tint,.saxl-viewer .node-package .tint{stroke-dasharray:5 3}
.saxl-viewer .node-system .tint,.saxl-viewer .node-library .tint{fill-opacity:.05;stroke-width:2}
.saxl-viewer .node-api .tint{stroke-width:2.5;fill-opacity:.16}
.saxl-viewer .edge{fill:none;stroke:var(--saxl-muted);stroke-width:1.5}
.saxl-viewer .edge-async{stroke:var(--saxl-event);stroke-dasharray:6 4}
.saxl-viewer .edge-reply{stroke-dasharray:2 4;opacity:.75}
.saxl-viewer .edge-impl{stroke:var(--saxl-request);stroke-dasharray:6 4}
.saxl-viewer .edge-label{paint-order:stroke;stroke:var(--saxl-bg);stroke-width:4px}
.saxl-viewer .lifeline{stroke:var(--saxl-structure);stroke-dasharray:4 4;opacity:.6}
.saxl-viewer .group rect{fill:none;stroke:var(--saxl-structure);stroke-width:1.2}
.saxl-viewer .group-cluster rect,.saxl-viewer .group-managed rect,.saxl-viewer .group-package rect{stroke-dasharray:6 3}
.saxl-viewer .group-hook rect{stroke:none;fill:var(--saxl-request);fill-opacity:.07}
.saxl-viewer .group-parallel rect{stroke:var(--saxl-request);fill:var(--saxl-request);fill-opacity:.06}
.saxl-viewer .arrow-sync,.saxl-viewer .arrow-reply{fill:var(--saxl-muted)}
.saxl-viewer .arrow-async{fill:var(--saxl-event)}
.saxl-viewer .arrow-impl{fill:var(--saxl-bg);stroke:var(--saxl-request)}
.saxl-viewer.dimmed .saxl-stage{opacity:.45}
.saxl-viewer .marker circle{fill:var(--saxl-request);stroke:var(--saxl-bg);stroke-width:2}
.saxl-viewer .marker text{fill:#151a22;font-size:12px;font-weight:700}
.saxl-viewer .marker-error circle{fill:var(--saxl-event)}
.saxl-viewer .marker-error text{fill:#fff}
.saxl-canvas{position:absolute;inset:0;width:100%;height:100%;display:block;cursor:auto}
.saxl-tower-panel{position:absolute;top:10px;left:10px;display:flex;flex-direction:column;gap:8px;max-width:220px;cursor:auto}
.saxl-tower-panel fieldset{margin:0;padding:6px 10px 8px;border:1px solid var(--saxl-line);border-radius:8px;background:var(--saxl-panel);display:flex;flex-direction:column;gap:4px}
.saxl-tower-panel legend{padding:0 4px;color:var(--saxl-muted);font-size:11px;text-transform:uppercase;letter-spacing:.04em}
.saxl-tower-panel label{display:flex;gap:6px;align-items:center;cursor:pointer}
.saxl-tower-panel button,.saxl-tower-panel select{font:inherit;color:var(--saxl-fg);background:var(--saxl-bg);border:1px solid var(--saxl-line);border-radius:6px;padding:3px 8px;cursor:pointer}
.saxl-tower-panel .saxl-row{display:flex;gap:4px}
.saxl-tower-panel .saxl-row button:nth-child(2){flex:1}
.saxl-tower-panel input[type=range]{width:100%;accent-color:var(--saxl-axo-accent)}
.saxl-tower-panel .saxl-step{color:var(--saxl-muted);font-size:11px;min-height:1.4em}
.saxl-card .diag{margin-top:4px;color:var(--saxl-request)}
.saxl-card .diag.error{color:var(--saxl-event)}
@media (max-width:560px){.saxl-tower-panel{max-width:170px;font-size:12px}}
`;
