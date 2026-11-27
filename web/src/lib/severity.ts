// How much a plugin's state blocks a project: 0 opens, 1 the app repairs it (settings kept),
// 2 loads but its settings are lost, 3 not installed. ONE table, severity.json, read by the
// Rust side too (plugins::status_severity): change it there, both follow.
import table from "./severity.json";

const T = table as Record<string, number>;

/** An unknown state counts as missing, as on the Rust side. */
export const statusSeverity = (status: string | null | undefined) => T[status ?? "ok"] ?? 3;
