import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { readFile, writeFile } from "node:fs/promises";
import { createServer } from "vite";

Object.defineProperty(globalThis, "window", {
  value: { location: { pathname: "/", search: "" } },
  configurable: true
});

const vite = await createServer({
  appType: "custom",
  logLevel: "error",
  server: { middlewareMode: true }
});

try {
  const { default: App } = await vite.ssrLoadModule("/src/App.tsx");
  const markup = renderToString(createElement(App));
  const indexPath = new URL("../dist/index.html", import.meta.url);
  let html = await readFile(indexPath, "utf8");
  const stylesheet = html.match(/<link rel="stylesheet" crossorigin href="([^"]+)"\s*\/?>/);
  if (stylesheet) {
    const cssPath = new URL(`../dist${stylesheet[1]}`, import.meta.url);
    const css = await readFile(cssPath, "utf8");
    html = html.replace(stylesheet[0], `<style>${css}</style>`);
  }
  html = html.replace('<div id="root"></div>', `<div id="root" data-prerendered-home="true">${markup}</div>`);
  await writeFile(indexPath, html);
} finally {
  await vite.close();
}
