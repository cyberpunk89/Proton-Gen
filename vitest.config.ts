import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";

// Unit tests for the pure frontend modules (shell quoting, preset codes, the
// overlay-config round trips, markdown, fuzzy, util). The svelte plugin is
// here for `.svelte.ts` rune modules and bits-ui (fuzzy.ts imports its scorer).
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
