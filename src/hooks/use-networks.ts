/**
 * useAllNetworks — the reactive `default + custom` network list. Re-renders the
 * calling component whenever a custom network is added or removed (in Settings or
 * the Add-Token panel), so surfaces like the home chain selector, Receive, and the
 * token pickers reflect the change immediately, with no reload. Backed by the same
 * subscribe/notify store that `refreshCustomNetworks` drives.
 */
import { useSyncExternalStore } from 'react';
import { getAllNetworksSync, subscribeNetworks, type Network } from '@/models/network';

export function useAllNetworks(): Network[] {
  return useSyncExternalStore(subscribeNetworks, getAllNetworksSync, getAllNetworksSync);
}
