import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/health": "http://127.0.0.1:3001",
      "/control": "http://127.0.0.1:3001",
      "/memory": "http://127.0.0.1:3001",
      "/sessions": "http://127.0.0.1:3001",
      "/consolidate": "http://127.0.0.1:3001",
      "/mcp": "http://127.0.0.1:3001"
    }
  }
});
