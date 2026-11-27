// The Songs page's offer to refine the styles: how many songs a question of the styles page
// is about (a word of their folders that may be a style, a style heard otherwise, a song
// Spool is unsure of), asked
// again when the base changes, and whether it was put aside for this session.
import * as api from "./api";

export const refine = $state({ n: 0, off: false });
let asked = -1;
export function askRefine(version: number) {
  if (version === asked) return;
  asked = version;
  api.styleReview().then((r) => (refine.n = [...r.candidates, ...r.groups].reduce((a, g) => a + Number(g.n), 0) + r.questions.length), () => {});
}
