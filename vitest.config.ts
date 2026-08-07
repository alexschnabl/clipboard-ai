import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	// Needed so a test can import a `.svelte.ts` runes module (e.g.
	// `src/lib/state/config.svelte.ts`) directly -- without this plugin,
	// `$state(...)` etc. in that file are just calls to an undefined global
	// and the import blows up before any test in the file can run.
	plugins: [svelte()],
	test: {
		environment: 'node',
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
