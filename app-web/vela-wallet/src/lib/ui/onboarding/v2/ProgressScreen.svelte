<script lang="ts">
	/**
	 * Deriving the address.
	 *
	 * Three task rows, driven by the stage the core reported — never by elapsed
	 * time. A bar that advances on a timer tells the person something the
	 * wallet does not know, and the one moment they are most owed the truth is
	 * while their key set is being frozen. This is why spec 014's
	 * elapsed-seconds ring is gone from the create flow.
	 *
	 * The percentage meter that used to head the rows is gone too (founder
	 * call, 2026-08-25, and the desktop had already reached the same
	 * conclusion): it LOOKED measured and was not — the same three statuses the
	 * rows below name, divided by three — and its label named one phase while
	 * another was running.
	 *
	 * The running row SPINS. Every one of these three waits on something
	 * outside the app (a passkey prompt, a derivation, a network write), and a
	 * still dot says nothing about whether anything is still happening.
	 */
	import { PROGRESS_TASKS, type ProgressPosition } from '$lib/onboarding/core/copy';

	interface Props {
		position: ProgressPosition;
		keyCount: number;
		strings: (key: string, params?: Record<string, string | number>) => string;
	}

	let { position, keyCount, strings }: Props = $props();

	function state(index: number): 'done' | 'active' | 'pending' {
		if (index < position.activeTask) return 'done';
		return index === position.activeTask ? 'active' : 'pending';
	}
</script>

<section class="screen">
	<header class="intro">
		<h1 class="title">{strings('onboarding.create.progressTitle')}</h1>
		<p class="subtitle">
			{strings('onboarding.create.progressSubtitle', { count: keyCount })}
		</p>
	</header>

	<ul class="tasks">
		{#each PROGRESS_TASKS as task, index (task)}
			<li class="task" data-state={state(index)}>
				{#if state(index) === 'active'}
					<span class="spinner" aria-hidden="true"></span>
				{:else}
					<span class="mark" aria-hidden="true">{state(index) === 'done' ? '✓' : '○'}</span>
				{/if}
				<span class="what">{strings(task)}</span>
			</li>
		{/each}
	</ul>
</section>

<style>
	.screen {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-4xl);
	}

	.intro {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.015em;
	}

	.subtitle {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.tasks {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.task {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
		padding-block: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
	}

	.mark {
		flex: 0 0 var(--icon-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	/* The running row's mark: an ink arc turning in place. Sized to the same
	   column the ✓ and ○ occupy, so the three rows never shift. */
	.spinner {
		flex: 0 0 var(--icon-base);
		width: var(--icon-base);
		height: var(--icon-base);
		border: var(--border-emphasis) solid var(--color-border-base);
		border-top-color: var(--color-fg-base);
		border-radius: var(--radius-full);
		/* Two of the slow token: 400ms is a transition length, and a full
		   revolution that fast reads as a blur rather than as waiting. */
		animation: spin calc(var(--motion-duration-slow) * 2) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.task[data-state='done'] {
		color: var(--color-fg-muted);
	}

	.task[data-state='done'] .mark {
		color: var(--color-success-base);
	}

	.task[data-state='active'] {
		color: var(--color-fg-base);
	}

	.task[data-state='active'] .mark {
		color: var(--color-fg-base);
	}

	.task[data-state='pending'] {
		color: var(--color-fg-muted);
	}

	.task[data-state='pending'] .mark {
		color: var(--color-fg-subtle);
	}

	@media (prefers-reduced-motion: reduce) {
		/* Still, but still in ink: the mark keeps saying WHICH row is running. */
		.spinner {
			animation: none;
		}
	}
</style>
