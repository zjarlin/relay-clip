<script lang="ts">
  import { cn } from '$lib/utils'

  let className = ''
  export { className as class }
</script>

<div
  class={cn('rounded-xl border bg-card text-card-foreground shadow-xs', className)}
  {...$$restProps}
>
  <slot />
</div>
