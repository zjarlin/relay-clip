<script lang="ts">
  import { cn } from '$lib/utils'

  let className = ''
  export { className as class }
</script>

<div class={cn('px-4 pb-4', className)} {...$$restProps}>
  <slot />
</div>
