// The last statistics received (Home and the Statistics page): shown at once when a page
// opens, while the new ones are worked out (a second or more on a large catalogue).
import type { HomeStats, Progress } from "./api";

export const lastStats: { h: HomeStats | null; p: Progress | null } = { h: null, p: null };
