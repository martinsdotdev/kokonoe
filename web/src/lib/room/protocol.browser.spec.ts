import { expect, it } from 'vitest';
import type { ServerMsg } from '#lib/engine.js';
import { readServerMsg } from './protocol.js';
import { welcome } from './room.fixtures.js';

const read = (message: unknown) => readServerMsg(JSON.stringify(message));

const clocks = {
	x_ms: 171_250,
	o_ms: 180_000,
	running: 'X',
	at: 1_793_600_000_000,
	aborts_at: null
} as const;

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
		welcome('X', undefined, [], {
			clock: '3+2',
			clocks: { ...clocks, running: null, aborts_at: 1_793_600_030_000 }
		}),
		{ type: 'Moved', ply: 0, cell: 40, clocks: null },
		{ type: 'Moved', ply: 2, cell: 40, clocks },
		{ type: 'Ended', result: '1-0', termination: 'timeout' },
		{ type: 'Rejected', reason: 'NotAllowed' },
		{ type: 'Ended', result: '0-1', termination: 'resign' },
		{ type: 'Offer', kind: 'Rematch', by: 'O', lapses_in: 60_000 },
		{ type: 'OfferAnswered', kind: 'Draw', accepted: false },
		{ type: 'TookBack', to_ply: 2, clocks: null },
		{ type: 'TookBack', to_ply: 2, clocks },
		{ type: 'RematchRoom', room: 'b'.repeat(20) },
		{ type: 'Aborted', reason: 'FirstMoveTimeout' },
		{ type: 'Pong', t: 1_793_600_000_000, server_ms: 1_793_600_000_040 },
		welcome('X', undefined, [], {
			presence: { X: null, O: { since: 1_793_599_990_000, claims_in: 50_000 } }
		}),
		{ type: 'Presence', player: 'O', away: { since: 1_793_600_000_000, claims_in: 60_000 } },
		{ type: 'Presence', player: 'X', away: null },
		{ type: 'Ended', result: '1-0', termination: 'claim' },
		{ type: 'Ended', result: '1/2-1/2', termination: 'claim' },
		{ type: 'Ended', result: null, termination: 'abandoned' }
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
		{ type: 'Moved', ply: 1.5, cell: 40, clocks: null },
		{ type: 'OfferAnswered', kind: 'Draw', accepted: 'no' },
		{ type: 'Ended', result: '2-0', termination: 'resign' },
		{ type: 'Ended', result: '1-0', termination: 'abandoned' },
		{ type: 'Offer', kind: 'Draw', by: 'Z', lapses_in: null },
		{ type: 'OfferAnswered', kind: 'Draw' },
		{ type: 'Offer', kind: 'Draw', by: 'X' },
		{ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: -1 },
		{ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: 1.5 },
		{ type: 'Offer', kind: 'Takeback', by: 'X', lapses_in: 'soon' },
		{ type: 'TookBack', to_ply: -1, clocks: null },
		{ ...welcome('X'), snapshot: { ...snapshot, clock: '4+4' } },
		{ ...welcome('X'), snapshot: { ...snapshot, clocks: { ...clocks, at: undefined } } },
		{ type: 'Moved', ply: 2, cell: 40 },
		{ type: 'Moved', ply: 2, cell: 40, clocks: { ...clocks, x_ms: -1 } },
		{ type: 'Moved', ply: 2, cell: 40, clocks: { ...clocks, running: 'Z' } },
		{ type: 'TookBack', to_ply: 2, clocks: { ...clocks, aborts_at: 'soon' } },
		{ type: 'RematchRoom', room: '../r' },
		{ type: 'Aborted', reason: 'Bored' },
		{ type: 'Aborted' },
		{ type: 'Pong', t: -1, server_ms: 0 },
		{ type: 'Pong', t: 1.5, server_ms: 0 },
		{ type: 'Pong', t: 0 },
		{ type: 'Pong', t: 0, server_ms: 'now' },
		{ ...welcome('X'), snapshot: { ...snapshot, presence: undefined } },
		{
			...welcome('X'),
			snapshot: { ...snapshot, presence: { X: { since: -1, claims_in: 0 }, O: null } }
		},
		{ type: 'Presence', player: 'Z', away: null },
		{ type: 'Presence', player: 'X' },
		{ type: 'Presence', player: 'X', away: { since: 1_793_600_000_000 } },
		{ type: 'Ended', result: null, termination: 'resign' },
		{ type: 'Claim' }
	];
	for (const message of wrong) expect(read(message), JSON.stringify(message)).toBeNull();
});
