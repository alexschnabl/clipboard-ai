<script lang="ts">
	import type { Tone } from '$lib/health-format';

	let {
		tone,
		text,
		title,
		class: className = ''
	}: { tone: Tone; text: string; title?: string; class?: string } = $props();

	const DOT: Record<Tone, string> = {
		ok: 'bg-success',
		warn: 'bg-warning',
		error: 'bg-destructive',
		busy: 'animate-pulse bg-muted-foreground/60',
		idle: 'bg-muted-foreground/35'
	};
</script>

<span
	class={['flex min-w-0 items-start gap-2', className]}
	title={title && title !== text ? `${text}\n${title}` : text}
>
	<span class={['mt-[0.5lh] size-[7px] shrink-0 -translate-y-1/2 rounded-full', DOT[tone]]} aria-hidden="true"
	></span>
	<span class="line-clamp-2 min-w-0">{text}</span>
</span>
