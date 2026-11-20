<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import CheckIcon from '@lucide/svelte/icons/check';
	import { cn, type WithoutChild } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		value,
		label,
		children: childrenProp,
		...restProps
	}: WithoutChild<SelectPrimitive.ItemProps> = $props();
</script>

<SelectPrimitive.Item
	bind:ref
	{value}
	data-slot="select-item"
	class={cn(
		"relative flex h-8 w-full cursor-default items-center gap-2 rounded-[7px] pr-8 pl-2.5 text-[13.5px] outline-hidden select-none transition-colors duration-75 data-highlighted:bg-accent data-highlighted:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		className
	)}
	{...restProps}
>
	{#snippet children({ selected, highlighted })}
		<span class="absolute end-2.5 flex size-4 items-center justify-center text-foreground/70">
			{#if selected}
				<CheckIcon class="size-4" />
			{/if}
		</span>
		<span class="flex flex-1 gap-2 shrink-0 whitespace-nowrap">
			{#if childrenProp}
				{@render childrenProp({ selected, highlighted })}
			{:else}
				{label || value}
			{/if}
		</span>
	{/snippet}
</SelectPrimitive.Item>
