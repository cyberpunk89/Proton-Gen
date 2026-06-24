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

export function applyTheme(id: string) {
  const theme = THEMES.some((t) => t.id === id) ? id : DEFAULT_THEME;
  document.documentElement.dataset.theme = theme;
}
