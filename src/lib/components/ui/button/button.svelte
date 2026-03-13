<script context="module" lang="ts">
  import { tv } from 'tailwind-variants'

  export const buttonVariants = tv({
    base: 'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2',
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground hover:bg-primary/90',
        secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/80',
        outline: 'border border-border bg-background hover:bg-accent hover:text-accent-foreground',
        ghost: 'hover:bg-accent hover:text-accent-foreground',
      },
      size: {
        default: 'h-9 px-4 py-2',
        sm: 'h-8 rounded-md px-3 text-xs',
        icon: 'h-9 w-9',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  })
</script>

<script lang="ts">
  import { cn } from '$lib/utils'

  export let variant: 'default' | 'secondary' | 'outline' | 'ghost' = 'default'
  export let size: 'default' | 'sm' | 'icon' = 'default'
  export let type: 'button' | 'submit' | 'reset' = 'button'
  let className = ''
  export { className as class }
</script>

<button class={cn(buttonVariants({ variant, size }), className)} {type} on:click {...$$restProps}>
  <slot />
</button>
