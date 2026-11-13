import { Config } from "@remotion/cli/config";
Config.setEntryPoint("src/index.ts");
// PNG frames: no JPEG blocks shimmering on slow zooms
Config.setVideoImageFormat("png");
Config.setCodec("h264");
Config.setPixelFormat("yuv420p");
