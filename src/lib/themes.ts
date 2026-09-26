export interface ThemeDef {
  id: string;
  label: string;
}

export const THEMES: ThemeDef[] = [
  { id: "mocha", label: "Catppuccin Mocha" },
  { id: "macchiato", label: "Catppuccin Macchiato" },
  { id: "frappe", label: "Catppuccin Frappé" },
  { id: "latte", label: "Catppuccin Latte" },
  { id: "dracula", label: "Dracula" },
  { id: "nord", label: "Nord" },
  { id: "tokyo-night", label: "Tokyo Night" },
  { id: "gruvbox", label: "Gruvbox" },
  { id: "rose-pine", label: "Rosé Pine" },
  { id: "one-dark", label: "One Dark" },
];

export const DEFAULT_THEME = "mocha";

/** Follow the desktop's light/dark preference: Latte when light, Mocha when
 *  dark. Stored as this id, which older builds fall back from to the default. */
export const SYSTEM_THEME = "system";

let systemQuery: MediaQueryList | null = null;
let onSystemChange: (() => void) | null = null;

export function applyTheme(id: string) {
  const root = document.documentElement;
  if (systemQuery && onSystemChange) systemQuery.removeEventListener("change", onSystemChange);
  onSystemChange = null;

  if (id === SYSTEM_THEME) {
    // WebKitGTK reports GTK's dark-mode preference here; a desktop that only
    // sets its own colour scheme (not GTK's) will read as light.
    const query = (systemQuery = window.matchMedia("(prefers-color-scheme: dark)"));
    onSystemChange = () => (root.dataset.theme = query.matches ? "mocha" : "latte");
    onSystemChange();
    query.addEventListener("change", onSystemChange);
    return;
  }
  root.dataset.theme = THEMES.some((t) => t.id === id) ? id : DEFAULT_THEME;
}
