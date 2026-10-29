/**
 * Reading a game file back (spec §14): a `.utt` or JSON file, parsed here without the engine,
 * then checked with it by `openFile` before the Replay viewer shows it.
 */
import {
	Game,
	type Cell,
	type Player,
	type Record as GameRecord,
	type ReplayError,
	type Ruleset
} from '#lib/engine.js';
import { result } from './gamefile.js';
import { playerAt } from './moves.js';
import { notation, parseNotation } from './notation.js';
import { resultText, type End } from './result.js';
import { rulesetCode } from './rules.js';
import { presetByCode } from './shelf.js';

/** A file Kokonoe can't read, with what's wrong in words for the player. */
export class FileProblem extends Error {}

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

/**
 * Checks a game file with the engine, as far as its tags let it: every move legal, the plies it
 * writes as dealt the ones its Seed deals, and its Result how the moves end. Returns who
 * resigned, which only a `.utt` file's tags can say. Throws a FileProblem.
 */
export const openFile = (parsed: ParsedFile): { resigned: Player | null } => {
	const { record, tags } = parsed;
	let opened: Game | ReplayError;
	try {
		opened = Game.open(record);
	} catch (error) {
		throw new FileProblem(`This file can’t be read: ${(error as Error).message}.`);
	}
	if (!(opened instanceof Game)) {
		// Number the move as the move list does: by its ply, the dealt opening counted.
		const deal = new Game(record.ruleset, record.seed);
		const ply = deal.opening().length + opened.index + 1;
		deal.free();
		const cell = notation(record.moves[opened.index], record.ruleset.depth);
		throw new FileProblem(`Move ${ply}, ${playerAt(ply)} ${cell}, isn’t legal.`);
	}
	const game = opened;
	try {
		const opening = game.opening();
		if (parsed.dealt.length && parsed.dealt.join() !== opening.join()) {
			throw new FileProblem(`Its dealt opening isn’t what Seed ${record.seed} deals.`);
		}
		const status = game.status();
		const said = tags.get('Result');
		let resigned: Player | null = null;
		if (tags.get('Termination') === 'resign') {
			if ('Over' in status)
				throw new FileProblem('It says a player resigned after the game ended.');
			if (said !== '1-0' && said !== '0-1') {
				throw new FileProblem('It says a player resigned, but its Result names no winner.');
			}
			resigned = said === '1-0' ? 'O' : 'X';
		}
		const end: End | null = resigned
			? { resigned }
			: 'Over' in status
				? { outcome: status.Over }
				: null;
		if (said !== undefined && said !== result(end)) {
			const won = { X: game.wonCount('X'), O: game.wonCount('O') };
			const truth = end ? resultText(end, record.ruleset, won) : 'the game isn’t over';
			throw new FileProblem(`It says ${said}, but ${truth}.`);
		}
		return { resigned };
	} finally {
		game.free();
	}
};
