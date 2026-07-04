/**
 * Nimiq identicon — a deterministic geometric avatar derived from an address
 * (github.com/nimiq/identicons via the pure-JS `identicons-esm` rewrite).
 * Generation is synchronous string work (~10µs) with no DOM dependency, so it
 * runs identically under Hermes, JSC and the web; rendering goes through
 * react-native-svg's SvgXml, which the library also implements for web.
 *
 * We assemble the SVG ourselves from the library's params instead of calling
 * `createIdenticon`, for two reasons:
 *  - The stock output is hexagonal; every other avatar in the app is a circle,
 *    so we clip to a full circle to keep one shape language everywhere.
 *  - The stock output hardcodes `clipPath id="a"`. On the web the inline SVGs
 *    share one DOM, so `url(#a)` resolves document-wide to the FIRST `#a` —
 *    with several identicons (or one in a hidden subtree) the clip silently
 *    breaks and the background paints as a full square. Ids are namespaced
 *    per seed so instances can never collide.
 */
import React from 'react';
import { SvgXml } from 'react-native-svg';
import { getIdenticonsParams, defaultCircleShape, defaultShadow } from 'identicons-esm/core';

/** Stable, collision-safe id suffix for SVG defs (djb2 → base36). */
function idSuffix(seed: string): string {
  let h = 5381;
  for (let i = 0; i < seed.length; i++) h = ((h << 5) + h + seed.charCodeAt(i)) | 0;
  return Math.abs(h).toString(36);
}

function circularIdenticonXml(seed: string): string {
  const { sections, colors } = getIdenticonsParams(seed);
  const id = `vela-idc-${idSuffix(seed)}`;
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 160">` +
    `<defs><clipPath id="${id}"><circle cx="80" cy="80" r="80"/></clipPath></defs>` +
    `<path fill="${colors.background}" d="M0 0h160v160H0z" clip-path="url(#${id})"/>` +
    `<g fill="${colors.accent}" clip-path="url(#${id})" color="${colors.main}">` +
    defaultCircleShape(colors.main) + defaultShadow +
    sections.top + sections.sides + sections.face + sections.bottom +
    `</g></svg>`
  );
}

// Addresses appear in both checksummed and lowercase forms across the app
// (stored accounts vs. typed input vs. dApp payloads). Hash the lowercased
// form so the same address always draws the same identicon.
const CACHE_LIMIT = 128;
const cache = new Map<string, string>();

function identiconXml(seed: string): string {
  // Cap the hash input: the library's chaotic hash underflows to a denormal on
  // ~1500+ char inputs, leaking an exponent digit into the hash and producing
  // fill="undefined". Real seeds (addresses) are 42 chars; nothing legit is cut.
  const key = seed.toLowerCase().slice(0, 128);
  const hit = cache.get(key);
  if (hit) return hit;
  const xml = circularIdenticonXml(key);
  if (cache.size >= CACHE_LIMIT) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  cache.set(key, xml);
  return xml;
}

export const Identicon = React.memo(function Identicon({ seed, size }: {
  seed: string;
  size: number;
}) {
  return <SvgXml xml={identiconXml(seed)} width={size} height={size} />;
});
