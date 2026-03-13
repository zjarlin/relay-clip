<script lang="ts">
  import { cn } from '$lib/utils'

  let className = ''
  export { className as class }
</script>

<div class={cn('flex flex-col gap-1.5 p-4', className)} {...$$restProps}>
  <slot />
</div>
