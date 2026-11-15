/**
 * Build-time syntax highlighting for the docs' code blocks.
 *
 * The docs only show shell commands and `.env` files, so that is all this
 * knows; any other fence is shown as plain, escaped text. It runs during the
 * build and emits static HTML with a class per token — no JavaScript reaches
 * the browser — and the colors live in tokens.css (light and dark).
 *
 * Token classes: `t-c` comment, `t-fn` command, `t-s` argument or string,
 * `t-f` flag, `t-v` variable, `t-op` operator.
 */

/** Languages with real highlighting, by fence name. */
export const LANGUAGES = {
	sh: 'shell',
	bash: 'shell',
	shell: 'shell',
	zsh: 'shell',
	dotenv: 'dotenv',
	env: 'dotenv'
};

/** Fence names that mean "no highlighting" on purpose. */
export const PLAIN = ['', 'text', 'txt', 'plaintext'];

/**
 * HTML-escape, and turn a backslash into an entity too: the result is spliced into
 * a JavaScript template literal (`{@html `…`}`), where a bare backslash would
 * be eaten as an escape — a shell line continuation would vanish.
 * @param {string} s
 */
function escapeHtml(s) {
	return s
		.replace(/&/g, '&amp;')
		.replace(/\\/g, '&#92;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;')
		.replace(/'/g, '&#39;');
}

/** @typedef {[string | null, string]} Token */

const OPERATOR = '|&;<>()';
const WORD_END = /[\s|&;<>()'"]/;

/** @param {string} code @returns {Token[]} */
function tokenizeShell(code) {
	/** @type {Token[]} */
	const out = [];
	const n = code.length;
	let i = 0;
	let commandStart = true;
	while (i < n) {
		const ch = code[i];
		if (ch === '\n') {
			out.push([null, '\n']);
			i++;
			commandStart = true;
		} else if (ch === ' ' || ch === '\t') {
			let j = i;
			while (j < n && (code[j] === ' ' || code[j] === '\t')) j++;
			out.push([null, code.slice(i, j)]);
			i = j;
		} else if (ch === '#' && (i === 0 || /\s/.test(code[i - 1]))) {
			let j = code.indexOf('\n', i);
			if (j < 0) j = n;
			out.push(['t-c', code.slice(i, j)]);
			i = j;
		} else if (ch === "'" || ch === '"') {
			let j = i + 1;
			while (j < n && code[j] !== ch) j += ch === '"' && code[j] === '\\' ? 2 : 1;
			j = Math.min(j + 1, n);
			out.push(['t-s', code.slice(i, j)]);
			i = j;
			commandStart = false;
		} else if (ch === '\\') {
			if (code[i + 1] === '\n') {
				// A continued line is still the same command.
				out.push(['t-op', '\\'], [null, '\n']);
				i += 2;
			} else {
				out.push([null, code.slice(i, i + 2)]);
				i += 2;
			}
		} else if (ch === '$' && code[i + 1] === '(') {
			out.push(['t-op', '$(']);
			i += 2;
			commandStart = true;
		} else if (ch === '$' && code[i + 1] === '{') {
			let j = code.indexOf('}', i);
			j = j < 0 ? n : j + 1;
			out.push(['t-v', code.slice(i, j)]);
			i = j;
		} else if (ch === '$' && /[A-Za-z_]/.test(code[i + 1] ?? '')) {
			let j = i + 1;
			while (j < n && /[A-Za-z0-9_]/.test(code[j])) j++;
			out.push(['t-v', code.slice(i, j)]);
			i = j;
		} else if (OPERATOR.includes(ch)) {
			let j = i;
			while (j < n && OPERATOR.includes(code[j])) j++;
			const op = code.slice(i, j);
			out.push(['t-op', op]);
			i = j;
			if (/[|&;(]/.test(op)) commandStart = true;
		} else {
			let j = i;
			while (j < n && !WORD_END.test(code[j])) j++;
			if (j === i) j = i + 1;
			const word = code.slice(i, j);
			const assign = /^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/.exec(word);
			if (commandStart && assign) {
				out.push(['t-v', assign[1]], ['t-op', '=']);
				if (assign[2]) out.push(['t-s', assign[2]]);
			} else if (commandStart) {
				out.push(['t-fn', word]);
				commandStart = false;
			} else {
				out.push([word.startsWith('-') ? 't-f' : 't-s', word]);
			}
			i = j;
		}
	}
	return out;
}

/** @param {string} code @returns {Token[]} */
function tokenizeDotenv(code) {
	/** @type {Token[]} */
	const out = [];
	code.split('\n').forEach((line, index) => {
		if (index > 0) out.push([null, '\n']);
		const entry = /^(\s*)(export\s+)?([A-Za-z_][A-Za-z0-9_.]*)(\s*=\s*)(.*)$/.exec(line);
		if (/^\s*#/.test(line)) out.push(['t-c', line]);
		else if (entry) {
			const [, lead, exp, key, eq, value] = entry;
			out.push([null, lead]);
			if (exp) out.push(['t-fn', exp]);
			out.push(['t-v', key], ['t-op', eq]);
			const comment = value.search(/\s#/);
			if (comment >= 0) out.push(['t-s', value.slice(0, comment)], ['t-c', value.slice(comment)]);
			else out.push(['t-s', value]);
		} else out.push([null, line]);
	});
	return out;
}

/**
 * Highlight one fence. Returns `<pre>` HTML; the caller escapes it for Svelte.
 * @param {string} code
 * @param {string} [lang]
 */
export function highlight(code, lang = '') {
	const key = lang.trim().toLowerCase();
	const kind = /** @type {Record<string, string>} */ (LANGUAGES)[key];
	const source = code.replace(/\n+$/, '');
	/** @type {Token[]} */
	const tokens =
		kind === 'shell'
			? tokenizeShell(source)
			: kind === 'dotenv'
				? tokenizeDotenv(source)
				: [[null, source]];

	// One <span class="line"> per line, a token that crosses a newline split.
	/** @type {string[]} */
	const lines = [''];
	for (const [cls, text] of tokens) {
		text.split('\n').forEach((part, k) => {
			if (k > 0) lines.push('');
			if (!part) return;
			const html = escapeHtml(part);
			lines[lines.length - 1] += cls ? `<span class="${cls}">${html}</span>` : html;
		});
	}
	const body = lines.map((l) => `<span class="line">${l}</span>`).join('\n');
	const dataLang = kind ? ` data-lang="${escapeHtml(key)}"` : '';
	return `<pre class="code"${dataLang} tabindex="0"><code>${body}</code></pre>`;
}
