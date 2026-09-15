<!--
	Hero visual: a face that turns into a key.
	An invisible pen draws the Face ID frame and a minimal face, then the line
	drops out of the face and loops into a key — pupils and key teeth are the
	only orange. One 14s cycle: draw → hold → fade. Pure CSS/SVG, colors come
	from the global theme tokens, so both themes work automatically.
-->
<section class="stage" aria-label="Animated illustration: a face drawn in one line, which becomes a key">
	<svg class="art" viewBox="0 0 400 500" role="img" aria-hidden="true">
		<!-- ambient dashed ring, persists across cycles -->
		<circle class="ring anim" cx="200" cy="255" r="175" pathLength="1" />

		<g class="scene anim">
			<!-- 01 · Face ID brackets, drawn corner by corner -->
			<path
				class="draw frame anim"
				pathLength="1"
				d="M120,122 V104 Q120,90 134,90 H152
				   M248,90 H266 Q280,90 280,104 V122
				   M280,198 V216 Q280,230 266,230 H248
				   M152,230 H134 Q120,230 120,216 V198"
			/>

			<!-- minimal features: nose, then smile -->
			<path
				class="draw face anim"
				pathLength="1"
				d="M203,138 V164 Q203,174 192,174
				   M170,196 Q200,212 230,196"
			/>

			<!-- pupils, the only orange in the face -->
			<circle class="eye anim" cx="166" cy="144" r="5" />
			<circle class="eye anim" cx="234" cy="144" r="5" />

			<!-- the line drops out of the face… -->
			<path class="draw thread anim" pathLength="1" d="M200,238 V270" />

			<!-- …and loops into a key: bow -->
			<path
				class="draw bow anim"
				pathLength="1"
				d="M200,270 a28,28 0 1,1 0,56 a28,28 0 1,1 0,-56"
			/>

			<!-- shaft -->
			<path class="draw shaft anim" pathLength="1" d="M200,326 V430" />

			<!-- teeth, orange -->
			<path class="draw teeth anim" pathLength="1" d="M200,402 H228 M200,424 H236" />

			<!-- editorial annotations -->
			<text class="label label-scan anim" x="120" y="72">01 · scan</text>
			<text class="label label-sign anim" x="246" y="302">02 · sign</text>
		</g>
	</svg>
</section>

<style>
	.stage {
		position: relative;
		width: 100%;
		max-width: 440px;
		aspect-ratio: 4 / 5;
		display: grid;
		place-items: center;
		margin-inline: auto;
		background: radial-gradient(
			60% 48% at 50% 46%,
			color-mix(in srgb, var(--accent) 5%, transparent),
			transparent 78%
		);
	}
	/* On small screens the visual sits above the headline — keep it compact
	   so the copy stays near the fold. */
	@media (max-width: 900px) {
		.stage {
			max-width: 300px;
		}
	}

	.art {
		display: block;
		width: 100%;
		height: 100%;
		color: var(--text);
	}

	/* ambient ring (outside the fading scene, keeps the loop grounded) */
	.ring {
		fill: none;
		stroke: var(--border);
		stroke-width: 1.25;
		stroke-linecap: round;
		stroke-dasharray: 0.004 0.018;
		transform-box: fill-box;
		transform-origin: center;
		animation: spin 90s linear infinite;
	}

	/* shared pen-drawing setup: base state = fully drawn (reduced-motion state) */
	.draw {
		fill: none;
		stroke-linecap: round;
		stroke-linejoin: round;
		stroke-dasharray: 1;
		stroke-dashoffset: 0;
		opacity: 1;
		animation-duration: 14s;
		animation-timing-function: ease-in-out;
		animation-iteration-count: infinite;
	}

	.scene {
		opacity: 1;
		animation: scene 14s ease-in-out infinite;
	}

	.frame {
		stroke: var(--text-secondary);
		stroke-width: 2;
		animation-name: d-frame;
	}
	.face {
		stroke: var(--text);
		stroke-width: 2;
		animation-name: d-face;
	}
	.thread {
		stroke: var(--text-muted);
		stroke-width: 1.5;
		animation-name: d-thread;
	}
	.bow {
		stroke: var(--text);
		stroke-width: 2;
		animation-name: d-bow;
	}
	.shaft {
		stroke: var(--text);
		stroke-width: 2;
		animation-name: d-shaft;
	}
	.teeth {
		stroke: var(--accent);
		stroke-width: 2;
		animation-name: d-teeth;
	}

	.eye {
		fill: var(--accent);
		stroke: none;
		opacity: 1;
		animation: eyes 14s ease-in-out infinite;
	}

	.label {
		fill: var(--text-muted);
		font-family: var(--font-mono);
		font-size: 10.5px;
		letter-spacing: 0.12em;
		opacity: 1;
		animation-duration: 14s;
		animation-timing-function: ease-in-out;
		animation-iteration-count: infinite;
	}
	.label-scan {
		animation-name: lab-scan;
	}
	.label-sign {
		animation-name: lab-sign;
	}

	/* timeline (14s): frame → face → pupils → thread → bow → shaft → teeth → hold → fade */

	@keyframes d-frame {
		0%,
		2% {
			stroke-dashoffset: 1;
			opacity: 0;
		}
		5% {
			opacity: 1;
		}
		20%,
		100% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
	}

	@keyframes d-face {
		0%,
		22% {
			stroke-dashoffset: 1;
			opacity: 0;
		}
		25% {
			opacity: 1;
		}
		38%,
		100% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
	}

	@keyframes eyes {
		0%,
		38% {
			opacity: 0;
		}
		44%,
		100% {
			opacity: 1;
		}
	}

	@keyframes d-thread {
		0%,
		47% {
			stroke-dashoffset: 1;
			opacity: 0;
		}
		49% {
			opacity: 1;
		}
		53%,
		100% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
	}

	@keyframes d-bow {
		0%,
		53% {
			stroke-dashoffset: 1;
			opacity: 0;
		}
		56% {
			opacity: 1;
		}
		64%,
		100% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
	}

	@keyframes d-shaft {
		0%,
		64% {
			stroke-dashoffset: 1;
			opacity: 0;
		}
		66% {
			opacity: 1;
		}
		71%,
		100% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
	}

	@keyframes d-teeth {
		0%,
		71% {
			stroke-dashoffset: 1;
			opacity: 0;
		}
		73% {
			opacity: 1;
		}
		78%,
		100% {
			stroke-dashoffset: 0;
			opacity: 1;
		}
	}

	@keyframes lab-scan {
		0%,
		20% {
			opacity: 0;
		}
		26%,
		100% {
			opacity: 1;
		}
	}

	@keyframes lab-sign {
		0%,
		78% {
			opacity: 0;
		}
		83%,
		100% {
			opacity: 1;
		}
	}

	@keyframes scene {
		0%,
		91% {
			opacity: 1;
		}
		98%,
		100% {
			opacity: 0;
		}
	}

	@keyframes spin {
		0% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(360deg);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.anim {
			animation: none !important;
		}
	}
</style>
