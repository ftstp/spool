<script lang="ts">
  // The player bar's volume: the speaker mutes (and brings back), the slider sets it. Over
  // everything the app plays; kept between launches.
  import { t } from "../i18n/index.svelte";
  import { vol, setVolume, toggleMute } from "../volume.svelte";
</script>

<div class="vk">
  <button class="spk" onclick={toggleMute} aria-label={vol.v ? t("song.player.mute") : t("song.player.unmute")} title={vol.v ? t("song.player.mute") : t("song.player.unmute")}>
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 9h4l5-4v14l-5-4H4z" fill="currentColor" stroke="none" />
      {#if vol.v === 0}<path d="M16 9l5 6M21 9l-5 6" />{:else}<path d="M16.5 8.5a5 5 0 0 1 0 7" />{#if vol.v > 0.5}<path d="M19 6a8.5 8.5 0 0 1 0 12" />{/if}{/if}</svg>
  </button>
  <input type="range" min="0" max="1" step="0.01" value={vol.v} oninput={(e) => setVolume(+e.currentTarget.value)} aria-label={t("song.player.volume")} title={t("song.player.volumeTip", { n: Math.round(vol.v * 100) })} style:--p={`${vol.v * 100}%`} />
</div>

<style>
  .vk { display: inline-flex; align-items: center; gap: 4px; flex: none; }
  .spk { border: 0; background: none; color: var(--muted); cursor: pointer; padding: 4px; display: grid; place-items: center; border-radius: 6px; }
  .spk:hover { color: var(--ink); background: var(--panel2); }
  input { width: 72px; height: 4px; appearance: none; -webkit-appearance: none; border-radius: 2px; cursor: pointer;
    background: linear-gradient(to right, var(--accent) var(--p), var(--line) var(--p)); }
  input::-webkit-slider-thumb { -webkit-appearance: none; width: 11px; height: 11px; border-radius: 50%; background: var(--ink); border: 0; }
</style>
