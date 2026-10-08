import { defineConfig } from "vite";
import preact from "@preact/preset-vite";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

// Página de quem assiste à transmissão. Vai embutida no executável (rust-embed lê
// src-tauri/viewer-dist) e é servida pelo servidor HLS do próprio CapturaX.
export default defineConfig({
    root: resolve(__dirname, "viewer"),
    base: "/",
    publicDir: false,
    plugins: [preact(), tailwindcss()],
    build: {
        outDir: resolve(__dirname, "src-tauri/viewer-dist"),
        emptyOutDir: true,
        // Sem source maps: não expõe o código-fonte para quem assiste.
        sourcemap: false,
        assetsInlineLimit: 0,
    },
});
