/**
 * Constructs the `contacts` core and wires it to the web shell (spec 024).
 *
 * The factory. The app holds ONE session, `contacts-book.svelte.ts`, read at
 * sign-in and shared by the contacts page and the send picker (founder,
 * 2026-09-26: a session per visit made 通讯录 flash unread on every visit).
 * Research D8's route scope is retired; build through the book.
 */

import { ContactsCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { ContactEvent } from '$lib/core/generated/ContactEvent';
import type { ContactShellResult } from '$lib/core/generated/ContactShellResult';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import { contactOperationFailure, executeContactOperation } from './contacts-executor';
import type { ContactEffect, ContactsSessionOptions } from './contacts-types';

export type ContactsSession = EffectLoop<ContactEvent>;

/** Callers `loadCore()` first (route onMount) — construction is synchronous. */
export function createContactsSession(options: ContactsSessionOptions): ContactsSession {
	return createJsonWasmShell<ContactsView, ContactEvent, ContactEffect, ContactShellResult>(
		new ContactsCore(),
		{
			onView: options.onView,
			execute: executeContactOperation,
			toFailure: contactOperationFailure,
			onError: options.onError
		}
	);
}
