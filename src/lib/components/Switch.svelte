<script lang="ts">
  let {
    checked = false,
    onchange,
    label = "",
  }: { checked?: boolean; onchange?: () => void; label?: string } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  onclick={onchange}
  class="relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
  style="background: {checked
    ? 'var(--accent)'
    : 'var(--surface-2)'}"
>
  <span
    class="absolute top-[3px] size-4 rounded-full bg-white shadow transition-all duration-200"
    style="left: {checked ? '19px' : '3px'}"
  ></span>
</button>
