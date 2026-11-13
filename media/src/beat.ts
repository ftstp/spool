// The beat grid of the soundtrack: "Strung Out" by Lowpoly (SNG-911), 94 bpm.
// Grid measured on the track with librosa (beat_track, then a least-squares fit:
// first beat 0.2014 s, period 0.63829 s, 94.001 bpm, max deviation 23 ms); downbeats on
// beats 0, 4, 8... (they match the catalogue's section starts at 46.12, 74.19, 115.05 s).
// The video starts on the downbeat of bar 29 of the track (74.245 s), so beat n of the
// video is n * BEAT_S seconds in.
export const FPS = 30;
export const BEAT_S = 0.6382895;
export const BEAT = BEAT_S * FPS; // 19.15 frames
/** frame of beat n (n may be fractional) */
export const b = (n: number) => Math.round(n * BEAT);

// scene starts, in beats (4 beats = 1 bar). The music is bars 29-53 of the track, unbroken
// (no splice): 25 bars = 100 beats. The outro starts on beat 92 and its last line lands on
// beat 96, the downbeat of the track's final chord.
export const START = {
  intro: 0, catalogue: 8, songs: 16, song: 22, versions: 30, calendar: 36, mood: 42, repair: 50,
  cleanup: 56, release: 66, theme: 74, lang: 84, outro: 92,
};
// 100 beats of music, then 1.2 s of the final chord ringing out
export const TOTAL = Math.round(100 * BEAT + 1.2 * FPS);
export const OUTRO_LEN = TOTAL - b(START.outro);
