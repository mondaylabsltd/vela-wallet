/**
 * The notes registry (spec 101): every part and module, in reading order.
 *
 * This file is the only place that orders modules and names them. A note's
 * module is its folder under `src/content/notes/`, and a folder that is not
 * listed here fails `notes.test.ts`, so a typo can't quietly become a
 * twenty-fourth module. A module with no published notes is simply not shown.
 */

export interface NoteModule {
	slug: string;
	title: string;
	/** One line under the module's heading, on the index and the module page. */
	intro: string;
}

export interface NotePart {
	key: string;
	title: string;
	modules: NoteModule[];
}

export const NOTE_PARTS: NotePart[] = [
	{
		key: 'how',
		title: 'How Vela works',
		modules: [
			{
				slug: 'how-it-works',
				title: 'The basics',
				intro:
					'The core mechanics in plain words: your address, your keys, what you sign, and what happens when you send.'
			}
		]
	},
	{
		key: 'using',
		title: 'Using Vela well',
		modules: [
			{
				slug: 'settings',
				title: 'Settings',
				intro: "What each setting does, what it's set to out of the box, and when you'd change it."
			},
			{
				slug: 'paying-less',
				title: 'Paying less',
				intro: 'Where the fee comes from, and the choices that actually change it.'
			},
			{
				slug: 'faster',
				title: 'Landing faster',
				intro: 'What decides how quickly a transaction lands, and what you can do about it.'
			},
			{
				slug: 'security-layers',
				title: 'Security, layer by layer',
				intro:
					"Protection you can add one layer at a time, and what each layer does and doesn't cover."
			}
		]
	},
	{
		key: 'why',
		title: "Why it's like this",
		modules: [
			{
				slug: 'principles',
				title: 'Principles',
				intro: 'What Vela promises, what it refuses to promise, and why the wording matters.'
			},
			{
				slug: 'privacy',
				title: 'Privacy',
				intro: 'What Vela can see, what it publishes on purpose, and what it never sends.'
			}
		]
	},
	{
		key: 'wallet',
		title: 'Your wallet',
		modules: [
			{
				slug: 'keys',
				title: 'Keys',
				intro: 'How the keys that control a wallet are made, checked and shown.'
			},
			{
				slug: 'address',
				title: 'Your address',
				intro: 'Your address, the Safe contract behind it, and why neither ever moves.'
			},
			{
				slug: 'sign-in',
				title: 'Signing in and the registry',
				intro:
					'Signing in on a new device, the public registry, and what signing out really removes.'
			},
			{
				slug: 'security-keys',
				title: 'Security keys and phones',
				intro:
					'Hardware security keys and scanning with a phone, and the platform quirks each one hit.'
			}
		]
	},
	{
		key: 'money',
		title: 'Moving money',
		modules: [
			{
				slug: 'fees',
				title: 'Fees',
				intro: 'What a transaction costs, how that number is made, and the guards around it.'
			},
			{
				slug: 'speed',
				title: 'Speed',
				intro: 'The speed setting: what it buys, and when it buys nothing.'
			},
			{
				slug: 'relay',
				title: 'The relay',
				intro:
					"The service that submits your transactions, what it can and can't do to them, and when it went wrong."
			},
			{
				slug: 'signing',
				title: 'Signing',
				intro: 'The confirm screen, what it refuses to show you, and what it refuses to sign.'
			},
			{
				slug: 'trusted-signer',
				title: 'The Trusted Signer',
				intro:
					'The optional signing page at sign.getvela.app, and the trade-off behind each of its rules.'
			},
			{
				slug: 'send-receive',
				title: 'Sending and receiving',
				intro:
					'Sending to the right address, receiving on the right network, and paying many people at once.'
			},
			{
				slug: 'transactions',
				title: 'After you press send',
				intro:
					'What happens after you press send, and why the wallet would rather say “unknown” than guess.'
			},
			{
				slug: 'balances',
				title: 'Balances and numbers',
				intro: 'Prices, totals, and how numbers are typed and shown.'
			}
		]
	},
	{
		key: 'world',
		title: 'Out in the world',
		modules: [
			{
				slug: 'networks',
				title: 'Networks',
				intro: 'Which networks Vela runs on, what each one needed, and how the wallet reads them.'
			},
			{
				slug: 'dapps',
				title: 'dApps',
				intro: 'How a website talks to Vela, and the rules every request goes through.'
			},
			{
				slug: 'browser',
				title: 'The built-in browser',
				intro: 'The browser inside the desktop and phone apps.'
			}
		]
	},
	{
		key: 'interface',
		title: 'The interface',
		modules: [
			{
				slug: 'interface',
				title: 'Interface details',
				intro: 'Small parts of the interface, and why each one looks or behaves the way it does.'
			},
			{
				slug: 'language',
				title: 'Language and readability',
				intro: 'Fifteen languages, six text sizes, and the bugs that only showed up in one of them.'
			}
		]
	},
	{
		key: 'behind',
		title: 'Behind it',
		modules: [
			{
				slug: 'self-hosting',
				title: 'Running it yourself',
				intro: "Running Vela's services yourself, and the one thing you can't move."
			},
			{
				slug: 'engineering',
				title: "How it's built",
				intro: 'One Rust core under four apps, and how it got that way.'
			},
			{
				slug: 'practice',
				title: 'How the work is done',
				intro: 'How one person keeps four apps telling the same truth.'
			},
			{
				slug: 'releases',
				title: 'Releases and downloads',
				intro: 'How a release is built, signed and downloaded.'
			}
		]
	}
];

/** Every module, in reading order. */
export const NOTE_MODULES: NoteModule[] = NOTE_PARTS.flatMap((part) => part.modules);

export function getNoteModule(slug: string): NoteModule | undefined {
	return NOTE_MODULES.find((m) => m.slug === slug);
}
