/**
 * An offer waiting for an answer in a Room (spec §10.2): a draw, a takeback or a rematch, as the
 * game screen shows it and its live line says it. Engine-free.
 */
import type { OfferKind } from '#lib/engine.js';

export interface PendingOffer {
	kind: OfferKind;
	/** This page made it, and waits for the other player's answer. */
	mine: boolean;
	/** The other player's Handle. */
	other: string;
	/** When it lapses, in epoch milliseconds, if it does. */
	lapses: number | null;
}

/** The draw offers each player has in a game, the server's `Draws::MAX`. */
export const DRAWS = 3;

/**
 * Each kind of offer: how it's worded, and how long it waits before it lapses, the server's
 * `Timing`, counted from when this page heard it, so a little longer than the server's. A draw
 * waits to be answered.
 */
export const OFFERS: Record<
	OfferKind,
	{ made: string; heard: string; name: string; lapses: number | null }
> = {
	Draw: { made: 'offered a draw', heard: 'offers a draw', name: 'the draw', lapses: null },
	Takeback: {
		made: 'asked to take back your last move',
		heard: 'asks to take back their last move',
		name: 'the takeback',
		lapses: 15_000
	},
	Rematch: {
		made: 'offered a rematch',
		heard: 'offers a rematch',
		name: 'the rematch',
		lapses: 60_000
	}
};

/** What the offer is, for the player who answers it, or for the one who made it. */
export const offerText = ({ kind, mine, other }: PendingOffer): string =>
	mine ? `You ${OFFERS[kind].made}. Waiting for ${other}…` : `${other} ${OFFERS[kind].heard}.`;

/** What became of this page's offer when the other player declined it or let it lapse. */
export const unansweredText = (kind: OfferKind, other: string): string =>
	`${other} didn’t accept ${OFFERS[kind].name}.`;
