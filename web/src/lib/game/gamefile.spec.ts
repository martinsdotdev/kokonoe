import { describe, expect, it } from 'vitest';
import type { Ruleset } from '#lib/engine.js';
import { DEALT, LINE_OF_MIDDLE_BOARDS, TOP_ROW, cells } from './games.fixtures.js';
import {
	FileProblem,
	TAGS,
	dayOf,
	fileName,
	parseFile,
	result,
	termination,
	variantName,
	writeJson,
	writeUtt,
	type GameFile
} from './gamefile.js';
import { SHELF } from './shelf.js';

const STANDARD = SHELF[0].ruleset;
const DEEP_DEALT: Ruleset = { ...SHELF[4].ruleset, opening: { Random: 2 } };

/** The engine's `parseRulesetCode`, for the two codes these files use. */
const rulesetOf = (code: string): Ruleset => {
	const known: Record<string, Ruleset> = { std: STANDARD, 'd3+rand2': DEEP_DEALT };
	if (!(code in known)) throw new Error('a code this test doesn’t know');
	return known[code];
};

const topRow: GameFile = {
	record: { rules_version: 1, ruleset: STANDARD, seed: '00000000005eed00', moves: cells(TOP_ROW) },
	variant: 'Standard',
	seats: { X: 'Player 1', O: 'Player 2' },
	date: '2026-10-28',
	opening: [],
	end: { outcome: { Objective: 'X' } }
};

const dealt: GameFile = {
	record: {
		rules_version: 1,
		ruleset: DEEP_DEALT,
		seed: DEALT.seed,
		moves: cells(LINE_OF_MIDDLE_BOARDS)
	},
	variant: 'Custom variant',
	seats: { X: 'You', O: 'Computer, Level 3' },
	date: '2026-10-28',
	opening: cells(DEALT.opening),
	end: { outcome: { Objective: 'X' } }
};

describe('the .utt text', () => {
	it('writes the tags in their fixed order and spelling, then the moves in pairs', () => {
		expect(writeUtt(topRow)).toMatchInlineSnapshot(`
			"[Variant "Standard"]
			[Ruleset "std"]
			[Seed "00000000005eed00"]
			[RulesVersion "1"]
			[X "Player 1"]
			[O "Player 2"]
			[Date "2026-10-28"]
			[TimeControl "-"]
			[Result "1-0"]
			[Termination "line"]

			1. 1·4 4·1 2. 1·5 5·1 3. 1·6 6·2 4. 2·4 4·2 5. 2·5 5·2 6. 2·6 6·3 7. 3·7 7·3
			8. 3·8 8·3 9. 3·9 1-0
			"
		`);
	});

	it('writes dealt plies in parentheses, numbered from the first, wrapped at 80 columns', () => {
		const text = writeUtt(dealt);
		expect(text).toMatchInlineSnapshot(`
			"[Variant "Custom variant"]
			[Ruleset "d3+rand2"]
			[Seed "0000000000000016"]
			[RulesVersion "1"]
			[X "You"]
			[O "Computer, Level 3"]
			[Date "2026-10-28"]
			[TimeControl "-"]
			[Result "1-0"]
			[Termination "line"]

			1. (8·1·3) (1·3·9) 2. 3·9·1 9·1·4 3. 1·4·2 4·2·4 4. 2·4·2 4·2·6 5. 2·6·2 6·2·6
			6. 2·6·1 6·1·5 7. 1·5·3 5·3·8 8. 3·8·2 8·2·5 9. 2·5·3 5·3·7 10. 3·7·3 7·3·8
			11. 3·8·3 8·3·9 12. 3·9·2 9·2·6 13. 2·6·3 6·3·8 14. 3·8·1 8·1·4 15. 1·4·3 4·3·8
			16. 3·7·2 7·2·6 17. 2·4·1 4·1·5 18. 1·5·2 5·2·4 19. 2·4·3 4·3·9 20. 3·9·3 9·3·8
			21. 3·7·1 7·1·5 22. 1·5·1 5·1·4 23. 1·4·1 4·1·6 24. 1·6·3 6·3·3 25. 1·6·1 6·1·4
			26. 1·6·2 6·2·4 27. 2·5·1 5·1·6 28. 2·5·2 1-0
			"
		`);
		const moves = text.split('\n\n')[1].trimEnd().split('\n');
		expect(moves.every((line) => line.length <= 80)).toBe(true);
	});

	it('writes every tag the spec names, once', () => {
		const names = [...writeUtt(topRow).matchAll(/^\[(\w+) /gm)].map((tag) => tag[1]);
		expect(names).toEqual([...TAGS]);
	});

	it('escapes quotes and backslashes in a name, and reads them back', () => {
		const odd = { ...dealt, variant: 'The "fast" one \\ two' };
		const text = writeUtt(odd);
		expect(text).toContain('[Variant "The \\"fast\\" one \\\\ two"]');
		expect(parseFile(text, rulesetOf).name).toBe(odd.variant);
	});
});

describe('the Result and Termination', () => {
	const rules = (edit: Partial<Ruleset>): Ruleset => ({ ...STANDARD, ...edit });

	it('names each way a game ends', () => {
		const cases: [GameFile['end'], Ruleset, string, string][] = [
			[null, STANDARD, '*', '-'],
			[{ outcome: { Objective: 'X' } }, STANDARD, '1-0', 'line'],
			[{ outcome: { Objective: 'O' } }, rules({ objective: { FirstTo: 5 } }), '0-1', 'count'],
			[{ outcome: { MostWon: 'X' } }, rules({ fallback: 'MostWon' }), '1-0', 'count'],
			[{ outcome: 'Draw' }, rules({ fallback: 'MostWon' }), '1/2-1/2', 'count'],
			[{ outcome: 'Draw' }, STANDARD, '1/2-1/2', 'decided'],
			[{ resigned: 'X' }, STANDARD, '0-1', 'resign']
		];
		for (const [end, ruleset, said, how] of cases) {
			expect([result(end), termination(end, ruleset)]).toEqual([said, how]);
		}
	});
});

describe('the JSON Record', () => {
	it('writes a Preset’s Record in the spec’s order, without a name', () => {
		expect(writeJson(topRow)).toBe(
			'{"rules_version":1,"ruleset":{"depth":2,"continuation":"Off","drawn_owner":"Neither",' +
				'"objective":"Line","fallback":"Draw","opening":"Free"},"seed":"00000000005eed00",' +
				`"moves":[${cells(TOP_ROW).join(',')}]}\n`
		);
	});

	it('names a Custom variant between its Ruleset and its Seed', () => {
		const json = writeJson(dealt);
		expect(Object.keys(JSON.parse(json))).toEqual([
			'rules_version',
			'ruleset',
			'name',
			'seed',
			'moves'
		]);
		expect(JSON.parse(json).name).toBe('Custom variant');
	});
});

describe('reading a file', () => {
	it('reads back what it writes, in both formats', () => {
		for (const game of [topRow, dealt]) {
			const utt = parseFile(writeUtt(game), rulesetOf);
			expect(utt.record).toEqual(game.record);
			expect(utt.dealt).toEqual(game.opening);
			expect(utt.tags.get('X')).toBe(game.seats.X);
			const json = parseFile(writeJson(game), rulesetOf);
			expect(json.record).toEqual(game.record);
			expect(json.dealt).toEqual([]);
		}
		expect(variantName(parseFile(writeJson(topRow), rulesetOf))).toBe('Standard');
		expect(variantName(parseFile(writeJson(dealt), rulesetOf))).toBe('Custom variant');
	});

	it('takes `.` for `·`, and skips move numbers and the Result', () => {
		const text =
			'[Ruleset "std"]\n[Seed "00000000005eed00"]\n[RulesVersion "1"]\n\n1. 1.4 4.1 2. 1·5 *';
		expect(parseFile(text, rulesetOf).record.moves).toEqual(cells('03 30 04'));
	});

	it('says what’s wrong with a file it can’t read', () => {
		const tags = '[Ruleset "std"]\n[Seed "00000000005eed00"]\n[RulesVersion "1"]\n\n';
		const cases: [string, string][] = [
			['[Seed "00000000005eed00"]\n\n1. 5·5', 'This file names no Ruleset.'],
			[
				'[Ruleset "std+zz"]\n',
				'Its Ruleset “std+zz” isn’t one Kokonoe knows: a code this test doesn’t know.'
			],
			[
				'[Ruleset "std"]\n[Seed "00000000005eed00"]\n[RulesVersion "one"]',
				'Its RulesVersion “one” isn’t a number.'
			],
			[`${tags}1. 5·0`, '“5·0” isn’t a move in Notation.'],
			[`${tags}1. 5·5·5`, '“5·5·5” isn’t a move in Notation.'],
			[`${tags}1. 5·5 (5·1)`, 'The dealt move “(5·1)” comes after a move a player made.'],
			['{"rules_version":1', 'This file isn’t valid JSON.'],
			[
				'{"rules_version":1,"ruleset":{},"seed":"","moves":[],"x":1}',
				'It has a field no Record has: “x”.'
			],
			['{"rules_version":1,"ruleset":{},"moves":[]}', 'Its Record has no “seed”.'],
			[
				'{"rules_version":1,"ruleset":{},"name":7,"seed":"","moves":[]}',
				'Its variant name isn’t text.'
			]
		];
		for (const [text, message] of cases) {
			expect(() => parseFile(text, rulesetOf), text).toThrow(new FileProblem(message));
		}
	});
});

describe('the file’s name and date', () => {
	it('names the file by its day and variant', () => {
		expect(fileName(topRow, 'utt')).toBe('kokonoe-2026-10-28-standard.utt');
		expect(fileName({ ...dealt, date: '', variant: 'Ūltra: Tic!' }, 'json')).toBe(
			'kokonoe-ūltra-tic.json'
		);
	});

	it('dates a game by the day it began, here', () => {
		expect(dayOf(new Date(2026, 9, 28, 23, 59).getTime())).toBe('2026-10-28');
		expect(dayOf(new Date(2026, 0, 2).getTime())).toBe('2026-01-02');
	});
});
