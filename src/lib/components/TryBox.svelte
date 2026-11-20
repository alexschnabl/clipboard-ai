<script lang="ts">
	import { onDestroy } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import * as Select from '$lib/components/shadcn/select/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import PlayIcon from '@lucide/svelte/icons/play';
	import SquareIcon from '@lucide/svelte/icons/square';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import Keycaps from './Keycaps.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { wordDiff } from '$lib/diff';
	import { formatDuration } from '$lib/time-format';
	import {
		canPreview,
		editorErrorKey,
		normalizeExe,
		inertHtml,
		toEditorError,
		trySamples,
		tryInputsKey
	} from '$lib/editor';
	import type { CommandConfig, EditorError, TryResult } from '$lib/types';

	let { draft, sampleKey }: { draft: CommandConfig; sampleKey: string } = $props();

	const ANYWHERE = '';

	let text = $state('');
	let app = $state(ANYWHERE);
	let running = $state(false);
	let startedAt = $state(0);
	let now = $state(0);
	let generation = 0;
	let ticker: ReturnType<typeof setInterval> | undefined;
	let outcome = $state<
		| { kind: 'result'; result: TryResult; input: string; key: string; model: string }
		| { kind: 'error'; error: EditorError; key: string }
		| null
	>(null);

	$effect.pre(() => {
		text = trySamples.get(sampleKey) ?? '';
	});

	$effect(() => {
		trySamples.set(sampleKey, text);
	});

	const rules = $derived(draft.appRules ?? []);
	const selectedRule = $derived(
		app === ANYWHERE ? undefined : rules.find((r) => normalizeExe(r.exe) === app)
	);

	$effect(() => {
		if (app !== ANYWHERE && !selectedRule) app = ANYWHERE;
	});

	const effectiveBackendId = $derived(selectedRule?.backendId ?? draft.backendId);
	const backend = $derived(configState.config.backends.find((b) => b.id === effectiveBackendId));
	const previewable = $derived(canPreview(backend));
	const isHttp = $derived(backend?.kind === 'http');
	const commandSendOnly = $derived.by(() => {
		const own = configState.config.backends.find((b) => b.id === draft.backendId);
		if (!own || canPreview(own)) return false;
		return !rules.some((r) =>
			canPreview(configState.config.backends.find((b) => b.id === r.backendId))
		);
	});

	const currentKey = $derived(
		tryInputsKey([
			text,
			app,
			draft.systemPrompt,
			draft.backendId,
			draft.formatting,
			draft.appRules ?? [],
			configState.vocabulary
		])
	);
	const stale = $derived(
		outcome !== null && !running && previewable && outcome.key !== currentKey
	);
	const canRun = $derived(previewable && text.trim().length > 0 && draft.backendId.length > 0);

	const diff = $derived.by(() => {
		if (outcome?.kind !== 'result' || outcome.result.html) return null;
		return wordDiff(outcome.input, outcome.result.text);
	});
	const unchanged = $derived(
		outcome?.kind === 'result' &&
			!outcome.result.html &&
			outcome.input.trim() === outcome.result.text.trim()
	);

	function appLabel(exe: string): string {
		const rule = rules.find((r) => normalizeExe(r.exe) === exe);
		return rule?.name.trim() || rule?.exe || exe;
	}

	function stopTicker(): void {
		clearInterval(ticker);
		ticker = undefined;
	}

	async function run(): Promise<void> {
		if (!canRun) return;
		const mine = ++generation;
		const input = text;
		const key = currentKey;
		const model = modelText;
		running = true;
		startedAt = Date.now();
		now = startedAt;
		stopTicker();
		ticker = setInterval(() => (now = Date.now()), 200);
		try {
			const result = await invoke<TryResult>('try_command', {
				command: $state.snapshot(draft),
				text: input,
				app: app === ANYWHERE ? null : app
			});
			if (mine !== generation) return;
			outcome = { kind: 'result', result, input, key, model };
		} catch (e) {
			if (mine !== generation) return;
			outcome = { kind: 'error', error: toEditorError(e), key };
		} finally {
			if (mine === generation) {
				running = false;
				stopTicker();
			}
		}
	}

	function stop(): void {
		generation++;
		running = false;
		stopTicker();
	}

	function onKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			if (!running) void run();
		}
	}

	onDestroy(() => {
		generation++;
		stopTicker();
	});

	const elapsed = $derived(Math.max(0, Math.floor((now - startedAt) / 1000)));
	const modelText = $derived(backend?.model?.trim() || i18n.t.commands.tryIt.defaultModel);
</script>

<section class="shrink-0">
	<div class="group-title">
		<span>{i18n.t.commands.tryIt.title}</span>
		<span class="text-xs text-muted-foreground/80">{i18n.t.commands.tryIt.hint}</span>
	</div>

	{#if commandSendOnly}
		<div class="list-card px-5 py-4 text-[13px] leading-relaxed text-muted-foreground">
			{i18n.t.commands.tryIt.sendOnly}
		</div>
	{:else}
		<div class="list-card grid grid-cols-2">
			<div class="flex min-h-40 min-w-0 flex-col gap-2 p-4">
				<label for="try-input" class="text-xs text-muted-foreground"
					>{i18n.t.commands.tryIt.inputLabel}</label
				>
				<textarea
					id="try-input"
					bind:value={text}
					onkeydown={onKeydown}
					rows="4"
					spellcheck="false"
					placeholder={i18n.t.commands.tryIt.inputPlaceholder}
					class="max-h-64 min-h-20 flex-1 resize-none bg-transparent text-[13.5px] leading-relaxed outline-none [field-sizing:content] placeholder:text-muted-foreground/70"
				></textarea>
				<div class="flex flex-wrap items-center gap-x-2 gap-y-1.5 text-xs text-muted-foreground">
					{#if running}
						<Button variant="outline" size="sm" onclick={stop}>
							<SquareIcon class="size-3" />
							{i18n.t.commands.tryIt.stop}
						</Button>
					{:else}
						<Button variant="outline" size="sm" onclick={() => void run()} disabled={!canRun}>
							<PlayIcon class="size-3" />
							{i18n.t.commands.tryIt.run}
						</Button>
						<span class="flex items-center gap-1.5"
							>{i18n.t.commands.tryIt.shortcutOr} <Keycaps hotkey="Ctrl+Enter" /></span
						>
					{/if}
					{#if rules.length > 0}
						<div class="ml-auto flex min-w-0 items-center gap-1.5">
							<span class="shrink-0">{i18n.t.commands.tryIt.appLabel}</span>
							<Select.Root type="single" bind:value={app}>
								<Select.Trigger size="sm" class="max-w-44 min-w-0 text-xs" aria-label={i18n.t.commands.tryIt.appLabel}>
									<span class="truncate"
										>{app === ANYWHERE ? i18n.t.commands.tryIt.appAnywhere : appLabel(app)}</span
									>
								</Select.Trigger>
								<Select.Content>
									<Select.Item value={ANYWHERE} label={i18n.t.commands.tryIt.appAnywhere} />
									{#each rules as rule, i (i)}
										<Select.Item
											value={normalizeExe(rule.exe)}
											label={rule.name.trim() || rule.exe}
										/>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					{/if}
				</div>
				{#if isHttp && previewable}
					<p class="text-xs text-muted-foreground/80">{i18n.t.commands.tryIt.httpNote}</p>
				{/if}
			</div>

			<div class="flex min-h-40 min-w-0 flex-col gap-2 border-l bg-canvas p-4">
				<div class="flex items-center justify-between gap-2">
					<span class="text-xs text-muted-foreground">{i18n.t.commands.tryIt.outputLabel}</span>
					{#if stale}
						<span class="text-xs text-warning">{i18n.t.commands.tryIt.stale}</span>
					{/if}
				</div>

				<div
					class={[
						'max-h-72 min-h-20 flex-1 overflow-y-auto text-[13.5px] leading-relaxed select-text',
						stale && 'opacity-60'
					]}
					aria-live="polite"
					aria-busy={running}
				>
					{#if backend && !previewable}
						<p class="text-muted-foreground">{i18n.t.commands.tryIt.errors.nothingToPreview}</p>
					{:else if running}
						<div class="flex items-center gap-2 text-muted-foreground">
							<LoaderIcon class="size-4 animate-spin" />
							<span>{i18n.t.commands.tryIt.asking(backend?.name ?? '')}</span>
							{#if elapsed > 0}<span class="text-muted-foreground/70" aria-hidden="true"
									>{formatDuration(elapsed * 1000)}</span
								>{/if}
						</div>
					{:else if outcome?.kind === 'error'}
						{@const key = editorErrorKey(outcome.error.reason)}
						<div class="flex items-start gap-2.5">
							<span class="mt-[7px] size-2 shrink-0 rounded-full bg-destructive"></span>
							<div class="min-w-0">
								<p>{i18n.t.commands.tryIt.errors[key]}</p>
								{#if outcome.error.detail}
									<p class="mt-1 text-xs break-words text-muted-foreground">{outcome.error.detail}</p>
								{/if}
							</div>
						</div>
					{:else if outcome?.kind === 'result'}
						{#if outcome.result.html}
							<div class="rich-preview">{@html inertHtml(outcome.result.html)}</div>
						{:else if unchanged}
							<p class="whitespace-pre-wrap">{outcome.result.text}</p>
							<p class="mt-2 text-xs text-muted-foreground">{i18n.t.commands.tryIt.noChanges}</p>
						{:else if diff}
							<p class="whitespace-pre-wrap">
								{#each diff as part, i (i)}
									{#if part.kind === 'same'}{part.text}{:else if part.kind === 'added'}<ins
											class="rounded-[3px] bg-success/15 px-0.5 no-underline">{part.text}</ins
										>{:else}<del
											class="rounded-[3px] bg-destructive/12 px-0.5 text-muted-foreground decoration-destructive/60"
											>{part.text}</del
										>{/if}
								{/each}
							</p>
						{:else}
							<p class="whitespace-pre-wrap">{outcome.result.text}</p>
						{/if}
					{:else}
						<p class="text-muted-foreground/70">{i18n.t.commands.tryIt.idle}</p>
					{/if}
				</div>

				{#if outcome?.kind === 'result' && !running && previewable && outcome.result.warnings.length > 0}
					<p class="text-xs text-warning">{outcome.result.warnings.join(' ')}</p>
				{/if}
				{#if outcome?.kind === 'result' && !running && previewable}
					<div class="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
						<span class="size-1.5 shrink-0 rounded-full bg-success"></span>
						<span class="truncate">
							{[
								outcome.result.backendName,
								outcome.model,
								formatDuration(outcome.result.backendMs),
								outcome.result.rule ? i18n.t.commands.tryIt.ruleApplied(outcome.result.rule) : null
							]
								.filter(Boolean)
								.join(' - ')}
						</span>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</section>

<style>
	.rich-preview :global(p),
	.rich-preview :global(ul),
	.rich-preview :global(ol),
	.rich-preview :global(table),
	.rich-preview :global(h1),
	.rich-preview :global(h2),
	.rich-preview :global(h3) {
		margin: 0 0 0.5rem;
	}
	.rich-preview :global(:last-child) {
		margin-bottom: 0;
	}
	.rich-preview :global(ul) {
		list-style: disc;
		padding-left: 1.25rem;
	}
	.rich-preview :global(ol) {
		list-style: decimal;
		padding-left: 1.25rem;
	}
	.rich-preview :global(h1) {
		font-size: 1.2em;
	}
	.rich-preview :global(h2) {
		font-size: 1.1em;
	}
	.rich-preview :global(h3) {
		font-size: 1em;
		font-weight: 600;
	}
	.rich-preview :global(a) {
		text-decoration: underline;
		cursor: default;
	}
	.rich-preview :global(b),
	.rich-preview :global(strong) {
		font-weight: 600;
	}
	.rich-preview :global(td),
	.rich-preview :global(th) {
		border: 1px solid var(--border);
		padding: 2px 6px;
	}
</style>
