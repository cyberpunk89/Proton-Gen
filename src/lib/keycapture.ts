/**
 * Recorded keystroke -> the key string each overlay tool expects, and back to
 * something readable. Pure data, no Svelte — `KeyCapture.svelte` is the UI.
 *
 * Three tools, three vocabularies:
 *   - vkBasalt `toggleKey`      — one X11 keysym name (`Home`, `Prior`, `F12`)
 *   - MangoHud `toggle_*`       — keysym names joined by `+` (`Shift_R+F12`)
 *   - OptiScaler `ShortcutKey`  — a Windows virtual-key code (`0x24`), because
 *                                 it runs inside the game under Wine
 *
 * Everything is keyed on `KeyboardEvent.code` (the physical key), not `.key`:
 * `.key` is layout- and modifier-dependent (Shift+1 is "!"), while every tool
 * here wants the key itself. Left/right modifiers stay distinct for the same
 * reason — MangoHud's default is `Shift_R+F12`, not "either Shift".
 */

export type KeyFormat = "keysym" | "combo" | "vk";

interface KeyDef {
  code: string;
  keysym: string;
  /** Windows virtual-key code, when the key has one worth offering. */
  vk?: number;
  label: string;
  modifier?: boolean;
}

const DEFS: KeyDef[] = [
  { code: "Home", keysym: "Home", vk: 0x24, label: "Home" },
  { code: "End", keysym: "End", vk: 0x23, label: "End" },
  { code: "Insert", keysym: "Insert", vk: 0x2d, label: "Insert" },
  { code: "Delete", keysym: "Delete", vk: 0x2e, label: "Delete" },
  { code: "PageUp", keysym: "Prior", vk: 0x21, label: "Page Up" },
  { code: "PageDown", keysym: "Next", vk: 0x22, label: "Page Down" },
  { code: "Pause", keysym: "Pause", vk: 0x13, label: "Pause" },
  { code: "ScrollLock", keysym: "Scroll_Lock", vk: 0x91, label: "Scroll Lock" },
  { code: "PrintScreen", keysym: "Print", vk: 0x2c, label: "Print Screen" },
  { code: "Backspace", keysym: "BackSpace", vk: 0x08, label: "Backspace" },
  { code: "Tab", keysym: "Tab", vk: 0x09, label: "Tab" },
  { code: "Enter", keysym: "Return", vk: 0x0d, label: "Enter" },
  { code: "Space", keysym: "space", vk: 0x20, label: "Space" },
  { code: "CapsLock", keysym: "Caps_Lock", vk: 0x14, label: "Caps Lock" },
  { code: "ArrowLeft", keysym: "Left", vk: 0x25, label: "←" },
  { code: "ArrowUp", keysym: "Up", vk: 0x26, label: "↑" },
  { code: "ArrowRight", keysym: "Right", vk: 0x27, label: "→" },
  { code: "ArrowDown", keysym: "Down", vk: 0x28, label: "↓" },
  { code: "Minus", keysym: "minus", vk: 0xbd, label: "-" },
  { code: "Equal", keysym: "equal", vk: 0xbb, label: "=" },
  { code: "BracketLeft", keysym: "bracketleft", vk: 0xdb, label: "[" },
  { code: "BracketRight", keysym: "bracketright", vk: 0xdd, label: "]" },
  { code: "Backslash", keysym: "backslash", vk: 0xdc, label: "\\" },
  { code: "Semicolon", keysym: "semicolon", vk: 0xba, label: ";" },
  { code: "Quote", keysym: "apostrophe", vk: 0xde, label: "'" },
  { code: "Backquote", keysym: "grave", vk: 0xc0, label: "`" },
  { code: "Comma", keysym: "comma", vk: 0xbc, label: "," },
  { code: "Period", keysym: "period", vk: 0xbe, label: "." },
  { code: "Slash", keysym: "slash", vk: 0xbf, label: "/" },
  { code: "NumpadDivide", keysym: "KP_Divide", vk: 0x6f, label: "Numpad /" },
  { code: "NumpadMultiply", keysym: "KP_Multiply", vk: 0x6a, label: "Numpad *" },
  { code: "NumpadSubtract", keysym: "KP_Subtract", vk: 0x6d, label: "Numpad -" },
  { code: "NumpadAdd", keysym: "KP_Add", vk: 0x6b, label: "Numpad +" },
  { code: "NumpadEnter", keysym: "KP_Enter", label: "Numpad Enter" },
  { code: "NumpadDecimal", keysym: "KP_Decimal", vk: 0x6e, label: "Numpad ." },
  { code: "ShiftLeft", keysym: "Shift_L", vk: 0xa0, label: "L Shift", modifier: true },
  { code: "ShiftRight", keysym: "Shift_R", vk: 0xa1, label: "R Shift", modifier: true },
  { code: "ControlLeft", keysym: "Control_L", vk: 0xa2, label: "L Ctrl", modifier: true },
  { code: "ControlRight", keysym: "Control_R", vk: 0xa3, label: "R Ctrl", modifier: true },
  { code: "AltLeft", keysym: "Alt_L", vk: 0xa4, label: "L Alt", modifier: true },
  { code: "AltRight", keysym: "Alt_R", vk: 0xa5, label: "R Alt", modifier: true },
  { code: "MetaLeft", keysym: "Super_L", label: "L Super", modifier: true },
  { code: "MetaRight", keysym: "Super_R", label: "R Super", modifier: true },
];

for (let i = 1; i <= 24; i++) DEFS.push({ code: `F${i}`, keysym: `F${i}`, vk: 0x6f + i, label: `F${i}` });
for (let i = 0; i <= 9; i++) {
  DEFS.push({ code: `Digit${i}`, keysym: String(i), vk: 0x30 + i, label: String(i) });
  DEFS.push({ code: `Numpad${i}`, keysym: `KP_${i}`, vk: 0x60 + i, label: `Numpad ${i}` });
}
for (let i = 0; i < 26; i++) {
  const ch = String.fromCharCode(65 + i);
  // X11 names letter keysyms in lowercase; `A` is the shifted symbol.
  DEFS.push({ code: `Key${ch}`, keysym: ch.toLowerCase(), vk: 0x41 + i, label: ch });
}

const BY_CODE = new Map(DEFS.map((d) => [d.code, d]));
const BY_KEYSYM = new Map(DEFS.map((d) => [d.keysym.toLowerCase(), d]));
const BY_VK = new Map(DEFS.filter((d) => d.vk != null).map((d) => [d.vk!, d]));

export function isModifierCode(code: string): boolean {
  return BY_CODE.get(code)?.modifier === true;
}

/**
 * The stored value for a recorded key. `codes` is every key held, in the order
 * pressed; only `"combo"` uses more than the last one. Returns null for a key
 * the format has no name for.
 */
export function encode(codes: string[], format: KeyFormat): string | null {
  const defs = codes.map((c) => BY_CODE.get(c));
  if (!defs.length || defs.some((d) => !d)) return null;
  if (format === "combo") {
    // Modifiers first, then the rest, each once: MangoHud matches the set.
    const uniq = [...new Set(defs as KeyDef[])];
    const ordered = [...uniq.filter((d) => d.modifier), ...uniq.filter((d) => !d.modifier)];
    return ordered.map((d) => d.keysym).join("+");
  }
  const last = defs[defs.length - 1]!;
  if (format === "keysym") return last.keysym;
  if (last.vk == null) return null;
  return `0x${last.vk.toString(16).toUpperCase().padStart(2, "0")}`;
}

/** Keycap labels for a stored value. An unknown name is shown verbatim, so a
 *  hand-edited config never displays as a different key. */
export function labels(value: string, format: KeyFormat): string[] {
  const v = value.trim();
  if (!v) return [];
  if (format === "vk") {
    const n = /^0x[0-9a-f]+$/i.test(v) ? parseInt(v, 16) : /^\d+$/.test(v) ? parseInt(v, 10) : NaN;
    return [BY_VK.get(n)?.label ?? v];
  }
  const parts = format === "combo" ? v.split("+") : [v];
  return parts.map((p) => BY_KEYSYM.get(p.trim().toLowerCase())?.label ?? p.trim());
}

/** Loose check for hand-typed values: tells the user when a name is unknown,
 *  without refusing it — every tool here accepts names this table lacks. */
export function isKnown(value: string, format: KeyFormat): boolean {
  const v = value.trim();
  if (!v) return true;
  if (format === "vk") {
    const n = /^0x[0-9a-f]+$/i.test(v) ? parseInt(v, 16) : /^-?\d+$/.test(v) ? parseInt(v, 10) : NaN;
    return Number.isInteger(n);
  }
  const parts = format === "combo" ? v.split("+") : [v];
  return parts.every((p) => BY_KEYSYM.has(p.trim().toLowerCase()));
}
