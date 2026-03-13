<script lang="ts">
  import { cn } from '$lib/utils'

  let className = ''
  export { className as class }
</script>

<p class={cn('text-xs text-muted-foreground', className)} {...$$restProps}>
  <slot />
</p>
