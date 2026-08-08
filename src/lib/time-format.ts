// lib/time-format.ts — small, dependency-free formatters for history's
// wall-clock timestamps and cycle durations. Split out of HistoryPanel.svelte
// so the formatting rules are unit-testable without mounting a component.

/**
 * Formats a wall-clock timestamp (`HistoryEntry.timestampMs`, milliseconds
 * since the Unix epoch -- see history.rs's "Timestamps" doc section) the way
 * the history table needs: a bare time ("14:32") for anything that happened
 * today, a date for anything older, since "14:32" alone would be ambiguous
 * -- and misleading -- for an entry from last week.
 *
 * `referenceMs` defaults to "now" but is a parameter (not a bare `Date.now()`
 * call inside) so a test can pin "today" without mocking the system clock.
 * `locale` is likewise a parameter, defaulting to the runtime's own (`Intl`
 * treats `undefined` as "use the environment's locale") -- passing one
 * explicitly is what makes this formatter's *rule* (not just its output for
 * one locale) testable deterministically.
 */
export function formatHistoryTimestamp(
	timestampMs: number,
	referenceMs: number = Date.now(),
	locale?: string
): string {
	const date = new Date(timestampMs);
	const reference = new Date(referenceMs);

	const isToday =
		date.getFullYear() === reference.getFullYear() &&
		date.getMonth() === reference.getMonth() &&
		date.getDate() === reference.getDate();

	if (isToday) {
		return new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(date);
	}

	// A different calendar year needs the year spelled out to stay
	// unambiguous ("Mar 3" from 14 months ago reads as recent otherwise);
	// anything within the current year doesn't.
	const sameYear = date.getFullYear() === reference.getFullYear();
	return new Intl.DateTimeFormat(locale, {
		month: 'short',
		day: 'numeric',
		year: sameYear ? undefined : 'numeric'
	}).format(date);
}

/**
 * Formats a duration in milliseconds (`backendMs`/`totalCycleMs`) the way
 * the mockup's history table does -- sub-second durations as whole
 * milliseconds (a 0.6 s abort would otherwise round to "1 s" and look like
 * it took a full second), everything else as seconds to one decimal place.
 * `null`/`undefined` covers `totalCycleMs`, which `history.rs` leaves unset
 * whenever nothing timed the whole cycle (see that field's doc comment).
 */
export function formatDuration(ms: number | null | undefined): string {
	if (ms == null) return '—';
	if (ms < 1000) return `${ms} ms`;
	return `${(ms / 1000).toFixed(1)} s`;
}
