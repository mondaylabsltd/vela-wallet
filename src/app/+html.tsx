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
        <ScrollViewStyleReset />
        <script
          src="https://tj.appsdata.org/api/script.js"
          data-site-id="d9a1055d13df"
          defer
      ></script>
      </head>
      <body>{children}</body>
    </html>
  );
}
