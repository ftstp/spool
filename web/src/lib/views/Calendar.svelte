<script lang="ts">
  // Songs on their date (release, deadline), and under each day a badge per song worked on
  // that day (a project of it saved). A song
  // dragged onto a day takes that date, dragged onto the side list loses it (Ctrl+Z undoes).
  import type { SongItem } from "../api";
  import * as api from "../api";
  import { app } from "../state.svelte";
  import { f } from "../filters.svelte";
  import { hue } from "../format";
  import { t, fmtDate } from "../i18n/index.svelte";

  let { list }: { list: SongItem[] } = $props();
  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const today = iso(new Date());
  const month = $derived.by(() => {
    const n = new Date();
    return f.month ? new Date(f.month + "-01T12:00:00") : new Date(n.getFullYear(), n.getMonth(), 1, 12);
  });
  const days = $derived.by(() => {
    const start = new Date(month);
    start.setDate(1 - ((month.getDay() + 6) % 7)); // weeks start on Monday
    const n = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate() + ((month.getDay() + 6) % 7) > 35 ? 42 : 35;
    return Array.from({ length: n }, (_, i) => {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      return d;
    });
  });
  /** Dates moved on screen at once, saved behind. */
  let moved = $state<Record<number, string | null>>({});
  $effect(() => {
    void app.version;
    moved = {};
  });
  const dateOf = (s: SongItem) => (s.id in moved ? moved[s.id] : s.date);
  const dated = $derived.by(() => {
    const m: Record<string, SongItem[]> = {};
    for (const s of list) {
      const d = dateOf(s);
      if (d) (m[d] ??= []).push(s);
    }
    return m;
  });
  const work = $derived.by(() => {
    const m: Record<string, SongItem[]> = {};
    if (f.work) for (const s of list) for (const d of s.days) (m[d] ??= []).push(s);
    return m;
  });
  const undated = $derived(list.filter((s) => !dateOf(s) && s.stage !== "done"));
  const upcoming = $derived(
    list.filter((s) => (dateOf(s) ?? "") >= today).sort((a, b) => (dateOf(a) ?? "").localeCompare(dateOf(b) ?? "")).slice(0, 8),
  );
  const label = $derived(fmtDate(month, { month: "long", year: "numeric" }));
  const fday = (d: string) => fmtDate(new Date(d + "T12:00:00"), { day: "numeric", month: "short" });
  // the short weekday names, Monday first (1 January 2024 was a Monday)
  const dow = $derived(Array.from({ length: 7 }, (_, i) => fmtDate(new Date(2024, 0, 1 + i, 12), { weekday: "short" })));

  function go(n: number) {
    const now = new Date();
    const d = n ? new Date(month.getFullYear(), month.getMonth() + n, 1, 12) : new Date(now.getFullYear(), now.getMonth(), 1, 12);
    f.month = iso(d).slice(0, 7);
  }
  /** The day showing all its badges. */
  let open = $state<string | null>(null);
  let dragged = $state<number | null>(null);
  let over = $state<string | null>(null);
  async function drop(day: string | null) {
    const s = list.find((x) => x.id === dragged);
    dragged = null;
    over = null;
    if (!s || (dateOf(s) ?? null) === day) return;
    moved = { ...moved, [s.id]: day };
    await app.act(api.songEdit(s.id, { date: day }));
    if (app.toast?.error) moved = {};
  }
  const dnd = (key: string, day: string | null) => ({
    ondragover: (e: DragEvent) => {
      if (dragged != null) {
        e.preventDefault();
        over = key;
      }
    },
    ondragleave: (e: DragEvent) => {
      if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) over = null;
    },
    ondrop: (e: DragEvent) => {
      e.preventDefault();
      drop(day);
    },
  });
  const grab = (s: SongItem) => ({
    draggable: true,
    ondragstart: (e: DragEvent) => {
      dragged = s.id;
      e.dataTransfer?.setData("text/plain", String(s.id));
    },
    ondragend: () => {
      dragged = null;
      over = null;
    },
  });
</script>

<div class="wrap">
  <div class="main">
    <div class="tools">
      <button class="btn sm" onclick={() => go(-1)} aria-label={t("songs.cal.prev")}>‹</button>
      <h2 class="mt">{label}</h2>
      <button class="btn sm" onclick={() => go(1)} aria-label={t("songs.cal.next")}>›</button>
      <button class="btn sm" onclick={() => go(0)}>{t("songs.cal.today")}</button>
      <label class="small muted wk" title={t("songs.cal.workTitle")}><input type="checkbox" bind:checked={f.work} /> {t("songs.cal.work")}</label>
    </div>
    <div class="dow">{#each dow as d}<span>{d}</span>{/each}</div>
    <div class="cal">
      {#each days as d (iso(d))}
        {@const k = iso(d)}
        {@const dt = dated[k] ?? []}
        {@const w = (work[k] ?? []).filter((s) => !dt.includes(s))}
        {@const room = Math.max(0, 4 - dt.length)}
        <div class="day" class:out={d.getMonth() !== month.getMonth()} class:today={k === today} class:drop={over === k} {...dnd(k, k)}>
          <div class="dn">{d.getDate()}{#if w.length}<span class="faint" title={t("songs.cal.workedDay", { n: w.length })}>{w.length}</span>{/if}</div>
          {#each dt as s (s.id)}
            <a class="ev s-{s.stage || 'none'}" href="#/song/{s.id}" title={t("songs.cal.dated", { title: s.title })} {...grab(s)}>{s.title}</a>
          {/each}
          {#each (open === k ? w : w.slice(0, room)) as s (s.id)}
            <a class="wk-b s-{s.stage || 'none'}" href="#/song/{s.id}" title={t("songs.cal.saved", { title: s.title })} style="--h:{hue(s.title ?? '')}" {...grab(s)}><i></i><span>{s.title}</span></a>
          {/each}
          {#if w.length > room && open !== k}
            <button class="more" onclick={() => (open = k)}>{t("songs.cal.others", { n: w.length - room })}</button>
          {:else if open === k && w.length > room}
            <button class="more" onclick={() => (open = null)}>{t("songs.cal.less")}</button>
          {/if}
        </div>
      {/each}
    </div>
  </div>
  <aside class="side">
    {#if upcoming.length}
      <h3>{t("songs.cal.upcoming")}</h3>
      {#each upcoming as s (s.id)}<a class="ev s-{s.stage || 'none'} up" href="#/song/{s.id}" {...grab(s)}><span class="ell">{s.title}</span><span class="faint">{fday(dateOf(s) ?? "")}</span></a>{/each}
    {/if}
    <h3>{t("songs.cal.undated")} <span class="faint">{undated.length}</span></h3>
    <p class="small faint">{t("songs.cal.hint")}</p>
    <div class="undated" class:drop={over === "none"} {...dnd("none", null)}>
      {#each undated.slice(0, 150) as s (s.id)}
        <a class="ev s-{s.stage || 'none'}" href="#/song/{s.id}" {...grab(s)}><span class="ell">{s.title}</span><span class="faint ell">{s.artist ?? ""}</span></a>
      {/each}
      {#if undated.length > 150}<p class="small faint">{t("songs.more", { n: undated.length - 150 })}</p>{/if}
    </div>
  </aside>
</div>

<style>
  .wrap { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 300px; }
  .main { display: flex; flex-direction: column; min-height: 0; padding: 12px 18px 16px 22px; overflow: auto; }
  .tools { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
  .mt { margin: 0 6px; font-size: 17px; text-transform: capitalize; min-width: 150px; text-align: center; }
  .wk { margin-left: auto; display: inline-flex; gap: 6px; align-items: center; cursor: pointer; }
  .dow { display: grid; grid-template-columns: repeat(7, 1fr); font-size: 11.5px; color: var(--faint); padding: 0 2px 4px; }
  .cal { flex: none; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); grid-auto-rows: minmax(112px, auto); gap: 1px; background: var(--line); border: 1px solid var(--line); border-radius: var(--radius); overflow: hidden; }
  .day { background: var(--panel); padding: 5px 6px; min-height: 0; overflow: hidden; display: flex; flex-direction: column; gap: 3px; transition: background-color .15s; }
  .day.out { background: var(--panel2); }
  .day.out .dn { color: var(--faint); }
  .day.today .dn { color: var(--accent); font-weight: 700; }
  .day.drop, .undated.drop { background: var(--accent-bg); }
  .dn { display: flex; justify-content: space-between; align-items: center; font-size: 12px; color: var(--muted); }
  .dn .faint { font-size: 10.5px; font-variant-numeric: tabular-nums; }
  .wk-b { display: flex; align-items: center; gap: 5px; font-size: 11px; padding: 1px 6px 1px 4px; border-radius: 99px; color: var(--muted); background: color-mix(in srgb, var(--line) 45%, transparent); min-width: 0; cursor: grab; }
  .wk-b:hover { color: var(--ink); background: color-mix(in srgb, var(--line) 80%, transparent); }
  .wk-b i { width: 6px; height: 6px; border-radius: 50%; flex: none; background: var(--s-none); }
  .wk-b span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .wk-b.s-draft i { background: var(--s-draft); } .wk-b.s-arrangement i { background: var(--s-arrangement); } .wk-b.s-mix i { background: var(--s-mix); }
  .wk-b.s-master i { background: var(--s-master); } .wk-b.s-done i { background: var(--s-done); } .wk-b.s-pause i { background: var(--s-pause); }
  .wk-b.s-draft, .wk-b.s-arrangement, .wk-b.s-mix, .wk-b.s-master, .wk-b.s-done, .wk-b.s-pause { border-left: 0; }
  .more { border: 0; background: none; color: var(--accent); cursor: pointer; font-size: 11px; padding: 0 4px; text-align: left; }
  .ev { display: flex; justify-content: space-between; gap: 8px; font-size: 11.5px; padding: 2px 7px; border-radius: 5px; color: var(--ink); background: var(--panel2); border-left: 3px solid var(--s-none); cursor: grab; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; min-width: 0; }
  .ev:hover { background: color-mix(in srgb, var(--line) 70%, transparent); }
  .s-draft { border-left-color: var(--s-draft); } .s-arrangement { border-left-color: var(--s-arrangement); } .s-mix { border-left-color: var(--s-mix); }
  .s-master { border-left-color: var(--s-master); } .s-done { border-left-color: var(--s-done); } .s-pause { border-left-color: var(--s-pause); }
  .side { border-left: 1px solid var(--line); padding: 14px 16px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; min-height: 0; }
  h3 { font-size: 13px; margin: 6px 0 4px; }
  .side p { margin: 0 0 6px; }
  .undated { display: flex; flex-direction: column; gap: 4px; flex: 1; border-radius: 8px; padding: 2px; transition: background-color .15s; }
  .side .ev { padding: 5px 8px; font-size: 12.5px; }
  .ell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  @container (max-width: 900px) { .wrap { grid-template-columns: 1fr; } .side { border-left: 0; border-top: 1px solid var(--line); max-height: 260px; } }
</style>
