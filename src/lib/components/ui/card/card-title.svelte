<script lang="ts">
  import { cn } from '$lib/utils'

  let className = ''
  export { className as class }
</script>

<h3 class={cn('text-sm font-semibold leading-none tracking-tight', className)} {...$$restProps}>
  <slot />
</h3>
