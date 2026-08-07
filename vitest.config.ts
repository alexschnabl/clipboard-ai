import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	// Needed so a test can import a `.svelte.ts` runes module (e.g.
	// `src/lib/state/config.svelte.ts`) directly -- without this plugin,
	// `$state(...)` etc. in that file are just calls to an undefined global
	// and the import blows up before any test in the file can run.
	plugins: [svelte()],
	test: {
		// Not just a DOM shim: `@sveltejs/vite-plugin-svelte` decides `client`
		// vs `server` codegen from Vite's environment `consumer`, and Vitest's
		// `node` environment reports `consumer: 'server'` -- SSR codegen for a
		// `$state` class field is a plain, non-reactive property, and
		// `$state.snapshot(x)` compiles straight down to `x` (nothing to
		// unwrap, so nothing to clone). `ConfigState.commit`'s rollback
		// (config.svelte.ts) relies on `$state.snapshot` producing a real,
		// independent deep clone -- under `node` the "snapshot" is the same
		// live reference the mutation is about to touch, so the rollback
		// tests can't fail even when the implementation is broken. `jsdom`
		// reports `consumer: 'client'`, matching how this module is actually
		// compiled for the settings window in a browser.
		environment: 'jsdom',
		include: ['test/unit/**/*.test.ts'],
		// @sveltejs/vite-plugin-svelte's module-compile transform races when
		// two test files are transformed concurrently under Vitest 4's
		// default file-level parallelism -- `this.environment` comes back
		// `undefined` inside the plugin's `transform` hook for whichever
		// file loses the race (reproduced: every file passes in isolation,
		// both fail together). The suite here is small enough that
		// serializing file execution costs nothing observable.
		fileParallelism: false
	}
});
