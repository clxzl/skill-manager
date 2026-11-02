import { getCurrentWindow } from "@tauri-apps/api/window";

export type Theme = "system" | "light" | "dark";

const KEY = "theme";
const darkMedia = () => window.matchMedia("(prefers-color-scheme: dark)");
const resolve = (theme: Theme): "light" | "dark" =>
  theme === "system" ? (darkMedia().matches ? "dark" : "light") : theme;

export const getTheme = (): Theme => {
  const value = localStorage.getItem(KEY);
  return value === "light" || value === "dark" ? value : "system";
};

let current: Theme = "system";

export function applyTheme(theme: Theme) {
  current = theme;
  document.documentElement.dataset.theme = resolve(theme);
  localStorage.setItem(KEY, theme);
  getCurrentWindow()
    .setTheme(theme === "system" ? null : theme)
    .catch(() => {});
}

darkMedia().addEventListener("change", () => {
  if (current === "system") {
    document.documentElement.dataset.theme = resolve("system");
  }
});
