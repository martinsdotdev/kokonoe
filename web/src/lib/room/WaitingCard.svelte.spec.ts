import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import WaitingCard from './WaitingCard.svelte';

const LINK = 'https://kokonoe.app/r/6f1c0a9b3e2d4c5a7b8e';

afterEach(() => vi.restoreAllMocks());

describe('the host’s wait', () => {
	it('copies the link, and says so', async () => {
		const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
		const screen = render(WaitingCard, {
			link: LINK,
			variant: 'Standard',
			you: 'X',
			trouble: null
		});
		await screen.getByRole('button', { name: 'Copy link' }).click();
		expect(writeText).toHaveBeenCalledWith(LINK);
		await expect.element(screen.getByRole('status')).toHaveTextContent('Link copied.');
	});

	it('selects the link to copy by hand when the clipboard is refused', async () => {
		vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('Not allowed'));
		const screen = render(WaitingCard, {
			link: LINK,
			variant: 'Standard',
			you: 'X',
			trouble: null
		});
		await screen.getByRole('button', { name: 'Copy link' }).click();
		await expect.element(screen.getByRole('status')).toHaveTextContent(/copy it with Ctrl\+C/);
		const field = screen.getByLabelText('Invite link').element() as HTMLInputElement;
		expect([field.selectionStart, field.selectionEnd]).toEqual([0, LINK.length]);
	});

	it('says when the connection is down', async () => {
		const screen = render(WaitingCard, {
			link: LINK,
			variant: 'Standard',
			you: 'O',
			trouble: 'Connection lost. Reconnecting…'
		});
		await expect
			.element(screen.getByRole('status'))
			.toHaveTextContent('Connection lost. Reconnecting…');
		await expect.element(screen.getByText('Standard · You play O')).toBeVisible();
	});
});
