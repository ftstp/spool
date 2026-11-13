// Renders a few frames to out/stills/ for review: node stills.mjs 100 400 ...
import { bundle } from "@remotion/bundler";
import { renderStill, selectComposition } from "@remotion/renderer";
import path from "node:path";
const serveUrl = await bundle({ entryPoint: path.resolve("src/index.ts") });
const composition = await selectComposition({ serveUrl, id: "SpoolLoop", inputProps: { music: false } });
for (const f of process.argv.slice(2).map(Number)) {
  await renderStill({ serveUrl, composition, frame: f, output: `out/stills/${String(f).padStart(4, "0")}.jpg`, imageFormat: "jpeg", jpegQuality: 85, inputProps: { music: false } });
  console.log("still", f);
}
