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
    name: 'Vela Wallet for Safe',
    short_name: 'Vela Safe',
    description: 'Connect an existing Vela Safe to Safe Wallet and approve transactions with its passkey.',
    icons: {
      16: 'icon/16.png',
      32: 'icon/32.png',
      48: 'icon/48.png',
      128: 'icon/128.png',
    },
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
