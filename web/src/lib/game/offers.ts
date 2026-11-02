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
	/** When it lapses, in epoch milliseconds, by the server's word, if it does. */
	lapses: number | null;
}

/** How each kind of offer is worded: as this page made it, as it heard it, and by name. */
const OFFERS: Record<OfferKind, { made: string; heard: string; name: string }> = {
	Draw: { made: 'offered a draw', heard: 'offers a draw', name: 'the draw' },
	Takeback: {
		made: 'asked to take back your last move',
		heard: 'asks to take back their last move',
		name: 'the takeback'
	},
	Rematch: { made: 'offered a rematch', heard: 'offers a rematch', name: 'the rematch' }
};

/** What the offer is, for the player who answers it, or for the one who made it. */
export const offerText = ({ kind, mine, other }: PendingOffer): string =>
	mine ? `You ${OFFERS[kind].made}. Waiting for ${other}…` : `${other} ${OFFERS[kind].heard}.`;

/** What became of this page's offer when the other player declined it or let it lapse. */
export const unansweredText = (kind: OfferKind, other: string): string =>
	`${other} didn’t accept ${OFFERS[kind].name}.`;
