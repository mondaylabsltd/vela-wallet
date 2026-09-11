<script lang="ts">
	/**
	 * In-app brand mark (docs/design-system.md §Brand): transparent sailboat, hull
	 * color follows the UI mode, sails from the mode's asset. Never boxed on
	 * the app-icon background. Colors below are ASSET content taken verbatim
	 * from docs/design/onboarding/logo-{light,dark}.svg (audit-whitelisted).
	 */
	interface Props {
		/** Rendered box in px (SVG is square-ish, 258×260). */
		size?: number;
	}

	let { size = 56 }: Props = $props();
</script>

<svg
	viewBox="0 0 258 260"
	width={size}
	height={size * (260 / 258)}
	fill="none"
	aria-hidden="true"
	class="mark"
>
	<path d="M122,0C70,53,38,118,18,187L122,187L122,0Z" class="sail-main" />
	<path d="M142,42C193,75,225,128,240,187L142,187L142,42Z" class="sail-secondary" />
	<path d="M0,207L258,207C243,240,211,260,165,260L92,260C49,260,16,240,0,207Z" class="hull" />
</svg>

<style>
	/* Dark-UI asset (base, matching the dark-default token layer). */
	.sail-main {
		fill: #ff6a45;
	}
	.sail-secondary {
		fill: #ffa98e;
	}
	.hull {
		fill: #ded5ce; /* Dusk Ivory */
	}

	@media (prefers-color-scheme: light) {
		:global(:root:not([data-theme='dark'])) .mark .sail-main {
			fill: #ff5a36;
		}
		:global(:root:not([data-theme='dark'])) .mark .hull {
			fill: #554b46; /* Warm Graphite */
		}
	}
	:global(:root[data-theme='light']) .mark .sail-main {
		fill: #ff5a36;
	}
	:global(:root[data-theme='light']) .mark .hull {
		fill: #554b46;
	}
</style>
