<script lang="ts">
  import { cn } from '$lib/utils'

  export let variant: 'default' | 'secondary' | 'outline' = 'default'
  let className = ''
  export { className as class }

  const variants = {
    default: 'border-transparent bg-primary text-primary-foreground',
    secondary: 'border-transparent bg-secondary text-secondary-foreground',
    outline: 'text-foreground',
  } as const
</script>

<div
  class={cn(
    'inline-flex items-center rounded-md border px-2 py-0.5 text-[11px] font-medium',
    variants[variant],
    className,
  )}
  {...$$restProps}
>
  <slot />
</div>
