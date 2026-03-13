<script lang="ts">
  import { cn } from '$lib/utils'

  export let value = 0
  let className = ''
  export { className as class }

  $: normalizedValue = Math.max(0, Math.min(100, value))
</script>

<div
  aria-valuemax="100"
  aria-valuemin="0"
  aria-valuenow={normalizedValue}
  class={cn('relative h-2 w-full overflow-hidden rounded-full bg-secondary', className)}
  role="progressbar"
>
  <div
    class="h-full bg-primary transition-all"
    style={`width: ${normalizedValue}%`}
  ></div>
</div>
