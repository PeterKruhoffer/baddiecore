import solid from "@solidjs/vite-plugin";
import stylex from "@stylexjs/unplugin";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [
    stylex.vite({
      devMode: "full",
      runtimeInjection: false,
    }),
    solid(),
  ],
  server: {
    proxy: {
      "/api": "http://localhost:3000",
      "/health": "http://localhost:3000",
    },
  },
});
