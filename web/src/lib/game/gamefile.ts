/**
 * Game files (spec §4.3 and §4.4): the `.utt` text, PGN-style tags and then the moves in
 * Notation, and the Record as JSON, written here without the engine. `fileimport.ts` reads them
 * back, apart, so the game screen loads no reader.
 */
import type { Cell, Player, Record as GameRecord, Ruleset } from '#lib/engine.js';
import { notation } from './notation.js';
import { winner, type End } from './result.js';
import { rulesetCode } from './rules.js';
import { presetByCode } from './shelf.js';

/** A game as a file holds it. */
export interface GameFile {
	record: GameRecord;
	/** The Preset's name, or the Custom variant's. */
	variant: string;
	/** Who played each side, as the player bars name them: "Computer, Level 3". */
	seats: Record<Player, string>;
	/** The day the game began, `2026-10-28`, or '' when no one knows. */
	date: string;
	/** The plies the Random opening dealt, written in parentheses. */
	opening: Cell[];
	/** How it ended, or null while it goes on. */
	end: End | null;
}

/** The tags of a `.utt` file, in the order it writes them. */
export const TAGS = [
	'Variant',
	'Ruleset',
	'Seed',
	'RulesVersion',
	'X',
	'O',
	'Date',
	'TimeControl',
	'Result',
	'Termination'
] as const;

export type Result = '1-0' | '0-1' | '1/2-1/2' | '*';

/** The Result tag: who won, a draw, or `*` while the game goes on. */
export const result = (end: End | null): Result => {
	if (end === null) return '*';
	const player = winner(end);
	return player === null ? '1/2-1/2' : player === 'X' ? '1-0' : '0-1';
};

/** The Termination tag (spec §4.4): how the game ended, or `-` while it goes on. */
export const termination = (end: End | null, rules: Ruleset): string => {
	if (end === null) return '-';
	if ('resigned' in end) return 'resign';
	const { outcome } = end;
	if (outcome === 'Draw') return rules.fallback === 'MostWon' ? 'count' : 'decided';
	return 'Objective' in outcome && rules.objective === 'Line' ? 'line' : 'count';
};

/** A Preset's Ruleset goes by the Preset's name, so its JSON needs no `name`. */
const isPreset = (rules: Ruleset) => presetByCode(rulesetCode(rules)) !== undefined;

const quote = (value: string) => `"${value.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;

/** The longest line the moves are wrapped to. */
const WIDTH = 80;

/** Words joined by spaces into lines no longer than `WIDTH`, as long as each word fits. */
const wrap = (words: string[]): string[] =>
	words.reduce<string[]>((lines, word) => {
		const last = lines.at(-1);
		if (last !== undefined && last.length + 1 + word.length <= WIDTH) {
			lines[lines.length - 1] = `${last} ${word}`;
		} else {
			lines.push(word);
		}
		return lines;
	}, []);

/**
 * The `.utt` text: the tags in their fixed order, a blank line, then every ply in Notation,
 * numbered in X-then-O pairs from the first dealt one, dealt plies in parentheses, and the
 * Result last.
 */
export const writeUtt = (game: GameFile): string => {
	const { record } = game;
	const values: Record<(typeof TAGS)[number], string> = {
		Variant: game.variant,
		Ruleset: rulesetCode(record.ruleset),
		Seed: record.seed,
		RulesVersion: String(record.rules_version),
		X: game.seats.X,
		O: game.seats.O,
		Date: game.date || '-',
		TimeControl: '-',
		Result: result(game.end),
		Termination: termination(game.end, record.ruleset)
	};
	const depth = record.ruleset.depth;
	const plies = [...game.opening, ...record.moves];
	const moves = plies.map((cell, i) => {
		const move = i < game.opening.length ? `(${notation(cell, depth)})` : notation(cell, depth);
		// A pair's number stays on the line with X's move.
		return i % 2 ? move : `${i / 2 + 1}. ${move}`;
	});
	const tags = TAGS.map((tag) => `[${tag} ${quote(values[tag])}]`);
	return `${tags.join('\n')}\n\n${wrap([...moves, values.Result]).join('\n')}\n`;
};

/** The Record as JSON (spec §4.3), with the variant's `name` only when it's a Custom variant. */
export const writeJson = (game: GameFile): string => {
	const { rules_version, ruleset, seed, moves } = game.record;
	const name = isPreset(ruleset) ? {} : { name: game.variant };
	return `${JSON.stringify({ rules_version, ruleset, ...name, seed, moves })}\n`;
};

/** `kokonoe-2026-10-28-deep.utt`: the day, then the variant in letters and digits. */
export const fileName = (game: GameFile, extension: 'utt' | 'json'): string => {
	const slug = game.variant
		.toLowerCase()
		.replace(/[^\p{L}\p{N}]+/gu, '-')
		.replace(/^-|-$/g, '');
	return `${['kokonoe', game.date, slug || 'game'].filter(Boolean).join('-')}.${extension}`;
};
