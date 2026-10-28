/**
 * What game `/play` shows, carried in its URL. A new game is `?rules=<Ruleset code>` for pass and
 * play, plus `&vs=computer&level=N&seat=X|O` against the computer; the New game sheet and Home
 * write these links. Once a game is saved, its address becomes `?game=<id>`, so a reload resumes
 * it.
 */
import type { Player } from '#lib/engine.js';

export const LEVELS = [1, 2, 3, 4, 5, 6, 7, 8] as const;
export type Level = (typeof LEVELS)[number];

/** Level 3 rather than the design's 5, chosen in KOK-40 while only Levels 1 to 4 played. */
export const DEFAULT_LEVEL: Level = 3;

export type Opponent = { kind: 'hotseat' } | { kind: 'computer'; level: Level; seat: Player };

export interface Launch {
	code: string;
	opponent: Opponent;
}

const isLevel = (n: number): n is Level => (LEVELS as readonly number[]).includes(n);

/** The query string of `/play` for `launch`. */
export const playQuery = ({ code, opponent }: Launch): `?${string}` => {
	const params = new URLSearchParams({ rules: code });
	if (opponent.kind === 'computer') {
		params.set('vs', 'computer');
		params.set('level', String(opponent.level));
		params.set('seat', opponent.seat);
	}
	return `?${params}`;
};

/** The query string of `/play` for the saved game `id`. */
export const gameQuery = (id: string): `?${string}` => `?${new URLSearchParams({ game: id })}`;

/** The query string of `/replay` for the saved game `id`, shown at `ply` when it's past the start. */
export const replayQuery = (id: string, ply = 0): `?${string}` =>
	`?${new URLSearchParams({ game: id, ...(ply ? { ply: String(ply) } : {}) })}`;

/** Who the game is against, for headers and lists: "vs Computer · Level 3", "Pass and play". */
export const againstLabel = (opponent: Opponent): string =>
	opponent.kind === 'computer' ? `vs Computer · Level ${opponent.level}` : 'Pass and play';

/** The new game a `/play` URL asks for, or what's wrong with it. */
export const readLaunch = (params: Pick<URLSearchParams, 'get'>): Launch | { error: string } => {
	// A `+` typed into a query reads back as a space, and a Ruleset code never holds one.
	const code = (params.get('rules') ?? 'std').replaceAll(' ', '+');
	const vs = params.get('vs') ?? 'hotseat';
	if (vs === 'hotseat') return { code, opponent: { kind: 'hotseat' } };
	if (vs !== 'computer') return { error: `“${vs}” isn’t an opponent.` };
	const level = Number(params.get('level') ?? DEFAULT_LEVEL);
	if (!isLevel(level)) return { error: `There’s no Level ${params.get('level')}.` };
	const seat = params.get('seat') ?? 'X';
	if (seat !== 'X' && seat !== 'O') return { error: `“${seat}” isn’t a seat.` };
	return { code, opponent: { kind: 'computer', level, seat } };
};
