import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import wasm from "vite-plugin-wasm";

const pkgDir = path.resolve(__dirname, "../pkg");

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
      "rs-wasm-tv-app": pkgDir,
    },
  },
  server: {
    fs: { allow: [path.resolve(__dirname, "..")] },
  },
  optimizeDeps: {
    exclude: ["rs-wasm-tv-app"],
  },
});
