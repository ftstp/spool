// Renders N consecutive frames of a composition as PNG into out/jit/ (for the motion check).
import { bundle } from "@remotion/bundler";
import { renderStill, selectComposition } from "@remotion/renderer";
import path from "node:path";
const [id, from, n] = [process.argv[2], Number(process.argv[3]), Number(process.argv[4] ?? 10)];
const serveUrl = await bundle({ entryPoint: path.resolve("src/index.ts") });
const inputProps = { music: false };
const composition = await selectComposition({ serveUrl, id, inputProps });
for (let f = from; f < from + n; f++)
  await renderStill({ serveUrl, composition, frame: f, output: `out/jit/${id}-${f}.png`, imageFormat: "png", inputProps });
console.log("done", id, from, n);
