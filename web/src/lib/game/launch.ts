/**
 * What game `/play` starts, carried in its URL so a reload restarts the same game:
 * `?rules=<Ruleset code>` for pass and play, plus `&vs=computer&level=N&seat=X|O` against the
 * computer. The New game sheet and Home write these links; `/play` reads them.
 */
import type { Player } from '#lib/engine.js';

export const LEVELS = [1, 2, 3, 4, 5, 6, 7, 8] as const;
export type Level = (typeof LEVELS)[number];

/** Levels 1 to 4 arrive first (KOK-41), so the default is among them. */
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

/** The game a `/play` URL asks for, or what's wrong with it. */
export const readLaunch = (params: Pick<URLSearchParams, 'get'>): Launch | { error: string } => {
	const code = params.get('rules') ?? 'std';
	const vs = params.get('vs') ?? 'hotseat';
	if (vs === 'hotseat') return { code, opponent: { kind: 'hotseat' } };
	if (vs !== 'computer') return { error: `“${vs}” isn’t an opponent.` };
	const level = Number(params.get('level') ?? DEFAULT_LEVEL);
	if (!isLevel(level)) return { error: `There’s no Level ${params.get('level')}.` };
	const seat = params.get('seat') ?? 'X';
	if (seat !== 'X' && seat !== 'O') return { error: `“${seat}” isn’t a seat.` };
	return { code, opponent: { kind: 'computer', level, seat } };
};
