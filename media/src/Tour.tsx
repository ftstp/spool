// The whole tour: every scene starts on a beat of the soundtrack (see beat.ts) and plays FADE
// frames under the next one, which fades in on top from its beat.
import React from "react";
import { AbsoluteFill, Audio, Sequence, staticFile } from "remotion";
import { START, TOTAL, b } from "./beat";
import { Catalogue, Copy, Intro, Lang, Mood, Outro, Release, Song, Songs, Theme, Versions } from "./scenes";
import { FADE } from "./ui";

export const SCENES: [keyof typeof START, React.FC][] = [
  ["intro", Intro], ["catalogue", Catalogue], ["songs", Songs], ["song", Song], ["versions", Versions], ["mood", Mood],
  ["repair", Copy], ["release", Release], ["theme", Theme], ["lang", Lang], ["outro", Outro],
];
export { TOTAL };

export const Tour: React.FC<{ music: boolean }> = ({ music }) => (
  <AbsoluteFill style={{ background: "#f4f1ec" }}>
    {SCENES.map(([name, C], i) => {
      const from = b(START[name]);
      const next = i + 1 < SCENES.length ? b(START[SCENES[i + 1][0]]) + FADE : TOTAL;
      return (
        <Sequence key={name} name={name} from={from} durationInFrames={next - from}>
          <C />
        </Sequence>
      );
    })}
    {music && <Audio src={staticFile("music.wav")} />}
  </AbsoluteFill>
);
