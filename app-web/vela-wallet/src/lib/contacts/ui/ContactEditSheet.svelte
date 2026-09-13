<script lang="ts">
	/**
	 * The add/edit contact form as the PHONE draws it (spec 024): the shared
	 * body inside a bottom sheet. The desktop puts the same body in its third
	 * column (`ContactsDesktop`'s `contactForm`). The 018 boards drew the
	 * book's states but never a form; flagged in the delivery report as a
	 * Penpot catalog gap to backfill.
	 */
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import type { ContactDraft, ContactFormCopy } from '../forms';
	import ContactForm from './ContactForm.svelte';

	interface Props {
		copy: ContactFormCopy;
		/** Present when editing — the address is identity and stays fixed. */
		initial?: ContactDraft;
		onsave: (draft: ContactDraft) => void;
		onclose: () => void;
	}

	let { copy, initial, onsave, onclose }: Props = $props();
</script>

<BottomSheet title={copy.title} closeLabel={copy.cancel} {onclose}>
	<ContactForm {copy} {initial} {onsave} />
</BottomSheet>
