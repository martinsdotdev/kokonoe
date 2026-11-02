import { expect, it } from 'vitest';
import type { ServerMsg } from '#lib/engine.js';
import { readServerMsg } from './protocol.js';
import { welcome } from './room.fixtures.js';

const read = (message: unknown) => readServerMsg(JSON.stringify(message));

it('reads every message the server sends', () => {
	const messages: ServerMsg[] = [
		welcome('X'),
		welcome('O', undefined, [], {
			takebacks: true,
			offer: { kind: 'Takeback', by: 'X', lapses_in: 15_000 },
			draws: { X: 1, O: 3 },
			end: { result: '1/2-1/2', termination: 'agreed draw' },
			rematch: 'b'.repeat(20)
		}),
		{ type: 'Moved', ply: 0, cell: 40 },
		{ type: 'Rejected', reason: 'NotAllowed' },
		{ type: 'Ended', result: '0-1', termination: 'resign' },
		{ type: 'Offer', kind: 'Rematch', by: 'O', lapses_in: 60_000 },
		{ type: 'OfferAnswered', kind: 'Draw', accepted: false },
		{ type: 'TookBack', to_ply: 2 },
		{ type: 'RematchRoom', room: 'b'.repeat(20) }
	];
	for (const message of messages) expect(read(message)).toEqual(message);
});

it('reads a message of the wrong shape as nothing', () => {
	const { snapshot } = welcome('X');
	const wrong = [
		{ ...welcome('X'), snapshot: { ...snapshot, takebacks: undefined } },
		{ ...welcome('X'), snapshot: { ...snapshot, offer: { kind: 'Truce', by: 'X' } } },
		{ ...welcome('X'), snapshot: { ...snapshot, draws: { X: -1, O: 0 } } },
		{ ...welcome('X'), snapshot: { ...snapshot, rematch: 'nowhere' } },
		{ ...welcome('X'), snapshot: { ...snapshot, players: { X: 7, O: null } } },
		{ type: 'Moved', ply: 1.5, cell: 40 },
		{ type: 'OfferAnswered', kind: 'Draw', accepted: 'no' },
		{ type: 'Ended', result: '2-0', termination: 'resign' },
		{ type: 'Ended', result: '1-0', termination: 'timeout' },
		{ type: 'Offer', kind: 'Draw', by: 'Z', lapses_in: null },
		{ type: 'OfferAnswered', kind: 'Draw' },
		{ type: 'Offer', kind: 'Draw', by: 'X' },
		{ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: -1 },
		{ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: 1.5 },
		{ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: 'soon' },
		{ type: 'TookBack', to_ply: -1 },
		{ type: 'RematchRoom', room: '../r' },
		{ type: 'Claim' }
	];
	for (const message of wrong) expect(read(message), JSON.stringify(message)).toBeNull();
});
