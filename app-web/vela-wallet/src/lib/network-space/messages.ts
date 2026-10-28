/**
 * The words of the spaces (spec 105): the header label, the testnet bar, the
 * switch, the add-network wizard's question and the network list's
 * headings. Resolved on the server, like every other page's copy.
 */
export interface NetworkSpaceMessages {
	mainnet: string;
	testnet: string;
	bar: string;
	title: string;
	mainnetHint: string;
	testnetHint: string;
	toTestnet: string;
	toMainnet: string;
	noValue: string;
	moveToMainnet: string;
	moveToTestnet: string;
	wizardQuestion: string;
	wizardTestnet: string;
	wizardMainnet: string;
	headingMainnet: string;
	headingTestnet: string;
	/**
	 * set-5: what an add says once it has landed, and where — templates with
	 * `{{name}}`. The wizard empties itself after an add; this is its answer.
	 */
	addedTestnet: string;
	addedMainnet: string;
	cancel: string;
}
