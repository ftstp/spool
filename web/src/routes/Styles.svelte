<script lang="ts">
  // "Refine your styles": nothing is asked at the start. The renders are heard; where Spool is
  // unsure of a song it asks for a confirmation in one click, a few at a time (a series of
  // five, then only if the user wants more). The user is also asked, once each: whether a word of their folders that names songs sounding alike is a
  // style of theirs, and where a style their folders name sounds like another one, whether
  // that one is theirs under another name. Their own style words and names are kept here too.
  import * as api from "../lib/api";
  import type { StyleReview } from "../lib/api/bindings/StyleReview";
  import type { ReviewGroup } from "../lib/api/bindings/ReviewGroup";
  import type { StyleCandidate } from "../lib/api/bindings/StyleCandidate";
  import { app } from "../lib/state.svelte";
  import { t, fmtNum } from "../lib/i18n/index.svelte";
  import Skeleton from "../lib/components/Skeleton.svelte";
  import StyleConfirm from "../lib/components/StyleConfirm.svelte";

  let r = $state<StyleReview | null>(null);
  let error = $state("");
  let busy = $state(false);
  let word = $state("");
  // the confirmations answered on this visit: a series of five, then only on request
  const SERIES = 5;
  let answered = $state(0);
  // per word asked about: the name the user gives it, and whether what Spool hears goes with it
  let named = $state<Record<string, string>>({});
  let linked = $state<Record<string, boolean>>({});
  // a style name as Spool compares them: no case, no accent, punctuation as spaces
  const key = (s: string) =>
    s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[^a-z0-9&]+/g, " ").trim();
  const known = $derived(new Set((r?.known ?? []).map(key)));
  const nameOf = (c: StyleCandidate) => named[c.word] ?? c.word;
  // what the name typed is to Spool: the style it hears in these songs, one it knows, a new one
  function hint(c: StyleCandidate): string {
    const k = key(nameOf(c));
    if (!k) return "";
    if (c.heard && (k === key(c.heard) || c.near[0] === k)) return t("styles.isHeard");
    if (known.has(k)) return t("styles.isKnown");
    return t("styles.isNew");
  }
  // names to offer: the style heard and its genre, then the known styles the name typed is close to
  function offers(c: StyleCandidate): string[] {
    const k = key(nameOf(c));
    const out = [...c.near];
    if (k.length >= 3 && r)
      for (const s of r.known) {
        if (out.length >= 6) break;
        // a word of the style starts like the name typed ("orch", "deep"), not a part inside one ("orc" in horrorcore)
        if (!out.includes(s) && s.split(" ").some((w) => w.startsWith(k) || (w.length >= 4 && k.split(" ").includes(w)))) out.push(s);
      }
    return out.filter((s) => s !== k);
  }
  $effect(() => {
    void app.version;
    api.styleReview().then((x) => (r = x)).catch((e) => (error = String(e)));
  });
  async function act(p: Promise<StyleReview>, said?: string) {
    busy = true;
    try {
      r = await p;
      if (said) app.say(said);
    } catch (e) {
      app.say(String(e), { error: true });
    }
    busy = false;
  }
  const keep = (g: ReviewGroup) => act(api.styleKeep(g.heard, g.mine), t("styles.kept", { heard: g.heard, mine: g.mine }));
  const dismiss = (g: ReviewGroup) => act(api.styleDismiss(g.mine, g.heard));
  function yes(c: StyleCandidate) {
    const name = nameOf(c).trim() || c.word;
    const said = key(name) === key(c.word) ? t("styles.wordAdded", { w: c.word }) : t("styles.wordNamed", { w: c.word, name });
    act(api.styleWordYes(c.word, name, linked[c.word] && c.heard ? c.heard : null), said);
  }
  const nameFor = (w: string) => r?.names.find((x) => x[0].toLowerCase() === w.toLowerCase())?.[1];
  const no = (c: StyleCandidate) => act(api.styleWordNo(c.word));
  function addWord(e: SubmitEvent) {
    e.preventDefault();
    const w = word.trim();
    if (!r || !w) return;
    word = "";
    act(api.setStyleVocab([...r.words, w], r.links, r.names), t("styles.wordAdded", { w }));
  }
  const dropWord = (w: string) => r && act(api.setStyleVocab(r.words.filter((x) => x !== w), r.links, r.names.filter((x) => x[0] !== w)));
  const dropLink = (h: string) => r && act(api.setStyleVocab(r.words, r.links.filter((x) => x[0] !== h), r.names));
  // the heard style's name as the model writes it ("brass band" is stored in lower case)
  const title = (s: string) => s.replace(/\b\p{L}/gu, (c) => c.toUpperCase());
</script>

<div class="page">
  <a class="small back" href="#/settings">← {t("app.nav.settings")}</a>
  <h1>{t("styles.title")}</h1>
  <p class="muted lede">{t("styles.lede")}</p>
  {#if error}<p class="bad">{error}</p>
  {:else if !r}<Skeleton kind="page" />
  {:else}
    {#if r.questions.length || answered}
      <section class="card">
        <h2>{t("styles.q.head")}</h2>
        {#if r.questions.length && answered < SERIES}
          {@const q = r.questions[0]}
          <p class="small muted">{t("styles.q.hint", { n: fmtNum(Number(q.helps)) })}</p>
          {#key q.id}
            <StyleConfirm sid={Number(q.id)} title={q.title ?? ""} temoin={q.temoin} options={q.options} known={r.known} done={(a) => { r = a.review; answered++; }} />
          {/key}
          <p class="small faint">{t("styles.q.progress", { k: answered + 1, n: Math.min(SERIES, answered + r.questions.length) })}</p>
        {:else if r.questions.length}
          <p class="small muted">{t("styles.q.pause")}</p>
          <div class="row"><button class="btn sm" onclick={() => (answered = 0)}>{t("styles.q.more")}</button></div>
        {:else}
          <p class="small muted">{t("styles.q.none")}</p>
        {/if}
      </section>
    {/if}

    {#if r.candidates.length}
      <section class="card">
        <h2>{t("styles.candidates")}</h2>
        <p class="small muted">{t("styles.candidatesHint")}</p>
        {#each r.candidates as c (c.word)}
          <div class="group">
            <p>
              <b>{c.heard ? t("styles.candidateHead", { word: c.word, n: fmtNum(c.n), heard: c.heard }) : t("styles.candidateHeadNamed", { word: c.word, n: fmtNum(c.n) })}</b>
              <span class="small faint">{c.genre}</span>
            </p>
            <div class="songs small">
              {#each c.songs as s (s.id)}<a href="#/song/{s.id}">{s.title || t("song.sound.untitled")}</a>{#if c.heard}<span class="faint"> {Math.round(s.act * 100)} %</span>{/if}<span class="sep"></span>{/each}
              {#if c.n > c.songs.length}<span class="faint">{t("styles.more", { n: c.n - c.songs.length })}</span>{/if}
            </div>
            <div class="naming">
              <label class="small muted" for="name-{c.word}">{t("styles.nameAs")}</label>
              <input id="name-{c.word}" list="known-styles" value={nameOf(c)} oninput={(e) => (named[c.word] = e.currentTarget.value)} autocomplete="off" spellcheck="false" />
              <span class="small faint">{hint(c)}</span>
            </div>
            {#if offers(c).length}
              <div class="offers small">
                <span class="faint">{t("styles.offers")}</span>
                {#each offers(c) as o (o)}<button class="chip-btn" onclick={() => (named[c.word] = o)}>{o}</button>{/each}
              </div>
            {/if}
            {#if c.heard && c.others > 0 && key(nameOf(c)) !== key(c.heard)}
              <label class="small also"><input type="checkbox" checked={!!linked[c.word]} onchange={(e) => (linked[c.word] = e.currentTarget.checked)} /> {t("styles.alsoHeard", { heard: c.heard, name: nameOf(c) || c.word, n: fmtNum(c.others) })}</label>
            {/if}
            <div class="row">
              <button class="btn primary" disabled={busy || !nameOf(c).trim()} onclick={() => yes(c)} title={t("styles.yesTipNamed", { word: c.word })}>{t("styles.yes", { word: nameOf(c).trim() || c.word })}</button>
              <button class="btn" disabled={busy} onclick={() => no(c)} title={t("styles.noTip")}>{t("styles.no")}</button>
            </div>
          </div>
        {/each}
        <datalist id="known-styles">{#each r.known as k (k)}<option value={k}></option>{/each}</datalist>
      </section>
    {/if}

    <section class="card">
      <h2>{t("styles.disagree")}</h2>
      {#if !r.heard}
        <p class="small muted">{t("styles.notHeard")}</p>
      {:else}
        <p class="small faint">{t("styles.heardCount", { n: fmtNum(r.heard), total: fmtNum(r.renders) })}</p>
        {#if !r.groups.length}
          <p class="small muted">{t("styles.agree")}</p>
        {/if}
        {#each r.groups as g (g.mine + "|" + g.heard)}
          <div class="group">
            <p><b>{t("styles.groupHead", { n: fmtNum(g.n), mine: g.mine, heard: g.heard })}</b> <span class="small faint">{g.genre}</span></p>
            <div class="songs small">
              {#each g.songs as s (s.id)}<a href="#/song/{s.id}">{s.title || t("song.sound.untitled")}</a> <span class="faint">{Math.round(s.act * 100)} %</span>{/each}
              {#if g.n > g.songs.length}<span class="faint">{t("styles.more", { n: g.n - g.songs.length })}</span>{/if}
            </div>
            <div class="row">
              <button class="btn primary" disabled={busy} onclick={() => keep(g)} title={t("styles.keepTip", { heard: g.heard, mine: g.mine })}>{t("styles.keep", { heard: g.heard, mine: g.mine })}</button>
              <button class="btn" disabled={busy} onclick={() => dismiss(g)} title={t("styles.dismissTip")}>{t("styles.dismiss")}</button>
            </div>
          </div>
        {/each}
      {/if}
    </section>

    <section class="card">
      <h2>{t("styles.words")}</h2>
      <p class="small muted">{t("styles.wordsHint")}</p>
      <div class="chips">
        {#each r.words as w (w)}<span class="chip">{w}{#if nameFor(w)}<span class="faint"> → </span>{nameFor(w)}{/if}<button aria-label={t("styles.remove", { w })} title={t("styles.remove", { w })} disabled={busy} onclick={() => dropWord(w)}>×</button></span>{/each}
        <form onsubmit={addWord}><input bind:value={word} placeholder={t("styles.addPh")} aria-label={t("styles.add")} /><button class="btn sm" disabled={busy || !word.trim()}>{t("styles.add")}</button></form>
      </div>
    </section>

    {#if r.links.length}
      <section class="card">
        <h2>{t("styles.links")}</h2>
        <p class="small muted">{t("styles.linksHint")}</p>
        <ul class="links">
          {#each r.links as [h, m] (h)}<li><span>{title(h)}</span><span class="faint">→</span><b>{m}</b><button class="btn sm" disabled={busy} onclick={() => dropLink(h)}>{t("styles.unlink")}</button></li>{/each}
        </ul>
      </section>
    {/if}
  {/if}
</div>

<style>
  .page { padding: 20px 24px 40px; max-width: 980px; display: grid; grid-template-columns: minmax(0, 1fr); gap: 14px; }
  .page > :global(*) { min-width: 0; }
  .back { color: var(--muted); }
  h1 { margin: 0; }
  .lede { margin: 0; max-width: 70ch; }
  h2 { margin: 0 0 10px; font-size: 15px; }
  .group { padding: 12px 0; border-top: 1px solid var(--line); display: grid; gap: 6px; }
  .group p { margin: 0; }
  .songs { display: flex; flex-wrap: wrap; gap: 2px 6px; color: var(--muted); }
  .songs a { color: var(--ink); }
  .songs .faint + a::before, .songs .sep + a::before { content: "·"; color: var(--faint); margin-right: 6px; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; }
  .naming { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .naming input { width: 16em; }
  .offers { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .chip-btn { border: 1px solid var(--line); background: none; border-radius: 999px; padding: 1px 9px; font: inherit; color: var(--ink); cursor: pointer; }
  .chip-btn:hover { background: var(--panel); }
  .also { display: flex; gap: 6px; align-items: center; color: var(--muted); }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin-top: 8px; }
  .chip { display: inline-flex; align-items: center; gap: 4px; padding: 2px 4px 2px 10px; border: 1px solid var(--line); border-radius: 999px; font-size: 12.5px; }
  .chip button { border: 0; background: none; color: var(--faint); cursor: pointer; font: inherit; padding: 0 4px; border-radius: 50%; }
  .chip button:hover { color: var(--ink); background: var(--panel); }
  form { display: inline-flex; gap: 6px; }
  form input { width: 14em; }
  .links { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .links li { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
</style>
