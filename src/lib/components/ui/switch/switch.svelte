<script lang="ts">
  import { cn } from '$lib/utils'

  export let checked = false
  export let disabled = false
  let className = ''
  export { className as class }
</script>

<label class={cn('inline-flex cursor-pointer items-center', className)}>
  <input
    bind:checked
    class="peer sr-only"
    {disabled}
    on:change
    type="checkbox"
    {...$$restProps}
  />
  <span
    class="relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent bg-input transition-colors peer-checked:bg-primary peer-focus-visible:ring-2 peer-focus-visible:ring-ring peer-disabled:cursor-not-allowed peer-disabled:opacity-50"
  >
    <span
      class="pointer-events-none absolute left-0.5 h-4 w-4 rounded-full bg-background shadow-xs transition-transform peer-checked:translate-x-4"
    ></span>
  </span>
</label>
