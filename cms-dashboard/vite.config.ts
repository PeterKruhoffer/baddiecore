import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  base: "/cms/",
  plugins: [solid()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:5088",
        changeOrigin: true
      }
    }
  },
  build: {
    outDir: "../wwwroot/cms",
    emptyOutDir: false
  }
});
