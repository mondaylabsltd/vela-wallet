import { ScrollViewStyleReset } from 'expo-router/html';
import type { PropsWithChildren } from 'react';

export default function Root({ children }: PropsWithChildren) {
  return (
    <html lang="en">
      <head>
        <meta charSet="utf-8" />
        <meta httpEquiv="X-UA-Compatible" content="IE=edge" />
        <meta name="viewport" content="width=device-width, initial-scale=1, shrink-to-fit=no" />
        {/* Safari address bar + status bar color — light mode */}
        <meta name="theme-color" content="#FAFAF8" media="(prefers-color-scheme: light)" />
        {/* Safari address bar + status bar color — dark mode */}
        <meta name="theme-color" content="#141412" media="(prefers-color-scheme: dark)" />
        {/* Open Graph / WeChat / Facebook */}
        <meta property="og:type" content="website" />
        <meta property="og:url" content="https://getvela.app" />
        <meta property="og:title" content="Vela Wallet — No seed phrases. Just your fingerprint." />
        <meta
          property="og:description"
          content="Self-custodial Ethereum wallet secured by passkeys, built on audited Safe smart contracts. Open source."
        />
        <meta property="og:image" content="https://getvela.app/og-image.png" />
        <meta property="og:image:width" content="1024" />
        <meta property="og:image:height" content="1024" />
        {/* Twitter / X */}
        <meta name="twitter:card" content="summary" />
        <meta name="twitter:title" content="Vela Wallet — No seed phrases. Just your fingerprint." />
        <meta
          name="twitter:description"
          content="Self-custodial Ethereum wallet secured by passkeys, built on audited Safe smart contracts. Open source."
        />
        <meta name="twitter:image" content="https://getvela.app/og-image.png" />
        <ScrollViewStyleReset />
   <script
    src="https://tj.appsdata.org/api/script.js"
    data-site-id="51bb55d72d55"
    defer
></script>
      </head>
      <body>{children}</body>
    </html>
  );
}
