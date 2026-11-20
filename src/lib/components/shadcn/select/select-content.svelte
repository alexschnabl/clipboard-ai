<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { cn, type WithoutChild } from "$lib/utils.js";
	import type { WithoutChildrenOrChild } from "$lib/utils.js";
	import SelectPortal from "./select-portal.svelte";
	import SelectScrollDownButton from "./select-scroll-down-button.svelte";
	import SelectScrollUpButton from "./select-scroll-up-button.svelte";
	import type { ComponentProps } from "svelte";

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 4,
		portalProps,
		children,
		preventScroll = true,
		...restProps
	}: WithoutChild<SelectPrimitive.ContentProps> & {
		portalProps?: WithoutChildrenOrChild<ComponentProps<typeof SelectPortal>>;
	} = $props();
</script>

<SelectPortal {...portalProps}>
	<SelectPrimitive.Content
		bind:ref
		{sideOffset}
		{preventScroll}
		data-slot="select-content"
		class={cn(
			"min-w-36 rounded-[11px] border bg-popover p-1 text-popover-foreground shadow-[0_10px_32px_-6px_rgb(0_0_0/0.18),0_2px_6px_-2px_rgb(0_0_0/0.08)] dark:shadow-[0_10px_32px_-6px_rgb(0_0_0/0.6)] data-open:animate-in data-open:fade-in-0 data-open:zoom-in-[0.97] data-[side=bottom]:slide-in-from-top-1 data-[side=top]:slide-in-from-bottom-1 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-[0.97] duration-150 isolate z-50 max-h-(--bits-select-content-available-height) overflow-x-hidden overflow-y-auto",
			className
		)}
		{...restProps}
	>
		<SelectScrollUpButton />
		<SelectPrimitive.Viewport
			class={cn(
				"w-full min-w-(--bits-select-anchor-width) scroll-my-1"
			)}
		>
			{@render children?.()}
		</SelectPrimitive.Viewport>
		<SelectScrollDownButton />
	</SelectPrimitive.Content>
</SelectPortal>
