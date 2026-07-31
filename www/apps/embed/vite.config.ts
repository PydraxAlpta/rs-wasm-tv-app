import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import wasm from "vite-plugin-wasm";

const pkgDir = path.resolve(__dirname, "../../../crates/tv-ui-web/pkg");
const repoRoot = path.resolve(__dirname, "../../..");
const tvWebgl = path.resolve(__dirname, "../../packages/tv-webgl/src/index.ts");

function watchPkg(): Plugin {
  return {
    name: "watch-pkg",
    configureServer(server) {
      server.watcher.add(pkgDir);
    },
  };
}

export default defineConfig({
  base: "./",
  plugins: [wasm(), watchPkg()],
  resolve: {
    alias: {
      "tv-ui-web": pkgDir,
      "tv-webgl": tvWebgl,
    },
  },
  server: {
    fs: { allow: [repoRoot] },
  },
  optimizeDeps: {
    exclude: ["tv-ui-web"],
  },
});
