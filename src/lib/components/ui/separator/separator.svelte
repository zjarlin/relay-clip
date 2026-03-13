<script lang="ts">
  import { cn } from '$lib/utils'

  export let orientation: 'horizontal' | 'vertical' = 'horizontal'
  let className = ''
  export { className as class }
</script>

<div
  aria-hidden="true"
  class={cn(
    'shrink-0 bg-border',
    orientation === 'horizontal' ? 'h-px w-full' : 'h-full w-px',
    className,
  )}
  {...$$restProps}
></div>
