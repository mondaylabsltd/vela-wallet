// Ported from src/services/wallet-state-core/contacts-types.ts @ e78afdfa
// (spec 024). One module per machine; the generic options live in #lib/core.

import type { ContactOperation } from '#lib/core/generated/ContactOperation.ts';
import type { ContactsView } from '#lib/core/generated/ContactsView.ts';
import type { SessionOptions } from '#lib/core/types.ts';

/** One request from the core, carrying the id it will be answered by. */
export type ContactEffect = { id: number; operation: ContactOperation };

export type ContactsSessionOptions = SessionOptions<ContactsView>;
