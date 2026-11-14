<script lang="ts">
	/**
	 * Light/dark switch. The resolved theme is a CSS concern (`color-scheme` +
	 * `data-theme` on <html>, see tokens.css), so the icon is chosen entirely in
	 * CSS below — this renders correctly during SSR without knowing the theme.
	 * Clicking stores an explicit choice that the inline script in app.html
	 * re-applies before paint on the next visit.
	 */
	function toggle() {
		const root = document.documentElement;
		const isDark =
			root.dataset.theme === 'dark' ||
			(root.dataset.theme !== 'light' && window.matchMedia('(prefers-color-scheme: dark)').matches);
		const next = isDark ? 'light' : 'dark';
		root.dataset.theme = next;
		// The theme-color metas in app.html switch on the OS scheme only; once a
		// theme is pinned, point both at the pinned background so mobile browser
		// chrome matches the page.
		const bg = next === 'dark' ? '#21201c' : '#faf9f5';
		for (const meta of document.querySelectorAll('meta[name="theme-color"]')) {
			meta.setAttribute('content', bg);
		}
		try {
			localStorage.setItem('vela-theme', next);
		} catch {
			/* storage unavailable — theme still applies for this page */
		}
	}
</script>

<button class="theme-toggle" onclick={toggle} aria-label="Toggle color theme" title="Toggle theme">
	<!-- moon: shown in light mode (click → dark) -->
	<svg
		class="moon"
		width="17"
		height="17"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
	</svg>
	<!-- sun: shown in dark mode (click → light) -->
	<svg
		class="sun"
		width="17"
		height="17"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="4" />
		<path
			d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32 1.41 1.41M2 12h2m16 0h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.66 1.41"
		/>
	</svg>
</button>

<style>
	.theme-toggle {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 34px;
		height: 34px;
		border: none;
		border-radius: 50%;
		background: transparent;
		color: var(--text-secondary);
		cursor: pointer;
		transition:
			color 0.15s ease,
			background 0.15s ease;
	}
	.theme-toggle:hover {
		color: var(--text);
		background: var(--accent-soft);
	}

	/* Icon choice mirrors the resolved theme (same cascade as tokens.css). */
	.sun {
		display: none;
	}
	:global([data-theme='dark']) .sun {
		display: block;
	}
	:global([data-theme='dark']) .moon {
		display: none;
	}
	@media (prefers-color-scheme: dark) {
		:global(:root:not([data-theme='light'])) .sun {
			display: block;
		}
		:global(:root:not([data-theme='light'])) .moon {
			display: none;
		}
	}
</style>
