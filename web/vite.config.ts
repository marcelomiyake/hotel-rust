import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api/hotels": { target: "http://127.0.0.1:8081", rewrite: (path) => path.replace(/^\/api\/hotels/, "/v1/hotels") },
      "/api/rates": { target: "http://127.0.0.1:8082", rewrite: (path) => path.replace(/^\/api\/rates/, "/v1/rates") },
      "/api/reservations": { target: "http://127.0.0.1:8083", rewrite: (path) => path.replace(/^\/api\/reservations\/availability$/, "/v1/availability").replace(/^\/api\/reservations/, "/v1/reservations") },
      "/api/admin": { target: "http://127.0.0.1:8085", rewrite: (path) => path.replace(/^\/api\/admin/, "/v1/admin") }
    }
  },
  preview: { port: 4173, strictPort: true },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    restoreMocks: true,
    clearMocks: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      reportsDirectory: "./coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/main.tsx", "src/test/**", "src/**/*.d.ts"],
      thresholds: { lines: 80, functions: 80, branches: 75, statements: 80 }
    }
  }
});
