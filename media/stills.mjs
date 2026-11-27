// Renders a few frames to out/stills/ for review: node stills.mjs [CompositionId] 100 400 ...
import { bundle } from "@remotion/bundler";
import { renderStill, selectComposition } from "@remotion/renderer";
import path from "node:path";
const args = process.argv.slice(2);
const id = isNaN(Number(args[0])) ? args.shift() : "SpoolLoop";
const serveUrl = await bundle({ entryPoint: path.resolve("src/index.ts") });
const inputProps = { music: false };
const composition = await selectComposition({ serveUrl, id, inputProps });
for (const f of args.map(Number)) {
  await renderStill({ serveUrl, composition, frame: f, output: `out/stills/${id}-${String(f).padStart(4, "0")}.jpg`, imageFormat: "jpeg", jpegQuality: 85, inputProps });
  console.log("still", id, f);
}
