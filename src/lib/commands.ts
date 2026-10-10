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
  Trash,
  ArrowClockwise,
  FileText,
  Robot,
  Export,
  GlobeHemisphereWest,
  SlidersHorizontal,
} from "phosphor-svelte";

import { app } from "./state.svelte";
import { lookups } from "./lookups.svelte";
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

export const forgetTuningAction: AppCommand = {
  id: "forget-tuning",
  get label() {
    return `Forget tuning for ${app.selectedGameName ?? "this game"}`;
  },
  icon: Trash,
  keywords: ["clear", "reset", "memory", "untune"],
  available: () => app.hasGameMemory,
  run() {
    app.forgetGameTuning();
    toast.success("Forgot this game's tuning", {
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
    app.expectPaste();
    toast.success("Command copied");
  },
};

/** Steam's half of the paste loop in one step: copy, open the game's Properties
 *  (Launch Options is on the first tab), and show the checklist that ticks
 *  itself off as Steam's config catches up. A failed deep link still leaves the
 *  command copied, so it says so instead of pretending nothing happened. */
export const copyAndOpenSteamAction: AppCommand = {
  id: "copy-open-steam",
  label: "Copy & open in Steam",
  icon: SteamLogo,
  keywords: ["paste", "launch options", "properties", "clipboard"],
  available: () => app.steamAppId != null && !app.umu,
  async run() {
    const id = app.steamAppId;
    if (id == null) return;
    try {
      await copyText(app.command);
    } catch (e) {
      // Opening Steam with nothing on the clipboard would send the user off to
      // paste whatever was there before.
      toast.error(`Couldn't copy the command: ${e instanceof Error ? e.message : e}`);
      return;
    }
    app.expectPaste();
    app.pasteGuideFor = id;
    const { openSteamUrl, steamPropertiesUrl } = await import("./util");
    if (await openSteamUrl(steamPropertiesUrl(id))) return;
    const { inTauri } = await import("./ipc");
    toast.info(
      inTauri
        ? "Copied — but couldn't hand the link to Steam. Is Steam installed?"
        : "Copied. Steam deep links only work in the desktop app.",
    );
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
  run: () => app.openSettings(),
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
    if (await openSteamUrl(steamPropertiesUrl(id))) {
      app.expectPaste();
      return;
    }
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
    if (id != null) lookups.requestTier(id);
  },
};

export const redoAction: AppCommand = {
  id: "redo",
  label: "Redo",
  icon: ArrowClockwise,
  keywords: ["again"],
  run: () => app.redo(),
};

export const logsAction: AppCommand = {
  id: "logs",
  label: "View this game's Proton log",
  icon: FileText,
  keywords: ["log", "crash", "diagnostics", "dxvk", "vkd3d"],
  available: () => app.selectedAppId != null,
  run: () => {
    app.showLogs = true;
  },
};

export const troubleshootAction: AppCommand = {
  id: "troubleshoot",
  get label() {
    return app.store.llm_enabled ? "Troubleshoot a problem (AI)" : "Browse fixes for a problem";
  },
  icon: Robot,
  keywords: ["fix", "crash", "stutter", "black screen", "help", "symptom"],
  run: () => {
    // Without the AI coach there is still a symptom → fix list: the recipes
    // section's troubleshooter tab.
    if (app.store.llm_enabled) app.showTroubleshooter = true;
    else app.setSection("recipes");
  },
};

export const applyToLauncherAction: AppCommand = {
  id: "apply-launcher",
  get label() {
    return app.nexusSlug != null ? "Apply to Nexus" : "Apply to Heroic";
  },
  icon: Export,
  keywords: ["nexus", "heroic", "write", "launcher"],
  available: () => app.nexusSlug != null || app.heroicId != null,
  run: () => {
    if (app.nexusSlug != null) app.nexusConfirmOpen = true;
    else if (app.heroicId != null) app.heroicConfirmOpen = true;
  },
};

export const applyDefaultProfileAction: AppCommand = {
  id: "apply-default-profile",
  label: "Apply default profile",
  icon: GlobeHemisphereWest,
  keywords: ["global", "baseline", "profile"],
  available: () => app.store.global_profile != null && app.view === "builder",
  run: () => {
    app.applyGlobalProfile();
    toast.success("Default profile applied", {
      action: { label: "Undo", onClick: () => app.undo() },
    });
  },
};

export const uiModeAction: AppCommand = {
  id: "ui-mode",
  get label() {
    return app.uiMode === "simple" ? "Switch to Advanced view" : "Switch to Simple view";
  },
  icon: SlidersHorizontal,
  keywords: ["simple", "advanced", "layout"],
  run: () => app.setUiMode(app.uiMode === "simple" ? "advanced" : "simple"),
};

/** Everything the palette offers under "Actions", in rough usefulness order. */
export const APP_COMMANDS: AppCommand[] = [
  copyCommandAction,
  copyAndOpenSteamAction,
  resetCommandAction,
  forgetTuningAction,
  undoAction,
  redoAction,
  applyToLauncherAction,
  applyDefaultProfileAction,
  backToLibraryAction,
  logsAction,
  troubleshootAction,
  activeOptionsAction,
  recipesAction,
  toggleModeAction,
  uiModeAction,
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
