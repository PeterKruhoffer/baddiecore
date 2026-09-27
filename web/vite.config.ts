import { defineConfig } from "vite";
import solid from "@solidjs/vite-plugin";
export default defineConfig({plugins:[solid()],server:{proxy:{"/api":"http://localhost:3000","/health":"http://localhost:3000"}}});
