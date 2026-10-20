/**
 * The words of the agent's layer over every page (spec 103): the request the
 * person approves, and the agent's bar. Resolved on the server, like every
 * other page's copy.
 */
export interface AgentMessages {
	title: string;
	connected: string;
	approveTitle: string;
	approveHint: string;
	allow: string;
	deny: string;
	stop: string;
	/** What each action an agent may ask for is called, by its `type`. */
	actions: Record<string, string>;
}
