<script lang="ts">
  import { Switch as SwitchPrimitive } from "bits-ui";
  import { mergeStyle } from "$lib/util";

  let {
    checked = false,
    onchange,
    label = "",
    labelledby = undefined,
  }: {
    checked?: boolean;
    onchange?: () => void;
    label?: string;
    /**
     * Id of the visible label element. Preferred over `label`: pointing at the
     * text the user can already see stops a screen reader announcing the same
     * words twice (once as the control's name, once as loose text).
     */
    labelledby?: string;
  } = $props();
</script>

<!--
  bits-ui owns the semantics (role, aria-checked, data-state, Space/Enter) and
  the `child` snippet keeps our own <button>, classes and inline-`left` knob
  animation verbatim. A function binding keeps the switch fully controlled: the
  rendered state is always the caller's `checked`, never a copy bits-ui flipped
  on its own.
-->
<SwitchPrimitive.Root
  bind:checked={() => checked, () => onchange?.()}
  aria-label={labelledby ? undefined : label}
  aria-labelledby={labelledby}
>
  {#snippet child({ props })}
    <button
      {...props}
      class="relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors duration-200"
      style={mergeStyle(props, `background: ${checked ? "var(--accent)" : "var(--surface-2)"}`)}
    >
      <span
        class="absolute top-[3px] size-4 rounded-full bg-white shadow transition-all duration-200"
        style="left: {checked ? '19px' : '3px'}"
      ></span>
    </button>
  {/snippet}
</SwitchPrimitive.Root>
