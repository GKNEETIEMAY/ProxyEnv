import type { AppTheme } from "../types";

export const palettes = ["warm", "blue", "teal", "neutral"] as const;
export type Palette = typeof palettes[number];
export const paletteStorageKey = "routespan.ui.palette.v1";
// A visual startup cache only. The backend's existing theme remains authoritative.
export const appearanceStorageKey = "routespan.ui.appearance.v1";
type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

export function normalizePalette(value: unknown): Palette {
  return palettes.includes(value as Palette) ? value as Palette : "warm";
}
function read(key: string, storage?: PreferenceStorage): string | null {
  try { return (storage ?? localStorage).getItem(key); } catch { return null; }
}
function save(key: string, value: string, storage?: PreferenceStorage): void {
  try { (storage ?? localStorage).setItem(key, value); } catch { /* Keep this process usable when storage is unavailable. */ }
}
export function readPalette(storage?: PreferenceStorage): Palette {
  return normalizePalette(read(paletteStorageKey, storage));
}
export function savePalette(value: Palette, storage?: PreferenceStorage): void {
  save(paletteStorageKey, normalizePalette(value), storage);
}
export function readAppearance(storage?: PreferenceStorage): AppTheme {
  const value = read(appearanceStorageKey, storage);
  return value === "light" || value === "dark" ? value : "system";
}
export function saveAppearance(value: AppTheme, storage?: PreferenceStorage): void {
  save(appearanceStorageKey, value === "light" || value === "dark" ? value : "system", storage);
}
export function applyTheme(root: HTMLElement, mode: AppTheme, palette: Palette, systemDark: boolean): void {
  root.dataset.palette = normalizePalette(palette);
  root.dataset.theme = mode === "system" ? systemDark ? "dark" : "light" : mode;
}
