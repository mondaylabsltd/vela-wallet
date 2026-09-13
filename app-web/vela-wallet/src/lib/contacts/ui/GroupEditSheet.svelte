<script lang="ts">
	/**
	 * The new/rename group form as the PHONE draws it (spec 024): the shared
	 * body inside a bottom sheet. The desktop puts it in a centred dialog.
	 */
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import type { GroupFormCopy } from '../forms';
	import GroupForm from './GroupForm.svelte';

	interface Props {
		copy: GroupFormCopy;
		initialName?: string;
		onsave: (name: string) => void;
		onclose: () => void;
	}

	let { copy, initialName, onsave, onclose }: Props = $props();
</script>

<BottomSheet title={copy.title} closeLabel={copy.cancel} {onclose}>
	<GroupForm {copy} {initialName} {onsave} />
</BottomSheet>
