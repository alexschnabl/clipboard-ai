// lib/table.ts — shared table styling.

/**
 * The sticky dense column-header style used by all three list tables
 * (Backends, Commands, History). Hoisted here because it was previously
 * copied verbatim into each of them: one source keeps the three tables from
 * drifting apart. Individual columns append width/alignment utilities
 * (`w-20`, `text-right`) alongside it.
 *
 * `text-foreground/65`, not `text-muted-foreground` (F3, review3-frontend.md):
 * muted-on-`bg-muted` computed to 4.15:1, just under WCAG AA's 4.5:1 for this
 * small, non-bold text. `text-foreground/65` is theme-aware for free and
 * clears AA in both themes against `bg-muted` -- 6.03:1 light, 6.88:1 dark,
 * computed from app.css's own oklch tokens.
 */
export const TABLE_HEAD_CLASS =
	'sticky top-0 z-10 h-7 bg-muted px-2 text-[10px] font-semibold tracking-wide text-foreground/65 uppercase';
