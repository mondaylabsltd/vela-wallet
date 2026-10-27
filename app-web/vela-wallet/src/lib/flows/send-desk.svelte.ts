/**
 * The wallet page's send, reachable from outside it (spec 103).
 *
 * The send session lives in the wallet page — it is that page's flow — so an
 * intent that drafts a transfer has no other way to it. The page attaches
 * this port while it is mounted: the same functions its own buttons call
 * (`openSend`, the session's `dispatch`, `closeSend`), and what they show.
 * Nothing about sending is decided here.
 */
import type { SendAlertKind } from '$lib/core/generated/SendAlertKind';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendOpenParams } from '$lib/core/generated/SendOpenParams';
import type { SendView } from '$lib/core/generated/SendView';

export type SendDeskPort = {
	/** The send open now, or `null`. */
	view(): SendView | null;
	/** The core's last refusal on the open send (an alert, not view state). */
	alert(): SendAlertKind | null;
	/** Which send is open: a new number every time one opens. */
	session(): number;
	/** Open the send flow, preselected — what an asset's own Send does. */
	open(prefill: Partial<SendOpenParams>): void;
	dispatch(event: SendEvent): void;
	/** Leave the send (its ✕). */
	close(): void;
};

class SendDesk {
	/** The wallet page's port while it is mounted. */
	port = $state<SendDeskPort | null>(null);
	#sessions = 0;

	/**
	 * A number for a send about to open — the app's, not the page's, so a
	 * page mounted again never hands out a number an earlier send had.
	 */
	begin(): number {
		this.#sessions += 1;
		return this.#sessions;
	}

	/** Attach the page's port; returns the detach for its unmount. */
	attach(port: SendDeskPort): () => void {
		this.port = port;
		return () => {
			if (this.port === port) this.port = null;
		};
	}
}

export const sendDesk = new SendDesk();
