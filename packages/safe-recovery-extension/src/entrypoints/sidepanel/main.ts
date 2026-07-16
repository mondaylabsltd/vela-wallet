interface PanelState {
  enabled: boolean;
  owner: string;
  rpId: string;
  chainId: number;
  chainName: string;
  rpcUrl: string;
  networks: Array<{ chainId: number; chainName: string }>;
  nativeSymbol: string;
  relayerAddress: string;
  credentialPinned: boolean;
  lastSafeAddress?: string;
  balanceWei?: string;
  rpcError?: string;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const input = (id: string) => $<HTMLInputElement>(id);

function setStatus(message: string, error = false) {
  $('status').textContent = message;
  $('status').classList.toggle('error', error);
}

async function action<T = any>(message: Record<string, unknown>): Promise<T> {
  const response = await chrome.runtime.sendMessage(message);
  if (response?.error) throw new Error(response.error.message);
  return response as T;
}

function formatBalance(value: string | undefined, nativeSymbol: string): string {
  if (!value) return 'Balance unavailable';
  try {
    const wei = BigInt(value);
    const whole = wei / 10n ** 18n;
    const fraction = ((wei % 10n ** 18n) / 10n ** 12n).toString().padStart(6, '0').replace(/0+$/, '');
    return `${whole}${fraction ? `.${fraction}` : ''} ${nativeSymbol}`;
  } catch {
    return 'Balance unavailable';
  }
}

function render(state: PanelState) {
  input('enabled').checked = state.enabled;
  $('access-state').textContent = state.enabled ? 'Ready on app.safe.global' : 'Turn on to connect';
  $('owner').textContent = state.owner;
  input('rp-id').value = state.rpId;
  input('chain-id').value = String(state.chainId);
  input('chain-name').value = state.chainName;
  input('rpc-url').value = state.rpcUrl;
  $('relayer').textContent = state.relayerAddress;
  $('balance').textContent = formatBalance(state.balanceWei, state.nativeSymbol);
  const networkSelect = $<HTMLSelectElement>('network-select');
  const options = state.networks.map((network) => {
    const option = document.createElement('option');
    option.value = String(network.chainId);
    option.textContent = network.chainName;
    return option;
  });
  networkSelect.replaceChildren(...options);
  networkSelect.value = String(state.chainId);
  networkSelect.disabled = false;
  $('rpc-state').textContent = state.rpcError ?? 'RPC connected';
  $('rpc-state').classList.toggle('error', Boolean(state.rpcError));
  $('credential-state').textContent = state.credentialPinned
    ? 'Remembered for this Safe'
    : state.lastSafeAddress
      ? 'Choose on next signature'
      : 'Choose on first signature';
  $('last-safe').textContent = state.lastSafeAddress ?? 'No Safe signed yet';
  $<HTMLButtonElement>('clear-credential').disabled = !state.lastSafeAddress || !state.credentialPinned;
}

async function refresh() {
  try {
    render(await action<PanelState>({ action: 'popup-get-state' }));
  } catch (error) {
    setStatus((error as Error).message, true);
  }
}

function normalizeRpId(raw: string): string {
  return raw.trim().toLowerCase().replace(/^[a-z][a-z0-9+.-]*:\/\//, '').replace(/\/.*$/, '').replace(/:\d+$/, '');
}

function permissionPattern(raw: string): string {
  const url = new URL(raw);
  return `${url.protocol}//${url.host}/*`;
}

input('enabled').addEventListener('change', async () => {
  try {
    await action({ action: 'popup-set-enabled', enabled: input('enabled').checked });
    setStatus(input('enabled').checked
      ? 'Safe access is on. Reload Safe Wallet if it is already open.'
      : 'Safe access is off.');
    await refresh();
  } catch (error) {
    input('enabled').checked = !input('enabled').checked;
    setStatus((error as Error).message, true);
  }
});

$<HTMLSelectElement>('network-select').addEventListener('change', async () => {
  const select = $<HTMLSelectElement>('network-select');
  const selectedName = select.selectedOptions[0]?.textContent ?? `Chain ${select.value}`;
  select.disabled = true;
  try {
    await action({ action: 'popup-switch-network', chainId: Number(select.value) });
    setStatus(`Switched to ${selectedName}. Safe Wallet will follow this network.`);
    await refresh();
  } catch (error) {
    setStatus((error as Error).message, true);
    await refresh();
  } finally {
    select.disabled = false;
  }
});

$('save').addEventListener('click', async () => {
  const button = $<HTMLButtonElement>('save');
  button.disabled = true;
  try {
    const rpcUrl = input('rpc-url').value.trim();
    const rpId = normalizeRpId(input('rp-id').value);
    const origins = [permissionPattern(rpcUrl), `https://${rpId}/*`];
    const granted = await chrome.permissions.request({ origins });
    if (!granted) throw new Error('Chrome did not grant RPC/passkey host access.');
    await action({
      action: 'popup-save-config',
      chainId: Number(input('chain-id').value),
      chainName: input('chain-name').value,
      rpcUrl,
      rpId,
    });
    setStatus('Network and passkey settings saved.');
    await refresh();
  } catch (error) {
    setStatus((error as Error).message, true);
  } finally {
    button.disabled = false;
  }
});

$('copy-relayer').addEventListener('click', async () => {
  await navigator.clipboard.writeText($('relayer').textContent ?? '');
  setStatus('Gas address copied.');
});
$('refresh').addEventListener('click', () => void refresh());

$('clear-credential').addEventListener('click', async () => {
  try {
    await action({ action: 'popup-clear-credential' });
    setStatus('The next signature for this Safe will show the passkey chooser.');
    await refresh();
  } catch (error) {
    setStatus((error as Error).message, true);
  }
});

$('export-key').addEventListener('click', async () => {
  if (!confirm('Reveal the gas relayer private key? Anyone with it can take funds held by the gas address.')) return;
  try {
    const result = await action<{ privateKey: string }>({ action: 'popup-export-relayer' });
    $('private-key').textContent = result.privateKey;
    $('private-key').classList.remove('hidden');
  } catch (error) {
    setStatus((error as Error).message, true);
  }
});

$('restore-key').addEventListener('click', async () => {
  try {
    await action({ action: 'popup-import-relayer', privateKey: input('import-key').value.trim() });
    input('import-key').value = '';
    $('private-key').classList.add('hidden');
    setStatus('Gas relayer key restored.');
    await refresh();
  } catch (error) {
    setStatus((error as Error).message, true);
  }
});

$('rotate-key').addEventListener('click', async () => {
  if (!confirm('Create a new gas address? Export or empty the current address first.')) return;
  if (!confirm('The old gas address will no longer be usable by this extension. Continue?')) return;
  try {
    await action({ action: 'popup-rotate-relayer' });
    $('private-key').classList.add('hidden');
    setStatus('New gas address created. Fund it before executing a Safe transaction.');
    await refresh();
  } catch (error) {
    setStatus((error as Error).message, true);
  }
});

let refreshTimer: number | undefined;

function scheduleRefresh() {
  if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
  refreshTimer = window.setTimeout(() => {
    refreshTimer = undefined;
    void refresh();
  }, 100);
}

chrome.storage.onChanged.addListener((_changes, areaName) => {
  if (areaName === 'local') scheduleRefresh();
});
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') scheduleRefresh();
});
window.addEventListener('focus', scheduleRefresh);

void refresh();
