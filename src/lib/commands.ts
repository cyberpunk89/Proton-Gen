import type { Component } from "svelte";
import {
  ArrowCounterClockwise,
  ArrowsClockwise,
  ArrowSquareOut,
  Copy,
  Gear,
  ListChecks,
  Package,
  Sparkle,
  SquaresFour,
  SteamLogo,
  Terminal,
  Question,
  DownloadSimple,
  FloppyDisk,
  Eye,
} from "phosphor-svelte";

import { app } from "./state.svelte";
import { toast } from "./toast.svelte";
import { copyText } from "./util";

/**
 * Shared action definitions.
 *
 * Several of these exist in two places at once — a button in the UI and an entry
 * in the command palette. "Reset with an undo toast" in particular used to live
 * inline in CommandPreview; duplicating it into the palette is how the two
 * silently diverge (different toast text, one gets undo and the other doesn't).
 * Both call sites import from here instead.
 */
export interface AppCommand {
  id: string;
  label: string;
  icon?: Component;
  /** Extra words the palette should match on beyond the label. */
  keywords?: string[];
  /** False when the action makes no sense right now; the palette dims it. */
  available?: () => boolean;
  run: () => void | Promise<void>;
}

export const resetCommandAction: AppCommand = {
  id: "reset",
  label: "Reset command",
  icon: ArrowCounterClockwise,
  keywords: ["clear", "defaults"],
  run() {
    app.resetCommand();
    toast.success("Command reset", {
      action: { label: "Undo", onClick: () => app.undo() },
    });
  },
};

export const copyCommandAction: AppCommand = {
  id: "copy",
  label: "Copy command",
  icon: Copy,
  keywords: ["clipboard"],
  async run() {
    await copyText(app.command);
    toast.success("Command copied");
  },
};

export const backToLibraryAction: AppCommand = {
  id: "back",
  label: "Back to library",
  icon: SquaresFour,
  keywords: ["games", "grid"],
  available: () => app.view === "builder",
  run: () => app.backToLibrary(),
};

export const genericCommandAction: AppCommand = {
  id: "generic",
  label: "Build a generic command",
  icon: Terminal,
  keywords: ["no game"],
  run: () => app.openGeneric(),
};

export const toggleModeAction: AppCommand = {
  id: "mode",
  // Reads as the destination, not the current state: a palette entry is a verb.
  get label() {
    return app.umu ? "Switch to Steam mode" : "Switch to umu mode";
  },
  icon: Package,
  keywords: ["umu", "steam", "launcher"],
  run: () => app.setUmu(!app.umu),
};

export const refreshLibraryAction: AppCommand = {
  id: "refresh",
  label: "Refresh library",
  icon: ArrowsClockwise,
  keywords: ["rescan", "games"],
  available: () => !app.refreshing,
  // Same feedback as the header's refresh button: the palette closes on run, so
  // without a toast a failed re-scan looks exactly like a successful one.
  async run() {
    const result = await app.refresh();
    if (result === "ok") toast.success("Library refreshed");
    else if (result === "failed") toast.error("Couldn't refresh the library");
  },
};

export const openSettingsAction: AppCommand = {
  id: "settings",
  label: "Open settings",
  icon: Gear,
  keywords: ["preferences", "theme"],
  run: () => {
    app.showSettings = true;
  },
};

export const importCommandAction: AppCommand = {
  id: "import",
  label: "Import a command",
  icon: DownloadSimple,
  keywords: ["paste", "parse"],
  run: () => {
    app.showImport = true;
  },
};

export const savePresetAction: AppCommand = {
  id: "save-preset",
  label: "Save as preset",
  icon: FloppyDisk,
  keywords: ["store", "bookmark"],
  run: () => {
    app.showSave = true;
  },
};

export const activeOptionsAction: AppCommand = {
  id: "active",
  label: "Show active options",
  icon: ListChecks,
  keywords: ["enabled", "what have i turned on"],
  run: () => app.setSection("@active"),
};

export const recipesAction: AppCommand = {
  id: "recipes",
  label: "Browse recipes",
  icon: Sparkle,
  keywords: ["profiles", "troubleshooter"],
  run: () => app.setSection("recipes"),
};

export const toggleIrrelevantAction: AppCommand = {
  id: "show-unsupported",
  get label() {
    return app.store.show_irrelevant
      ? "Hide unsupported options"
      : "Show unsupported options";
  },
  icon: Eye,
  keywords: ["hardware", "irrelevant", "hidden"],
  run: () => app.setShowIrrelevant(!app.store.show_irrelevant),
};

export const openInSteamAction: AppCommand = {
  id: "open-steam",
  label: "Open this game in Steam",
  icon: SteamLogo,
  keywords: ["properties", "launch options"],
  available: () => app.steamAppId != null,
  async run() {
    const id = app.steamAppId;
    if (id == null) return;
    const { openSteamUrl, steamPropertiesUrl } = await import("./util");
    const { inTauri } = await import("./ipc");
    if (await openSteamUrl(steamPropertiesUrl(id))) return;
    toast.info(
      inTauri
        ? "Couldn't hand that link to Steam. Is Steam installed?"
        : "Steam deep links only work in the desktop app.",
    );
  },
};

export const shortcutsAction: AppCommand = {
  id: "shortcuts",
  label: "Keyboard shortcuts",
  icon: Question,
  keywords: ["keys", "bindings", "help"],
  run: () => {
    app.showShortcuts = true;
  },
};

export const undoAction: AppCommand = {
  id: "undo",
  label: "Undo",
  icon: ArrowCounterClockwise,
  keywords: ["revert"],
  run: () => app.undo(),
};

export const protondbAction: AppCommand = {
  id: "protondb",
  label: "Check ProtonDB for this game",
  icon: ArrowSquareOut,
  keywords: ["compatibility", "tier"],
  available: () => app.steamAppId != null,
  run() {
    const id = app.steamAppId;
    if (id != null) app.requestTier(id);
  },
};

/** Everything the palette offers under "Actions", in rough usefulness order. */
export const APP_COMMANDS: AppCommand[] = [
  copyCommandAction,
  resetCommandAction,
  undoAction,
  backToLibraryAction,
  activeOptionsAction,
  recipesAction,
  toggleModeAction,
  genericCommandAction,
  openInSteamAction,
  protondbAction,
  importCommandAction,
  savePresetAction,
  toggleIrrelevantAction,
  refreshLibraryAction,
  openSettingsAction,
  shortcutsAction,
];
