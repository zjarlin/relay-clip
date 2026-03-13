<script lang="ts">
  import { cn } from '$lib/utils'

  export let type = 'text'
  let className = ''
  export { className as class }
</script>

<input
  {type}
  class={cn(
    'flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50',
    className,
  )}
  on:input
  on:change
  on:blur
  on:focus
  {...$$restProps}
/>
