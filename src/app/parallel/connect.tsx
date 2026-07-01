/**
 * `/parallel/connect` — the real Connect screen, running in the parallel space.
 *
 * This is the actual production ConnectScreen (same transport, provider, signing
 * sheet). The only thing different from real space is that approving a request signs
 * with the fixture passkey. Paste the connect URL printed by the local test relay
 * (see e2e/support/relay.mjs) to drive it.
 */
import ConnectScreen from '@/screens/connect/ConnectScreen';

export default ConnectScreen;
