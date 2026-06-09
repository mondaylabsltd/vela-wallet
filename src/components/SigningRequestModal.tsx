/**
 * Global signing request modal — ERC-7730 Clear Signing UI.
 *
 * Renders signing requests with intent-driven, human-readable layouts:
 *   - Clear signed transactions/signatures (descriptor found)
 *   - Plain message signing (personal_sign)
 *   - Blind sign fallback (no descriptor)
 *
 * Design principles:
 *   L1 — Intent: large colored action word (Swap, Send, Approve, Sign)
 *   L2 — Substance: token cards with amounts, recipients, flow arrows
 *   L3 — Context: contract info, chain, details (collapsed)
 */
import React, { useEffect, useState, useCallback } from 'react';
import {
  View, Text, ScrollView, Image, Pressable,
} from 'react-native';
import * as Clipboard from 'expo-clipboard';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet } from '@/models/wallet-state';
import { shortAddr } from '@/models/types';
import { chainName, nativeSymbol } from '@/models/network';
import {
  resolveTransaction, resolveTypedData,
  type ClearSignResult, type ClearSignField, type SigningRisk,
} from '@/services/clear-signing';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { ChainLogo } from '@/components/ChainLogo';
import { DEFAULT_NETWORKS } from '@/models/network';
import {
  Shield, AlertTriangle, Copy, ChevronDown, Check,
  ArrowDown, Lock, ShieldAlert, ShieldCheck, Pen,
} from 'lucide-react-native';

// ---------------------------------------------------------------------------
// Risk → color mapping
// ---------------------------------------------------------------------------

const RISK_COLORS: Record<SigningRisk, string> = {
  safe: '#22a456',
  normal: '#E8572A',
  caution: '#d4890a',
  danger: '#d43a2a',
};

const PURPLE = '#6c5ce7';

function riskColor(risk: SigningRisk): string {
  return RISK_COLORS[risk];
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function SigningRequestModal() {
  const {
    incomingRequest, isSigning, signError, chainId, dappInfo,
    approveRequest, rejectRequest, dismissRequest,
  } = useDAppConnection();
  const { activeAccount } = useWallet();

  const [clearSign, setClearSign] = useState<ClearSignResult | null>(null);
  const [resolving, setResolving] = useState(false);

  // Resolve clear signing when a new request comes in
  useEffect(() => {
    if (!incomingRequest) { setClearSign(null); return; }

    const { method, params } = incomingRequest;

    if (method === 'eth_sendTransaction' && params?.[0]) {
      setResolving(true);
      resolveTransaction(params[0].to, params[0].data, params[0].value, chainId)
        .then(setClearSign)
        .catch(() => setClearSign(null))
        .finally(() => setResolving(false));
    } else if (method.includes('signTypedData') && params) {
      setResolving(true);
      const typedDataRaw = params[1] ?? params[0];
      try {
        const typedData = typeof typedDataRaw === 'string' ? JSON.parse(typedDataRaw) : typedDataRaw;
        resolveTypedData(typedData, chainId)
          .then(setClearSign)
          .catch(() => setClearSign(null))
          .finally(() => setResolving(false));
      } catch {
        setClearSign(null);
        setResolving(false);
      }
    } else {
      setClearSign(null);
    }
  }, [incomingRequest, chainId]);

  if (!incomingRequest) return null;

  const { method, params } = incomingRequest;
  const isPersonalSign = method === 'personal_sign';
  const isTypedData = method.includes('signTypedData');
  const isTx = method === 'eth_sendTransaction';

  // Derive display info
  const displayOrigin = dappInfo?.name ?? incomingRequest.origin ?? 'dApp';
  const displayDomain = dappInfo?.url
    ? (() => { try { return new URL(dappInfo.url).host; } catch { return dappInfo.url; } })()
    : undefined;

  const addr = activeAccount?.address;

  // Choose which view to render
  const renderContent = () => {
    if (clearSign) {
      return <ClearSignView cs={clearSign} chainId={chainId} accountName={activeAccount?.name} accountAddress={addr} />;
    }
    if (isPersonalSign && params?.[0]) {
      return <MessageSignView hexMsg={params[0]} chainId={chainId} accountName={activeAccount?.name} accountAddress={addr} />;
    }
    if (isTypedData && params) {
      return <BlindTypedDataView params={params} chainId={chainId} accountName={activeAccount?.name} accountAddress={addr} />;
    }
    if (isTx && params?.[0]) {
      return <BlindTransactionView tx={params[0]} chainId={chainId} accountName={activeAccount?.name} accountAddress={addr} />;
    }
    // Fallback
    return (
      <View style={styles.fallback}>
        <Shield size={28} color={color.fg.muted} strokeWidth={2} />
        <Text style={styles.fallbackText}>Signature request</Text>
      </View>
    );
  };

  // Button config
  const buttonLabel = (): string => {
    if (isSigning) return 'Signing...';
    if (clearSign) {
      if (clearSign.type === 'signature') return 'Sign';
      const i = clearSign.intent;
      return `Confirm ${i.charAt(0).toUpperCase() + i.slice(1)}`;
    }
    if (isPersonalSign || isTypedData) return 'Sign';
    return 'Approve';
  };

  const buttonVariant = (): 'accent' | 'secondary' => 'accent';

  return (
    <AppModal visible={true} onClose={signError ? dismissRequest : rejectRequest}>
      <View style={styles.container}>
        <ScrollView showsVerticalScrollIndicator={false}>
          {/* dApp banner — always shown */}
          <DAppBanner
            name={displayOrigin}
            domain={displayDomain}
            icon={dappInfo?.icon}
          />

          {renderContent()}

          {/* Error */}
          {signError && (
            <View style={styles.errorCard}>
              <AlertTriangle size={16} color={color.error.base} strokeWidth={2} />
              <Text style={styles.errorText}>{signError}</Text>
            </View>
          )}
        </ScrollView>

        {/* Buttons */}
        <View style={styles.buttonRow}>
          {signError ? (
            <VelaButton
              title="Dismiss"
              onPress={dismissRequest}
              variant="secondary"
              style={styles.buttonFlex}
            />
          ) : (
            <>
              <VelaButton
                title="Reject"
                onPress={rejectRequest}
                variant="secondary"
                disabled={isSigning}
                style={styles.buttonFlex}
              />
              <VelaButton
                title={buttonLabel()}
                onPress={approveRequest}
                variant={buttonVariant()}
                loading={isSigning || resolving}
                disabled={resolving}
                style={styles.buttonFlex}
              />
            </>
          )}
        </View>
      </View>
    </AppModal>
  );
}

// ===========================================================================
// dApp Banner
// ===========================================================================

function DAppBanner({ name, domain, icon }: {
  name: string;
  domain?: string;
  icon?: string;
}) {
  return (
    <View style={styles.dappBanner}>
      {icon ? (
        <Image source={{ uri: icon }} style={styles.dappLogo} />
      ) : (
        <View style={[styles.dappLogo, styles.dappLogoFallback]}>
          <Text style={styles.dappLogoText}>{(name[0] ?? '?').toUpperCase()}</Text>
        </View>
      )}
      <View style={styles.dappInfo}>
        <Text style={styles.dappName} numberOfLines={1}>{name}</Text>
        {domain && <Text style={styles.dappDomain} numberOfLines={1}>{domain}</Text>}
      </View>
      <View style={styles.e2eBadge}>
        <Lock size={10} color={color.success.base} strokeWidth={2.5} />
        <Text style={styles.e2eText}>E2E</Text>
      </View>
    </View>
  );
}

// ===========================================================================
// Clear Sign View (descriptor found)
// ===========================================================================

function ClearSignView({ cs, chainId, accountName, accountAddress }: {
  cs: ClearSignResult;
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const rc = riskColor(cs.risk);

  // Separate fields by role
  const sendAmounts = cs.fields.filter(f => f.role === 'send-amount');
  const receiveAmounts = cs.fields.filter(f => f.role === 'receive-amount');
  const recipients = cs.fields.filter(f => f.role === 'recipient');
  const spenders = cs.fields.filter(f => f.role === 'spender');
  const generic = cs.fields.filter(f => f.role === 'generic');

  // Determine if this is a swap-like layout (send → receive)
  const isSwapLayout = sendAmounts.length > 0 && receiveAmounts.length > 0;
  const hasRecipient = recipients.length > 0 || spenders.length > 0;

  return (
    <View>
      {/* Context: chain + account */}
      <ContextStrip chainId={chainId} accountName={accountName} accountAddress={accountAddress} />

      {/* L1: Intent */}
      <IntentHeader intent={cs.intent} color={rc} />

      {/* L2: Token cards + flow */}
      {isSwapLayout ? (
        <>
          {sendAmounts.map((f, i) => (
            <TokenCard key={`s${i}`} field={f} variant="send" />
          ))}
          <FlowArrow />
          {receiveAmounts.map((f, i) => (
            <TokenCard key={`r${i}`} field={f} variant="receive" />
          ))}
        </>
      ) : sendAmounts.length > 0 ? (
        <>
          {sendAmounts.map((f, i) => (
            <TokenCard key={`s${i}`} field={f} variant={cs.risk === 'caution' ? 'caution' : 'send'} />
          ))}
          {hasRecipient && <FlowArrow />}
        </>
      ) : null}

      {/* Spender / recipient */}
      {spenders.map((f, i) => (
        <ContractBar
          key={`sp${i}`}
          label="Spender"
          name={f.value}
          address={cs.contractAddress}
          verified={cs.verified}
        />
      ))}
      {recipients.map((f, i) => (
        <ContractBar
          key={`re${i}`}
          label="Recipient"
          name={f.value}
          address={undefined}
          verified={false}
        />
      ))}

      {/* Warning for unlimited approvals etc. */}
      {cs.fields.some(f => f.warning) && (
        <WarningBanner
          severity="danger"
          text={`Unlimited — this contract can spend all your tokens`}
        />
      )}

      {/* Generic fields */}
      {generic.length > 0 && (
        <View style={styles.genericFields}>
          {generic.map((f, i) => (
            <GenericFieldRow key={i} field={f} />
          ))}
        </View>
      )}

      {/* Contract bar (if not already shown via spender/recipient) */}
      {!hasRecipient && cs.contractAddress && (
        <ContractBar
          label="Interacting with"
          name={cs.contractName ? `${cs.contractName}${cs.owner ? ` · ${cs.owner}` : ''}` : undefined}
          address={cs.contractAddress}
          verified={cs.verified}
        />
      )}
    </View>
  );
}

// ===========================================================================
// Message Sign View (personal_sign)
// ===========================================================================

function MessageSignView({ hexMsg, chainId, accountName, accountAddress }: {
  hexMsg: string;
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const decoded = decodePersonalMessage(hexMsg);

  return (
    <View>
      <ContextStrip chainId={chainId} accountName={accountName} accountAddress={accountAddress} />
      <IntentHeader intent="Sign Message" color={PURPLE} />

      <View style={styles.msgBubble}>
        <View style={styles.msgTag}>
          <Pen size={10} color={color.fg.subtle} strokeWidth={2} />
          <Text style={styles.msgTagText}>personal_sign · No gas fee</Text>
        </View>
        <Text style={styles.msgText}>{decoded}</Text>
      </View>
    </View>
  );
}

// ===========================================================================
// Blind Typed Data View (EIP-712, no descriptor)
// ===========================================================================

function BlindTypedDataView({ params, chainId, accountName, accountAddress }: {
  params: any[];
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const { primaryType, domain, fields } = parseTypedDataForDisplay(params);

  return (
    <View>
      <ContextStrip chainId={chainId} accountName={accountName} accountAddress={accountAddress} />
      <IntentHeader intent="Sign Typed Data" color="#d4890a" />

      {/* Domain info */}
      {domain && (
        <ContractBar
          label="Signing for"
          name={domain.name}
          address={domain.verifyingContract?.toLowerCase()}
          verified={false}
        />
      )}

      {/* Primary type + fields */}
      <View style={styles.genericFields}>
        {primaryType && (
          <View style={styles.genRow}>
            <Text style={styles.genLabel}>Type</Text>
            <Text style={styles.genValue}>{primaryType}</Text>
          </View>
        )}
        {fields.map(([k, v], i) => (
          <View key={i} style={styles.genRow}>
            <Text style={styles.genLabel}>{k}</Text>
            <Text style={styles.genValue} numberOfLines={2}>{v}</Text>
          </View>
        ))}
      </View>

      <WarningBanner
        severity="caution"
        text="This typed data could not be decoded with a known descriptor. Review carefully."
      />
    </View>
  );
}

// ===========================================================================
// Blind Transaction View (no descriptor)
// ===========================================================================

function BlindTransactionView({ tx, chainId, accountName, accountAddress }: {
  tx: any;
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const [showRaw, setShowRaw] = useState(false);
  const sym = nativeSymbol(chainId);
  const value = formatTxValue(tx.value, chainId);
  const hasData = tx.data && tx.data !== '0x';
  const dataSize = hasData ? Math.floor((tx.data.length - 2) / 2) : 0;

  return (
    <View>
      <ContextStrip chainId={chainId} accountName={accountName} accountAddress={accountAddress} />
      <IntentHeader
        intent={hasData ? 'Unknown' : 'Send'}
        color={hasData ? '#d43a2a' : '#E8572A'}
      />

      {/* Value card */}
      {value !== `0 ${sym}` && (
        <TokenCard
          field={{ label: 'Value', value, format: 'amount', role: 'send-amount' }}
          variant={hasData ? 'danger' : 'send'}
        />
      )}

      {hasData && <FlowArrow danger />}

      {/* Contract */}
      <ContractBar
        label={hasData ? 'Unverified contract' : 'Recipient'}
        address={tx.to}
        verified={false}
        warning={hasData}
      />

      {/* Blind sign warning */}
      {hasData && (
        <>
          <WarningBanner
            severity="danger"
            text={`Unable to decode — no ERC-7730 descriptor (${dataSize} bytes)`}
          />

          {/* Raw data toggle */}
          <Pressable
            style={styles.detailsToggle}
            onPress={() => setShowRaw(!showRaw)}
          >
            <Text style={styles.detailsToggleText}>
              Raw calldata · {dataSize} bytes
            </Text>
            <ChevronDown
              size={12}
              color={color.fg.subtle}
              strokeWidth={2}
              style={showRaw ? { transform: [{ rotate: '180deg' }] } : undefined}
            />
          </Pressable>
          {showRaw && (
            <ScrollView horizontal={false} style={styles.rawBlock}>
              <Text style={styles.rawText}>
                {tx.data.slice(0, 200)}{tx.data.length > 200 ? '...' : ''}
              </Text>
            </ScrollView>
          )}
        </>
      )}

    </View>
  );
}

// ===========================================================================
// Shared sub-components
// ===========================================================================

function IntentHeader({ intent, color: intentColor }: { intent: string; color: string }) {
  return (
    <View style={styles.intentHeader}>
      <Text style={[styles.intentText, { color: intentColor }]}>
        {intent}
      </Text>
    </View>
  );
}

function TokenCard({ field, variant }: {
  field: ClearSignField;
  variant: 'send' | 'receive' | 'caution' | 'danger';
}) {
  const bgMap = {
    send: { backgroundColor: '#FEF2EE' },
    receive: { backgroundColor: '#EEF6FF' },
    caution: { backgroundColor: '#FFF8EE' },
    danger: { backgroundColor: '#FDF0EE' },
  };

  return (
    <View style={[styles.tokenCard, bgMap[variant]]}>
      <View style={styles.tokenInfo}>
        <Text style={styles.tokenAmount} numberOfLines={1}>{field.value}</Text>
        <Text style={styles.tokenLabel}>{field.label}</Text>
      </View>
      {field.warning && (
        <View style={styles.tokenWarning}>
          <AlertTriangle size={14} color={RISK_COLORS.danger} strokeWidth={2} />
        </View>
      )}
    </View>
  );
}

function FlowArrow({ danger }: { danger?: boolean }) {
  return (
    <View style={styles.flowArrow}>
      <View style={[styles.flowCircle, danger && styles.flowCircleDanger]}>
        <ArrowDown
          size={14}
          color={danger ? RISK_COLORS.danger : color.fg.subtle}
          strokeWidth={2.5}
        />
      </View>
    </View>
  );
}

function ContractBar({ label, name, address, verified, warning }: {
  label: string;
  name?: string;
  address?: string;
  verified: boolean;
  warning?: boolean;
}) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    if (!address) return;
    await Clipboard.setStringAsync(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [address]);

  return (
    <View style={[styles.contractBar, warning && styles.contractBarWarning]}>
      <View style={styles.contractInfo}>
        <Text style={styles.contractLabel}>{label}</Text>
        <View style={styles.contractAddrRow}>
          {name && <Text style={styles.contractName} numberOfLines={1}>{name}</Text>}
          {address && (
            <Text style={styles.contractAddr}>{shortAddr(address)}</Text>
          )}
        </View>
      </View>
      {address && (
        <Pressable onPress={handleCopy} hitSlop={8} style={[styles.copyBtn, copied && styles.copyBtnDone]}>
          {copied
            ? <Check size={12} color={color.success.base} strokeWidth={2.5} />
            : <Copy size={12} color={color.fg.muted} strokeWidth={2} />
          }
        </Pressable>
      )}
      {verified && (
        <View style={styles.verifiedBadge}>
          <ShieldCheck size={12} color={color.success.base} strokeWidth={2} />
        </View>
      )}
      {warning && (
        <ShieldAlert size={14} color={RISK_COLORS.danger} strokeWidth={2} />
      )}
    </View>
  );
}

function WarningBanner({ severity, text: msg }: {
  severity: 'caution' | 'danger';
  text: string;
}) {
  const isDanger = severity === 'danger';
  return (
    <View style={[styles.warnBanner, isDanger ? styles.warnDanger : styles.warnCaution]}>
      <AlertTriangle
        size={14}
        color={isDanger ? RISK_COLORS.danger : RISK_COLORS.caution}
        strokeWidth={2}
      />
      <Text style={[styles.warnText, { color: isDanger ? RISK_COLORS.danger : RISK_COLORS.caution }]}>
        {msg}
      </Text>
    </View>
  );
}

function GenericFieldRow({ field }: { field: ClearSignField }) {
  return (
    <View style={[styles.genRow, field.warning && styles.genRowWarning]}>
      <Text style={styles.genLabel}>{field.label}</Text>
      <Text
        style={[styles.genValue, field.warning && { color: RISK_COLORS.danger }]}
        numberOfLines={2}
      >
        {field.value}
      </Text>
    </View>
  );
}

function ContextStrip({ chainId, accountName, accountAddress }: {
  chainId: number;
  accountName?: string;
  accountAddress?: string;
}) {
  const net = DEFAULT_NETWORKS.find(n => n.chainId === chainId);

  return (
    <View style={styles.contextStrip}>
      {net ? (
        <ChainLogo
          label={net.iconLabel}
          color={net.iconColor}
          bgColor={net.iconBg}
          logoURL={net.logoURL}
          size={18}
        />
      ) : null}
      <Text style={styles.contextChainName}>{chainName(chainId)}</Text>
      {accountName && (
        <>
          <Text style={styles.contextDot}>·</Text>
          <View style={styles.contextAccount}>
            <Text style={styles.contextAccountName}>{accountName}</Text>
            {accountAddress && (
              <Text style={styles.contextAccountAddr}>{shortAddr(accountAddress)}</Text>
            )}
          </View>
        </>
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function decodePersonalMessage(hexMsg: string): string {
  try {
    const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
    const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map(b => parseInt(b, 16)));
    const decoded = new TextDecoder().decode(bytes);
    if (/^[\x20-\x7E\n\r\t]+$/.test(decoded)) return decoded;
    return `0x${clean.slice(0, 64)}${clean.length > 64 ? '...' : ''}`;
  } catch {
    return hexMsg.slice(0, 66) + (hexMsg.length > 66 ? '...' : '');
  }
}

function formatTxValue(value: string | undefined, cid: number): string {
  const sym = nativeSymbol(cid);
  if (!value || value === '0x0' || value === '0x') return `0 ${sym}`;
  try {
    const clean = value.startsWith('0x') ? value.slice(2) : value;
    const wei = BigInt('0x' + clean);
    const eth = Number(wei) / 1e18;
    if (eth === 0) return `0 ${sym}`;
    if (eth < 0.0001) return `< 0.0001 ${sym}`;
    return eth.toFixed(4).replace(/\.?0+$/, '') + ' ' + sym;
  } catch {
    return value ?? '0';
  }
}

function parseTypedDataForDisplay(params: any[]): {
  primaryType: string | null;
  domain: any;
  fields: [string, string][];
} {
  try {
    const data = typeof params[1] === 'string' ? JSON.parse(params[1]) : (params[1] ?? params[0]);
    const primaryType = data?.primaryType ?? null;
    const domain = data?.domain;
    const msg = data?.message;
    const fields: [string, string][] = msg
      ? Object.entries(msg).slice(0, 5).map(([k, v]) => [k, String(v).slice(0, 60)])
      : [];
    return { primaryType, domain, fields };
  } catch {
    return { primaryType: null, domain: null, fields: [] };
  }
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  container: {
    flex: 1,
    padding: space['3xl'],
  },

  // ===== dApp Banner =====
  dappBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    marginBottom: space['2xl'],
  },
  dappLogo: {
    width: 36, height: 36, borderRadius: 10,
  },
  dappLogoFallback: {
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  dappLogoText: {
    fontSize: text.lg, ...inter.bold, color: color.accent.base,
  },
  dappInfo: { flex: 1, gap: 1 },
  dappName: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  dappDomain: {
    fontSize: text.xs, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },
  e2eBadge: {
    flexDirection: 'row', alignItems: 'center', gap: 3,
    paddingHorizontal: space.md, paddingVertical: space.xs,
    backgroundColor: color.success.soft, borderRadius: radius.full,
  },
  e2eText: { fontSize: text.xs, ...inter.bold, color: color.success.base },

  // ===== Intent Header =====
  intentHeader: {
    alignItems: 'center',
    paddingVertical: space.xl,
  },
  intentText: {
    fontSize: text['4xl'],
    ...inter.bold,
    textAlign: 'center',
    letterSpacing: -0.5,
  },

  // ===== Token Card =====
  tokenCard: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: space.xl,
    paddingHorizontal: space['2xl'],
    borderRadius: radius['2xl'],
    marginVertical: space.sm,
  },
  tokenInfo: { flex: 1 },
  tokenAmount: {
    fontSize: text['3xl'],
    ...inter.bold,
    color: color.fg.base,
    letterSpacing: -0.3,
  },
  tokenLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
    marginTop: space.xs,
  },
  tokenWarning: {
    width: 28, height: 28, borderRadius: 14,
    backgroundColor: 'rgba(212,58,42,0.1)',
    alignItems: 'center', justifyContent: 'center',
  },

  // ===== Flow Arrow =====
  flowArrow: {
    alignItems: 'center',
    marginVertical: -space.sm,
    zIndex: 1,
  },
  flowCircle: {
    width: 30, height: 30, borderRadius: 15,
    backgroundColor: color.bg.raised,
    borderWidth: 2, borderColor: color.border.base,
    alignItems: 'center', justifyContent: 'center',
    ...shadow.sm,
  },
  flowCircleDanger: {
    borderColor: '#e8a99a',
  },

  // ===== Contract Bar =====
  contractBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  contractBarWarning: {
    borderWidth: 1,
    borderColor: '#e8a99a',
  },
  contractInfo: { flex: 1, gap: 2 },
  contractLabel: {
    fontSize: 10, ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.3,
  },
  contractAddrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    flexWrap: 'wrap',
  },
  contractName: {
    fontSize: text.sm, ...inter.semibold, color: color.success.base,
  },
  contractAddr: {
    fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },
  copyBtn: {
    width: 28, height: 28, borderRadius: radius.md,
    borderWidth: 1, borderColor: color.border.base,
    backgroundColor: color.bg.raised,
    alignItems: 'center', justifyContent: 'center',
  },
  copyBtnDone: {
    borderColor: color.success.base,
    backgroundColor: color.success.soft,
  },
  verifiedBadge: {
    width: 24, height: 24,
    alignItems: 'center', justifyContent: 'center',
  },

  // ===== Warning Banner =====
  warnBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    borderRadius: radius.xl,
    marginVertical: space.md,
  },
  warnCaution: {
    backgroundColor: color.warning.soft,
    borderWidth: 1, borderColor: color.warning.border,
  },
  warnDanger: {
    backgroundColor: color.error.soft,
    borderWidth: 1, borderColor: '#e8a99a',
  },
  warnText: {
    fontSize: text.sm, ...inter.semibold, flex: 1, lineHeight: 18,
  },

  // ===== Generic Fields =====
  genericFields: {
    gap: space.sm,
    marginVertical: space.md,
  },
  genRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    gap: space.lg,
  },
  genRowWarning: {
    backgroundColor: color.warning.soft,
  },
  genLabel: {
    fontSize: text.sm, ...inter.medium, color: color.fg.muted,
    flexShrink: 0,
  },
  genValue: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base,
    textAlign: 'right', flex: 1,
    fontFamily: font.mono, fontWeight: '500' as const,
  },

  // ===== Message Bubble =====
  msgBubble: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius['2xl'],
    padding: space['2xl'],
    marginVertical: space.md,
  },
  msgTag: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    alignSelf: 'center',
    paddingVertical: space.xs,
    paddingHorizontal: space.lg,
    backgroundColor: color.border.base,
    borderRadius: radius.full,
    marginBottom: space.xl,
  },
  msgTagText: {
    fontSize: 10, ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.3,
  },
  msgText: {
    fontSize: text.base, ...inter.regular,
    color: color.fg.base,
    lineHeight: 22,
    textAlign: 'center',
  },

  // ===== Context Strip =====
  contextStrip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    marginBottom: space.lg,
  },
  contextChainName: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base,
  },
  contextDot: {
    fontSize: text.xs, color: color.fg.subtle,
  },
  contextAccount: {
    flex: 1, gap: 1,
  },
  contextAccountName: {
    fontSize: text.sm, ...inter.semibold, color: color.fg.base,
  },
  contextAccountAddr: {
    fontSize: 10, fontWeight: '500' as const, fontFamily: font.mono,
    color: color.fg.muted,
  },

  // ===== Details Toggle =====
  detailsToggle: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
  },
  detailsToggleText: {
    fontSize: text.xs, ...inter.semibold, color: color.fg.subtle,
  },

  // ===== Raw Data =====
  rawBlock: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    maxHeight: 80,
    marginBottom: space.lg,
  },
  rawText: {
    fontSize: 9, fontFamily: font.mono, fontWeight: '400' as const,
    color: color.fg.subtle, lineHeight: 14,
  },

  // ===== Fallback =====
  fallback: {
    alignItems: 'center',
    paddingVertical: space['5xl'],
    gap: space.lg,
  },
  fallbackText: {
    fontSize: text.lg, ...inter.regular, color: color.fg.muted,
  },

  // ===== Error =====
  errorCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.error.soft,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  errorText: { fontSize: text.sm, ...inter.regular, color: color.error.base, flex: 1 },

  // ===== Buttons =====
  buttonRow: { flexDirection: 'row', gap: space.lg, paddingTop: space.lg },
  buttonFlex: { flex: 1 },
}));
