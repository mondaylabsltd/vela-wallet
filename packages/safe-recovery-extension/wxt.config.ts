import { defineConfig } from 'wxt';

const rpcHosts = [
  'https://eth.llamarpc.com/*',
  'https://mainnet.optimism.io/*',
  'https://bsc-dataseed.binance.org/*',
  'https://rpc.gnosis.gateway.fm/*',
  'https://polygon.drpc.org/*',
  'https://mainnet.era.zksync.io/*',
  'https://mainnet.base.org/*',
  'https://arb1.arbitrum.io/*',
  'https://api.avax.network/*',
  'https://rpc.linea.build/*',
  'https://rpc.scroll.io/*',
];

export default defineConfig({
  srcDir: 'src',
  manifest: {
    name: 'Vela Safe Recovery',
    short_name: 'Vela Recovery',
    description: 'Control a Vela Safe with its getvela.app passkey, without any Vela service.',
    permissions: ['storage', 'windows'],
    host_permissions: [
      'https://app.safe.global/*',
      'https://getvela.app/*',
      ...rpcHosts,
    ],
    optional_host_permissions: [
      'https://*/*',
      'http://localhost/*',
      'http://127.0.0.1/*',
    ],
  },
});
