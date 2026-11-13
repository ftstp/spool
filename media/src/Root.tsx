import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/600.css";
import React from "react";
import { Composition, continueRender, delayRender } from "remotion";
import { Poster } from "./scenes";
import { TOTAL, Tour } from "./Tour";

// the faces are CSS-loaded: hold the first frame until they are in
const wait = delayRender("fonts");
Promise.all(["400 20px 'IBM Plex Sans'", "500 20px 'IBM Plex Sans'", "600 20px 'IBM Plex Sans'", "400 20px 'IBM Plex Mono'", "600 20px 'IBM Plex Mono'"]
  .map((f) => document.fonts.load(f))).then(() => continueRender(wait));

export const Root: React.FC = () => (
  <>
    <Composition id="SpoolTour" component={Tour} durationInFrames={TOTAL} fps={30} width={1920} height={1080} defaultProps={{ music: true }} />
    <Composition id="SpoolPoster" component={Poster} durationInFrames={1} fps={30} width={1920} height={1080} />
    <Composition id="SpoolLoop" component={Tour} durationInFrames={TOTAL} fps={30} width={1920} height={1080} defaultProps={{ music: false }} />
  </>
);
