<script lang="ts">
  import { cn } from '$lib/utils'

  let className = ''
  export { className as class }
</script>

<div class={cn('overflow-auto', className)} {...$$restProps}>
  <slot />
</div>
