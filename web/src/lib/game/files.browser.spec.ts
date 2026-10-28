import { beforeAll, describe, expect, it } from 'vitest';
import { Game, loadEngine, parseRulesetCode, presets, type Ruleset } from '#lib/engine.js';
import { DEALT, LINE_OF_MIDDLE_BOARDS, TOP_ROW, cells } from './games.fixtures.js';
import {
	FileProblem,
	parseFile,
	variantName,
	writeJson,
	writeUtt,
	type GameFile
} from './gamefile.js';
import { openFile } from './record.js';
import { rulesetCode } from './rules.js';
import { presetByCode } from './shelf.js';

beforeAll(() => loadEngine());

const read = (text: string) => parseFile(text, parseRulesetCode);

/** A whole game under `rules`, its moves picked by a small generator from `seed`. */
const randomGame = (rules: Ruleset, seed: number): Game => {
	const game = new Game(rules, seed.toString(16).padStart(16, '0'));
	let state = seed || 1;
	for (;;) {
		const status = game.status();
		if (!('ToMove' in status)) return game;
		state = (state * 1103515245 + 12345) % 2 ** 31;
		const { moves } = status.ToMove;
		game.play(game.ply(), moves[state % moves.length]);
	}
};

/** The file a finished engine Game makes, as the game screen writes it. */
const fileOf = (game: Game): GameFile => {
	const status = game.status();
	const record = game.record();
	return {
		record,
		variant: presetByCode(rulesetCode(record.ruleset))?.name ?? 'Custom variant',
		seats: { X: 'Player 1', O: 'Player 2' },
		date: '2026-10-28',
		opening: game.opening(),
		end: 'Over' in status ? { outcome: status.Over } : null
	};
};

describe('export then import', () => {
	it('round-trips every Preset at both depths, in both formats, dealt or not', () => {
		for (const [i, { ruleset }] of presets().entries()) {
			for (const depth of [2, 3] as const) {
				for (const opening of ['Free', { Random: 3 }] as const) {
					const game = randomGame({ ...ruleset, depth, opening }, i * 10 + depth);
					const file = fileOf(game);
					game.free();
					for (const text of [writeUtt(file), writeJson(file)]) {
						const parsed = read(text);
						expect(openFile(parsed)).toEqual({ resigned: null });
						expect(parsed.record).toEqual(file.record);
						expect(variantName(parsed)).toBe(file.variant);
					}
					expect(read(writeUtt(file)).dealt).toEqual(file.opening);
				}
			}
		}
	});

	it('keeps a resignation through the tags', () => {
		const game = new Game(presets()[0].ruleset, '00000000005eed00');
		game.play(0, cells('42')[0]);
		const file: GameFile = { ...fileOf(game), end: { resigned: 'O' } };
		game.free();
		expect(openFile(read(writeUtt(file)))).toEqual({ resigned: 'O' });
	});
});

describe('a file the engine refuses', () => {
	// Built in the tests, once the engine has loaded.
	const topRowFile = () =>
		writeUtt({
			record: {
				rules_version: 1,
				ruleset: presets()[0].ruleset,
				seed: '00000000005eed00',
				moves: cells(TOP_ROW)
			},
			variant: 'Standard',
			seats: { X: 'Player 1', O: 'Player 2' },
			date: '',
			opening: [],
			end: { outcome: { Objective: 'X' } }
		});
	const dealt = (): GameFile => ({
		record: {
			rules_version: 1,
			ruleset: parseRulesetCode(DEALT.code),
			seed: DEALT.seed,
			moves: cells(LINE_OF_MIDDLE_BOARDS)
		},
		variant: 'Custom variant',
		seats: { X: 'Player 1', O: 'Player 2' },
		date: '',
		opening: cells(DEALT.opening),
		end: { outcome: { Objective: 'X' } }
	});
	const refused = (text: string) => () => openFile(read(text));

	it('names its first bad move by ply, the deal counted, in Notation', () => {
		expect(refused(topRowFile().replace('2. 1·5', '2. 1·4'))).toThrow(
			new FileProblem('Move 3, X 1·4, isn’t legal.')
		);
		const file = dealt();
		file.record.moves = cells(`000 ${LINE_OF_MIDDLE_BOARDS}`);
		expect(refused(writeJson({ ...file, opening: [] }))).toThrow(
			new FileProblem('Move 3, X 1·1·1, isn’t legal.')
		);
	});

	it('refuses dealt plies the Seed doesn’t deal', () => {
		const text = writeUtt(dealt()).replace('(8·1·3)', '(8·1·4)');
		expect(refused(text)).toThrow(
			new FileProblem(`Its dealt opening isn’t what Seed ${DEALT.seed} deals.`)
		);
	});

	it('refuses a Result the moves don’t end in', () => {
		expect(refused(topRowFile().replace('[Result "1-0"]', '[Result "0-1"]'))).toThrow(
			new FileProblem('It says 0-1, but X wins on a line.')
		);
		const unfinished = topRowFile().replace(/ 3·9 1-0\n$/, ' *\n');
		expect(refused(unfinished)).toThrow(new FileProblem('It says 1-0, but the game isn’t over.'));
	});

	it('refuses a resignation after the game ended, or one that names no winner', () => {
		const resigned = topRowFile().replace('[Termination "line"]', '[Termination "resign"]');
		expect(refused(resigned)).toThrow(
			new FileProblem('It says a player resigned after the game ended.')
		);
		const short = resigned.replace(/ 2\. .*$/s, ' *\n').replace('[Result "1-0"]', '[Result "*"]');
		expect(refused(short)).toThrow(
			new FileProblem('It says a player resigned, but its Result names no winner.')
		);
	});

	it('says why a Record from another rules version can’t be read', () => {
		const json = JSON.parse(writeJson(dealt()));
		expect(refused(JSON.stringify({ ...json, rules_version: 2 }))).toThrow(/rules version 2/);
	});
});
