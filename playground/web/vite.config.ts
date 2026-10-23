import { defineConfig } from "vite";

export default defineConfig({
  // Relative asset paths, so the build works under a GitHub Pages subpath.
  base: "./",
  worker: { format: "es" },
  // three.js and CodeMirror make one ~1 MB chunk (~290 KB gzipped); expected.
  build: { target: "es2022", chunkSizeWarningLimit: 1200 },
});
