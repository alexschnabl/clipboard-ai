export type DiffPart = { kind: 'same' | 'added' | 'removed'; text: string };

const MAX_CELLS = 400_000;

export function wordDiff(before: string, after: string): DiffPart[] | null {
	const a = before.split(/(\s+)/).filter((t) => t.length > 0);
	const b = after.split(/(\s+)/).filter((t) => t.length > 0);
	const n = a.length;
	const m = b.length;
	if ((n + 1) * (m + 1) > MAX_CELLS) return null;

	const width = m + 1;
	const lcs = new Uint32Array((n + 1) * width);
	const at = (i: number, j: number): number => lcs[i * width + j] ?? 0;
	for (let i = n - 1; i >= 0; i--) {
		for (let j = m - 1; j >= 0; j--) {
			lcs[i * width + j] = a[i] === b[j] ? at(i + 1, j + 1) + 1 : Math.max(at(i + 1, j), at(i, j + 1));
		}
	}

	const parts: DiffPart[] = [];
	const push = (kind: DiffPart['kind'], text: string): void => {
		const effective = text.trim().length === 0 ? 'same' : kind;
		const last = parts[parts.length - 1];
		if (last && last.kind === effective) last.text += text;
		else parts.push({ kind: effective, text });
	};

	let i = 0;
	let j = 0;
	while (i < n && j < m) {
		const x = a[i] ?? '';
		const y = b[j] ?? '';
		if (x === y) {
			push('same', x);
			i++;
			j++;
		} else if (at(i + 1, j) >= at(i, j + 1)) {
			push('removed', x);
			i++;
		} else {
			push('added', y);
			j++;
		}
	}
	for (; i < n; i++) push('removed', a[i] ?? '');
	for (; j < m; j++) push('added', b[j] ?? '');
	return parts;
}
