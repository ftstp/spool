// A song sheet's transport: the bar kept in sight at the top of the sheet plays the render or
// the project. The project's playback lives in the arrangement (MiniSession), which hands its
// controls here while it is on the sheet; without it, the bar plays the render alone.

export type SheetTransport = {
  sid: number;
  /** What the play button plays: the reference render or the project's tracks. */
  src: "project" | "temoin";
  setSrc: (s: "project" | "temoin") => void;
  playPause: () => void;
  /** Playing (or loading the project's sounds). */
  sounding: boolean;
  loading: boolean;
  /** Where the project is, 0..1 (null when it does not play and has no start set). */
  at: number | null;
  /** Moves the project's start (and what plays) to a point, 0..1. */
  seek: (f: number) => void;
  /** Its length, in seconds at the song's tempo. */
  secs: number;
  /** "200 bars", for the sections bar. */
  summary: string;
  /** The song's key from its notes ("Gm"), for the sections bar. */
  key: string | null;
  /** The render it plays, and what the MIDI player calls the project's playback: the player
   *  bar at the bottom shows the same Render / Project switch while one of them plays. */
  temoin: string | null;
  project: string;
};

// one per song sheet open (several can be, in tabs): each sheet reads its own, the player bar
// and the Space key the one whose render or project is what plays
export const sheetTransport = $state<{ by: Record<number, SheetTransport> }>({ by: {} });
