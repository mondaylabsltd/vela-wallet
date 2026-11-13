/**
 * The words of the agent's layer over every page (spec 103): the request the
 * person approves, and the agent's bar. Resolved on the server, like every
 * other page's copy.
 */
export interface AgentMessages {
	title: string;
	connected: string;
	/** "In use in another tab": another tab of this wallet answers the agent. */
	elsewhere: string;
	/** Takes the agent back into this tab. */
	useHere: string;
	approveTitle: string;
	approveHint: string;
	/**
	 * The request as one sentence, in the locale's punctuation:
	 * `{{action}} — {{detail}}. {{hint}}`, or `{{action}}. {{hint}}`.
	 */
	approveLine: string;
	approveAction: string;
	/** `{{label}}: {{value}}`, with the locale's colon. */
	pair: string;
	allow: string;
	deny: string;
	stop: string;
	/** `set_balance_privacy`'s two ends, as the home's eye says them. */
	showBalance: string;
	hideBalance: string;
	/** A switch's two ends (analytics). */
	on: string;
	off: string;
	/** A network the wallet does not know by name: `Network {{id}}`. */
	chain: string;
	/** What each action an agent may ask for is called, by its `type`. */
	actions: Record<string, string>;
	/** Each preference by its `kind`, as Settings names it. */
	preferences: Record<string, string>;
	/**
	 * A preference's value as Settings says it, by `kind.value`
	 * (`theme.dark`, `fee_speed.fast`, `time_format.h24`, `language.auto`…).
	 */
	values: Record<string, string>;
	/** Each storage item by its id, as Settings → Device storage names it. */
	storage: Record<string, string>;
	/** Each service endpoint by its field. */
	services: Record<string, string>;
	/** A network's two addresses by their field (`rpc`, `explorer`). */
	fields: Record<string, string>;
	/** "Only use my node", as the network's switch in Settings names it. */
	onlyMine: string;
}
