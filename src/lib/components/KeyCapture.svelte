<script lang="ts">
  import { tick } from "svelte";
  import { ArrowCounterClockwise, PencilSimple, Record as RecordIcon, Prohibit } from "phosphor-svelte";
  import { encode, isKnown, isModifierCode, labels, type KeyFormat } from "$lib/keycapture";

  /**
   * The app's one way to assign a key: press it. Replaces fixed lists of
   * "common" keys, which could neither offer every key nor show a key a
   * hand-edited config already set (it displayed as the wrong option).
   *
   * Recording listens on the focused button itself rather than the window:
   * stopping the event there keeps it away from bits-ui's Escape layer (on
   * `document`) and the global shortcut table (on `window`), so pressing Escape
   * or "/" to *assign* it can't close the builder dialog or open search.
   */
  let {
    value,
    format,
    label,
    defaultLabel,
    onchange,
    disableValue,
  }: {
    value: string;
    format: KeyFormat;
    /** Accessible name, e.g. "In-game toggle key". */
    label: string;
    /** What the tool uses when the value is empty, e.g. "Home". */
    defaultLabel: string;
    onchange: (v: string) => void;
    /** A value meaning "no hotkey" (OptiScaler's `-1`); shows a Disable action. */
    disableValue?: string;
  } = $props();

  let recording = $state(false);
  let editing = $state(false);
  let draft = $state("");
  let hint = $state("");
  let held: string[] = [];
  let sawKey = false;
  let btn = $state<HTMLButtonElement | null>(null);
  let input = $state<HTMLInputElement | null>(null);

  let disabled = $derived(disableValue != null && value === disableValue);
  let caps = $derived(disabled ? [] : labels(value, format));

  async function startRecording() {
    editing = false;
    recording = true;
    hint = "";
    held = [];
    sawKey = false;
    await tick();
    btn?.focus();
  }

  function stop() {
    recording = false;
    held = [];
  }

  function commit(codes: string[]) {
    const v = encode(codes, format);
    if (v == null) {
      hint = "That key can't be used here — try another.";
      held = [];
      sawKey = false;
      return;
    }
    onchange(v);
    hint = "";
    stop();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    // No physical key (an IME or synthetic event): nothing to name.
    if (e.repeat || !e.code) return;
    // Escape alone cancels; there's no sensible reason to bind it in-game.
    if (e.code === "Escape" && held.length === 0) return stop();
    if (!held.includes(e.code)) held.push(e.code);
    if (isModifierCode(e.code)) return; // wait: either a key follows, or it's released alone
    sawKey = true;
    if (format !== "combo") return commit([e.code]);
    const mods = held.filter(isModifierCode);
    // A modifier held since before recording started never sent us a keydown;
    // the event's flags still know about it. Side unknown, so assume left.
    const flags: [boolean, string][] = [
      [e.ctrlKey, "Control"],
      [e.shiftKey, "Shift"],
      [e.altKey, "Alt"],
      [e.metaKey, "Meta"],
    ];
    for (const [on, name] of flags)
      if (on && !mods.some((c) => c.startsWith(name))) mods.push(`${name}Left`);
    commit([...mods, e.code]);
  }

  function onkeyup(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    // A modifier pressed and released on its own is the key being assigned.
    if (e.code && !sawKey && isModifierCode(e.code) && held.includes(e.code)) {
      commit(format === "combo" ? held : [e.code]);
    }
  }

  async function startEditing() {
    stop();
    editing = true;
    draft = value;
    await tick();
    input?.select();
  }

  function saveDraft() {
    if (!editing) return;
    editing = false;
    const v = draft.trim();
    if (v !== value) onchange(v);
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      saveDraft();
    } else if (e.key === "Escape") {
      // Cancel the edit, not the host dialog.
      e.preventDefault();
      e.stopPropagation();
      editing = false;
    }
  }

  const iconBtn =
    "shrink-0 rounded-md p-1 text-muted transition hover:bg-surface-2 hover:text-text disabled:opacity-30";
</script>

<div class="flex flex-col items-end gap-1">
  <div class="flex items-center gap-1">
    {#if editing}
      <input
        bind:this={input}
        bind:value={draft}
        onkeydown={onInputKey}
        onblur={saveDraft}
        aria-label="{label} (type a key name)"
        placeholder={format === "vk" ? "0x24" : format === "combo" ? "Shift_R+F12" : "Home"}
        class="w-36 rounded-lg border border-accent bg-surface-2 px-2 py-1 font-mono text-xs text-text outline-none"
      />
    {:else}
      <button
        bind:this={btn}
        type="button"
        aria-label="{label}: {recording ? 'press a key, Escape to cancel' : disabled ? 'disabled' : caps.join(' + ') || `default, ${defaultLabel}`}. Activate to record a new key."
        onclick={() => (recording ? stop() : startRecording())}
        {onkeydown}
        {onkeyup}
        onblur={stop}
        class="keycap-field flex min-w-36 items-center whitespace-nowrap justify-center gap-1 rounded-lg border px-2 py-1 text-xs transition {recording
          ? 'recording border-accent bg-accent/10 text-text'
          : 'border-border bg-surface-2 text-subtext hover:border-accent/40'}"
      >
        {#if recording}
          <RecordIcon size={12} weight="fill" class="text-red" />
          <span>Press a key…</span>
        {:else if disabled}
          <span class="text-muted">Disabled</span>
        {:else if caps.length}
          {#each caps as c, i (i)}
            {#if i > 0}<span class="text-muted">+</span>{/if}
            <kbd class="kbd">{c}</kbd>
          {/each}
        {:else}
          {#each defaultLabel.split(" + ") as c, i (i)}
            {#if i > 0}<span class="text-muted">+</span>{/if}
            <kbd class="kbd opacity-60">{c}</kbd>
          {/each}
          <span class="ml-0.5 text-muted">default</span>
        {/if}
      </button>
    {/if}
    <button type="button" class={iconBtn} title="Type a key name" aria-label="Type a key name for {label}" onclick={startEditing}>
      <PencilSimple size={13} />
    </button>
    {#if disableValue != null}
      <button
        type="button"
        class={iconBtn}
        title="No hotkey"
        aria-label="Disable {label}"
        disabled={disabled}
        onclick={() => onchange(disableValue)}
      >
        <Prohibit size={13} />
      </button>
    {/if}
    <button
      type="button"
      class={iconBtn}
      title="Reset to default ({defaultLabel})"
      aria-label="Reset {label} to default"
      disabled={value === ""}
      onclick={() => {
        stop();
        onchange("");
      }}
    >
      <ArrowCounterClockwise size={13} />
    </button>
  </div>
  {#if hint}
    <p class="text-[11px] text-yellow" role="status">{hint}</p>
  {:else if editing && draft.trim() && !isKnown(draft, format)}
    <p class="text-[11px] text-muted">Not a name this app recognises — it will be written as typed.</p>
  {/if}
</div>

<style>
  .kbd {
    display: inline-flex;
    align-items: center;
    min-width: 1.4rem;
    justify-content: center;
    padding: 0.05rem 0.4rem;
    border-radius: 0.35rem;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    background: var(--surface-solid);
    color: var(--text);
    font-family: ui-monospace, monospace;
    font-size: 11px;
    line-height: 1.3rem;
    white-space: nowrap;
  }
  .recording {
    animation: keycap-pulse 1.2s ease-in-out infinite;
  }
  @keyframes keycap-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 45%, transparent);
    }
    50% {
      box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 0%, transparent);
    }
  }
</style>
