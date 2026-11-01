import { expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import OfferCard from './OfferCard.svelte';

const offer = { kind: 'Draw', mine: false, other: 'guest-1066', lapses: null } as const;

it('asks for an answer to the other player’s offer', async () => {
	const onanswer = vi.fn();
	const screen = await render(OfferCard, { offer, onanswer });
	await expect.element(screen.getByText('guest-1066 offers a draw.')).toBeVisible();
	await screen.getByRole('button', { name: 'Accept' }).click();
	await screen.getByRole('button', { name: 'Decline' }).click();
	expect(onanswer.mock.calls).toEqual([[true], [false]]);
});

it('waits on this page’s own offer, with nothing to answer', async () => {
	const screen = await render(OfferCard, { offer: { ...offer, mine: true }, onanswer: vi.fn() });
	await expect
		.element(screen.getByText('You offered a draw. Waiting for guest-1066…'))
		.toBeVisible();
	expect(screen.getByRole('button').all()).toHaveLength(0);
});

it('counts down a takeback’s seconds', async () => {
	const lapses = Date.now() + 1500;
	const takeback = { ...offer, kind: 'Takeback', lapses } as const;
	const screen = await render(OfferCard, { offer: takeback });
	await expect
		.element(screen.getByText('guest-1066 asks to take back their last move.'))
		.toBeVisible();
	await expect
		.element(screen.getByText('1 second left to answer'), { timeout: 2000 })
		.toBeVisible();
});

it('shows no countdown for a rematch, which waits a minute', async () => {
	const rematch = { ...offer, kind: 'Rematch', lapses: Date.now() + 60_000 } as const;
	const screen = await render(OfferCard, { offer: rematch });
	await expect.element(screen.getByText('guest-1066 offers a rematch.')).toBeVisible();
	expect(screen.container.querySelector('[data-left]')).toBeNull();
});

it('says a lapsed takeback has no time left', async () => {
	const takeback = { ...offer, kind: 'Takeback', lapses: Date.now() - 1000 } as const;
	const screen = await render(OfferCard, { offer: takeback });
	await expect.element(screen.getByText('0 seconds left to answer')).toBeVisible();
});

it('has no buttons without a way to answer', async () => {
	const screen = await render(OfferCard, { offer });
	await expect.element(screen.getByText('guest-1066 offers a draw.')).toBeVisible();
	expect(screen.getByRole('button').all()).toHaveLength(0);
});
