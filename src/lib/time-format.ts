
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

	const sameYear = date.getFullYear() === reference.getFullYear();
	return new Intl.DateTimeFormat(locale, {
		month: 'short',
		day: 'numeric',
		year: sameYear ? undefined : 'numeric'
	}).format(date);
}

export function formatDuration(ms: number | null | undefined): string {
	if (ms == null) return '—';
	if (ms < 1000) return `${ms} ms`;
	return `${(ms / 1000).toFixed(1)} s`;
}
