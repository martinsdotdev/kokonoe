/**
 * Reading a game file (spec §14): a `.utt` or JSON file, parsed here without the engine. The
 * engine checks what's read when `openFile` (fileimport.ts) opens it.
 */
import type { Cell, Player, Record as GameRecord, Ruleset } from '#lib/engine.js';
import type { Seat } from './moves.js';
import { parseNotation } from './notation.js';
import { Problem } from './problem.js';
import type { ReplaySource } from './replay.svelte.js';
import { rulesetCode } from './rules.js';
import { presetByCode } from './shelf.js';

/** A file Kokonoe can't read, with what's wrong in words for the player. */
export class FileProblem extends Problem {}

/** What a file says, before the engine opens it. */
export interface ParsedFile {
	record: GameRecord;
	/** The variant's name as the file gives it, if it does. */
	name: string | null;
	/** Every tag a `.utt` file has; none for JSON. */
	tags: Map<string, string>;
	/** The plies the file writes as dealt, to check against the Seed's deal. */
	dealt: Cell[];
}

const RECORD_KEYS = ['rules_version', 'ruleset', 'name', 'seed', 'moves'];

const parseJson = (text: string): ParsedFile => {
	let data: unknown;
	try {
		data = JSON.parse(text);
	} catch {
		throw new FileProblem('This file isn’t valid JSON.');
	}
	if (typeof data !== 'object' || data === null || Array.isArray(data)) {
		throw new FileProblem('This file holds no Record.');
	}
	const fields = data as { [key: string]: unknown };
	const unknown = Object.keys(fields).find((key) => !RECORD_KEYS.includes(key));
	if (unknown !== undefined) throw new FileProblem(`It has a field no Record has: “${unknown}”.`);
	const missing = RECORD_KEYS.find((key) => key !== 'name' && !(key in fields));
	if (missing !== undefined) throw new FileProblem(`Its Record has no “${missing}”.`);
	const { rules_version, ruleset, name, seed, moves } = fields;
	if (name !== undefined && typeof name !== 'string') {
		throw new FileProblem('Its variant name isn’t text.');
	}
	// The engine checks every field's shape when it opens the Record.
	const record = { rules_version, ruleset, seed, moves } as GameRecord;
	return { record, name: name ?? null, tags: new Map(), dealt: [] };
};

/** A tag line, `[Name "value"]`, its value's quotes and backslashes escaped. */
const TAG = /^\[(\w+)\s+"((?:[^"\\]|\\.)*)"\]$/;

const RESULTS: readonly string[] = ['1-0', '0-1', '1/2-1/2', '*'];

const parseUtt = (text: string, rulesetOf: (code: string) => Ruleset): ParsedFile => {
	const lines = text.split(/\r?\n/).map((line) => line.trim());
	const tags = new Map<string, string>();
	let line = 0;
	for (; line < lines.length; line++) {
		if (lines[line] === '') continue;
		const tag = TAG.exec(lines[line]);
		if (!tag) break;
		if (!tags.has(tag[1])) tags.set(tag[1], tag[2].replace(/\\(.)/g, '$1'));
	}
	const required = (name: string) => {
		const value = tags.get(name);
		if (value === undefined) throw new FileProblem(`This file names no ${name}.`);
		return value;
	};
	const code = required('Ruleset');
	let ruleset: Ruleset;
	try {
		ruleset = rulesetOf(code);
	} catch (error) {
		throw new FileProblem(
			`Its Ruleset “${code}” isn’t one Kokonoe knows: ${(error as Error).message}.`
		);
	}
	const seed = required('Seed');
	const version = required('RulesVersion');
	if (!/^\d+$/.test(version)) {
		throw new FileProblem(`Its RulesVersion “${version}” isn’t a number.`);
	}

	const dealt: Cell[] = [];
	const moves: Cell[] = [];
	for (const token of lines.slice(line).join(' ').split(/\s+/)) {
		// A move number says nothing the order doesn't. It stands alone, since `4.1` is a move.
		if (token === '' || /^\d+\.$/.test(token) || RESULTS.includes(token)) continue;
		const inner = /^\((.*)\)$/.exec(token)?.[1];
		const cell = parseNotation(inner ?? token, ruleset.depth);
		if (cell === null) throw new FileProblem(`“${token}” isn’t a move in Notation.`);
		if (inner === undefined) {
			moves.push(cell);
		} else if (moves.length) {
			throw new FileProblem(`The dealt move “${token}” comes after a move a player made.`);
		} else {
			dealt.push(cell);
		}
	}
	const record = { rules_version: Number(version), ruleset, seed, moves };
	return { record, name: tags.get('Variant') ?? null, tags, dealt };
};

/**
 * What a `.utt` or JSON file says, told apart by its first character: JSON opens with `{`.
 * `rulesetOf` reads a Ruleset code: the engine's `parseRulesetCode`. Throws a FileProblem.
 */
export const parseFile = (text: string, rulesetOf: (code: string) => Ruleset): ParsedFile =>
	text.trimStart().startsWith('{') ? parseJson(text) : parseUtt(text, rulesetOf);

/** The name a file's game goes by: a Preset's own, else the file's, else "Custom variant". */
export const variantName = (parsed: ParsedFile): string => {
	const preset = presetByCode(rulesetCode(parsed.record.ruleset));
	const name = parsed.name?.trim();
	return preset?.name ?? (name && !name.includes('\n') ? name.slice(0, 40) : 'Custom variant');
};

/** An imported file, to replay, once `openFile` has checked it and found who resigned. */
export const fromFile = (
	parsed: ParsedFile,
	variant: string,
	resigned: Player | null
): ReplaySource => {
	const seat = (player: Player, otherwise: string): Seat => ({
		name: parsed.tags.get(player)?.trim() || otherwise,
		detail: null
	});
	const date = parsed.tags.get('Date') ?? '';
	return {
		record: parsed.record,
		variant,
		seats: { X: seat('X', 'Player 1'), O: seat('O', 'Player 2') },
		against: 'Imported',
		resigned,
		date: /^\d{4}-\d{2}-\d{2}$/.test(date) ? date : ''
	};
};
