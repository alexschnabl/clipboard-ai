<script lang="ts">
	import { Button } from '$lib/components/shadcn/button/index.js';
	import XIcon from '@lucide/svelte/icons/x';
	import CheckIcon from '@lucide/svelte/icons/check';
	import Keycaps from './Keycaps.svelte';
	import StatusLine from './StatusLine.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { healthState, isWorking } from '$lib/state/health.svelte.js';
	import { onboarding } from '$lib/state/onboarding.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';

	let { onOpenBackends }: { onOpenBackends: () => void } = $props();

	const t = $derived(i18n.t.commands.onboarding);
	const backends = $derived(configState.config.backends);
	const ids = $derived(backends.map((b) => b.id));
	const checking = $derived(ids.some((id) => healthState.isChecking(id)));
	const working = $derived(backends.find((b) => isWorking(healthState.results[b.id])));
	const allFailing = $derived(healthState.allCheckedAndFailing(ids));
	const firstCommand = $derived(configState.config.commands[0]);
	const step1Done = $derived(!!working);
	const step2Done = $derived(onboarding.succeeded);
	const commandBackend = $derived(
		firstCommand ? backends.find((b) => b.id === firstCommand.backendId) : undefined
	);
	const suggestSwitch = $derived(
		!!firstCommand &&
			!!working &&
			!!commandBackend &&
			working.id !== commandBackend.id &&
			!!healthState.results[commandBackend.id] &&
			!isWorking(healthState.results[commandBackend.id])
	);
	let switching = $state(false);

	async function useWorkingBackend(): Promise<void> {
		if (!firstCommand || !working) return;
		switching = true;
		await configState.updateCommand({ ...firstCommand, backendId: working.id });
		switching = false;
	}

	function hide(): void {
		onboarding.hiddenLocally = true;
		void configState.dismissOnboarding();
	}
</script>

<section class="list-card" aria-labelledby="getting-started-title">
	<div class="flex items-start gap-4 px-5 pt-4 pb-3.5">
		<div class="min-w-0 flex-1">
			<h2 id="getting-started-title" class="text-[15px]">{t.title}</h2>
			<p class="mt-1 text-[13px] text-muted-foreground">{t.intro}</p>
		</div>
		<Button
			variant="ghost"
			size="sm"
			class="-mt-1 -mr-2 text-muted-foreground"
			onclick={hide}
			aria-label={t.hideAria}
			title={t.hideAria}
		>
			<XIcon class="size-4" />
			{t.hide}
		</Button>
	</div>

	<div class="list-row items-start border-t">
		{@render marker(1, step1Done)}
		<div class="min-w-0 flex-1">
			<div>{t.step1Title}</div>
			<p class="mt-1 text-[13px] text-muted-foreground">{t.step1Text}</p>
			{#if checking}
				<StatusLine tone="busy" text={t.checking} class="mt-2 text-[13px] text-muted-foreground" />
			{:else if working}
				<StatusLine tone="ok" text={t.works(working.name)} class="mt-2 text-[13px] text-muted-foreground" />
			{:else if allFailing}
				<StatusLine tone="warn" text={t.noneWorking} class="mt-2 text-[13px] text-muted-foreground" />
			{/if}
		</div>
		<div class="flex shrink-0 items-center gap-2">
			{#if !working && ids.length > 0}
				<Button
					variant="ghost"
					size="sm"
					disabled={checking}
					onclick={() => void healthState.checkAll(ids)}>{t.checkBackends}</Button
				>
			{/if}
			<Button variant="outline" size="sm" onclick={onOpenBackends}>{t.openBackends}</Button>
		</div>
	</div>

	<div class="list-row items-start">
		{@render marker(2, step2Done)}
		<div class="min-w-0 flex-1">
			<div>{firstCommand ? t.step2Title(firstCommand.name) : i18n.t.commands.addCommand}</div>
			{#if step2Done}
				<StatusLine tone="ok" text={t.step2Done} class="mt-1 text-[13px] text-muted-foreground" />
			{:else if firstCommand?.hotkey}
				<p class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px] text-muted-foreground">
					<span>{t.step2Text}</span>
					<Keycaps hotkey={firstCommand.hotkey} />
				</p>
			{:else}
				<p class="mt-1 text-[13px] text-muted-foreground">{t.step2NoCommand}</p>
			{/if}
			{#if !step2Done && suggestSwitch && commandBackend}
				<StatusLine
					tone="warn"
					text={t.backendSilent(commandBackend.name)}
					class="mt-2 text-[13px] text-muted-foreground"
				/>
			{/if}
		</div>
		{#if !step2Done && suggestSwitch && working}
			<Button
				variant="outline"
				size="sm"
				class="shrink-0"
				disabled={switching}
				onclick={() => void useWorkingBackend()}>{t.useBackend(working.name)}</Button
			>
		{/if}
	</div>
</section>

{#snippet marker(n: number, done: boolean)}
	<span
		class={[
			'mt-px grid size-6 shrink-0 place-items-center rounded-full border text-xs tabular-nums transition-colors',
			done ? 'border-success/40 bg-success/10 text-success' : 'bg-canvas text-muted-foreground'
		]}
		aria-hidden="true"
	>
		{#if done}
			<CheckIcon class="size-3.5" />
		{:else}
			{n}
		{/if}
	</span>
{/snippet}
