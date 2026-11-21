<script lang="ts">
  // In the player bar, beside the song's chords: what the MIDI keyboard plays now (a chord's
  // name, or the notes when they make no chord), so an idea tried over a song can be read
  // against the chord heard. Shown while a keyboard is plugged in and chosen in Settings; the
  // last chord stays a moment after the keys are let go.
  import { keyboard } from "../keyboard.svelte";
  import { chordSource } from "../chordNow.svelte";
  import { chordOf, chordName, spelling } from "../theory";
  import { i18n, t } from "../i18n/index.svelte";

  const bound = $derived(!!keyboard.input && keyboard.inputs.some((i) => i.id === keyboard.input));
  const LAT: Record<string, string> = { C: "Do", D: "Ré", E: "Mi", F: "Fa", G: "Sol", A: "La", B: "Si" };
  const latin = $derived.by(() => {
    try {
      const m = localStorage.getItem("spool.chart.mode");
      return m === "latin" || (m !== "us" && i18n.lang === "fr");
    } catch {
      return i18n.lang === "fr";
    }
  });
  // written in the song's key when one is known (its flats or sharps), else the usual names
  const names = $derived(chordSource.v ? spelling(chordSource.v.tonic, chordSource.v.minor) : spelling(0, false));
  const pretty = (n: string) => {
    const s = n.replace("#", "♯").replace(/^(.)b/, "$1♭");
    return latin ? LAT[s[0]] + s.slice(1) : s;
  };
  function label(notes: number[]): string {
    if (!notes.length) return "";
    const pcs = [...new Set(notes.map((n) => n % 12))];
    const low = Math.min(...notes) % 12;
    if (pcs.length === 1) return pretty(names[low]);
    const w = new Array(12).fill(0);
    for (const n of notes) w[n % 12] += 1;
    const c = chordOf(w, low);
    if (!c) return [...notes].sort((a, b) => a - b).map((n) => pretty(names[n % 12])).join(" ");
    const head = chordName(c, names);
    const root = pretty(head.slice(0, head.length - c.q.length));
    const q = c.q;
    return root + (latin && q && !/^\d/.test(q) ? " " : "") + q + (low !== c.root % 12 ? "/" + pretty(names[low]) : "");
  }
  // the chord held, kept 1.5 s after the keys are let go
  let shown = $state("");
  let fading = $state(false);
  $effect(() => {
    const now = label(keyboard.down);
    if (now) {
      shown = now;
      fading = false;
      return;
    }
    fading = true;
    const id = setTimeout(() => (shown = ""), 1500);
    return () => clearTimeout(id);
  });
</script>

{#if bound}
  <span class="kc" class:fading title={t("song.player.keyChordTip")}>
    <span class="k">{t("song.player.keyChord")}</span><b>{shown || "-"}</b>
  </span>
{/if}

<style>
  .kc { flex: none; width: 128px; display: inline-flex; align-items: baseline; gap: 6px; white-space: nowrap; overflow: hidden; padding-left: 10px; border-left: 1px solid var(--line); }
  .k { font-size: 11px; color: var(--faint); }
  b { min-width: 0; overflow: hidden; text-overflow: ellipsis; font-size: 15px; font-weight: 700; color: var(--accent); transition: opacity .3s; }
  .fading b { opacity: .45; }
</style>
