/**
 * Global signing request modal.
 *
 * Pops up over any screen when a dApp sends a signing request
 * through the remote-inject bridge.
 *
 * For eth_sendTransaction and eth_signTypedData, attempts ERC-7730 clear signing
 * to show human-readable transaction details. Falls back to blind signing if no
 * descriptor matches.
 */
import React, { useEffect, useState } from 'react';
import { View, Text, ScrollView, Image } from 'react-native';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { shortAddr } from '@/models/types';
import { chainName, nativeSymbol } from '@/models/network';
import {
  resolveTransaction, resolveTypedData,
  type ClearSignResult, type ClearSignField,
} from '@/services/clear-signing';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import {
  Send, FileSignature, FileText, Shield, AlertTriangle, Globe,
  ArrowRightLeft, CheckCircle, Zap, Eye,
} from 'lucide-react-native';

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

  const { method, params, origin } = incomingRequest;
  const hasClearSign = !!clearSign;

  // Derive display origin from dappInfo or raw origin
  const displayOrigin = dappInfo?.name ?? origin ?? 'Remote Bridge';
  const displayDomain = dappInfo?.url ? (() => { try { return new URL(dappInfo.url).host; } catch { return dappInfo.url; } })() : undefined;

  return (
    <AppModal visible={true} onClose={signError ? dismissRequest : rejectRequest}>
      <View style={styles.container}>
        <ScrollView showsVerticalScrollIndicator={false}>

          {/* DApp banner */}
          <View style={styles.dappBanner}>
            {dappInfo?.icon ? (
              <Image source={{ uri: dappInfo.icon }} style={styles.dappIcon} />
            ) : (
              <View style={[styles.dappIcon, styles.dappIconFallback]}>
                <Text style={styles.dappIconText}>{(displayOrigin[0] ?? '?').toUpperCase()}</Text>
              </View>
            )}
            <View style={styles.dappBannerText}>
              <Text style={styles.dappName}>{displayOrigin}</Text>
              {displayDomain && <Text style={styles.dappDomain}>{displayDomain}</Text>}
            </View>
          </View>

          {/* ============================================================= */}
          {/* Clear signed transaction */}
          {/* ============================================================= */}
          {hasClearSign ? (
            <>
              {/* Intent header */}
              <View style={styles.intentHeader}>
                <View style={styles.intentIconWrap}>
                  {intentIcon(clearSign.intent)}
                </View>
                <Text style={styles.intentText}>{clearSign.intent}</Text>
                {clearSign.contractName && (
                  <Text style={styles.intentContract}>
                    via {clearSign.contractName}
                    {clearSign.owner ? ` · ${clearSign.owner}` : ''}
                  </Text>
                )}
              </View>

              {/* Network + Account strip */}
              <View style={styles.contextStrip}>
                <View style={styles.contextChip}>
                  <Globe size={12} color={color.fg.muted} strokeWidth={2} />
                  <Text style={styles.contextChipText}>{chainName(chainId)}</Text>
                </View>
                {activeAccount && (
                  <View style={styles.contextChip}>
                    <View style={styles.miniAvatar}>
                      <Text style={styles.miniAvatarText}>
                        {(activeAccount.name[0] ?? 'V').toUpperCase()}
                      </Text>
                    </View>
                    <Text style={styles.contextChipText}>
                      {activeAccount.name}
                    </Text>
                  </View>
                )}
              </View>

              {/* Clear sign fields */}
              <View style={styles.clearFields}>
                {clearSign.fields.map((field, i) => (
                  <ClearSignFieldRow key={i} field={field} />
                ))}
              </View>

              {/* spacer */}
              <View style={{ height: space.sm }} />
            </>
          ) : (
            <>
              {/* ============================================================= */}
              {/* Blind sign fallback */}
              {/* ============================================================= */}

              {/* Header */}
              <View style={styles.header}>
                {methodIcon(method)}
                <View style={styles.headerText}>
                  <Text style={styles.methodName}>{methodLabel(method)}</Text>
                </View>
              </View>

              {/* Account info */}
              {activeAccount && (
                <View style={styles.accountRow}>
                  <View style={styles.accountAvatar}>
                    <Text style={styles.accountAvatarText}>
                      {(activeAccount.name[0] ?? 'V').toUpperCase()}
                    </Text>
                  </View>
                  <View style={styles.accountInfo}>
                    <Text style={styles.accountName}>{activeAccount.name}</Text>
                    <Text style={styles.accountAddr}>{shortAddress(activeAccount.address)}</Text>
                  </View>
                </View>
              )}

              {/* Network badge */}
              <View style={styles.networkBadge}>
                <Globe size={13} color={color.fg.muted} strokeWidth={2} />
                <Text style={styles.networkText}>{chainName(chainId)}</Text>
              </View>

              {/* Request details */}
              <View style={styles.details}>
                <Text style={styles.description}>{methodDescription(method)}</Text>

                {method === 'personal_sign' && params?.[0] && (
                  <View style={styles.messagePreview}>
                    <Text style={styles.previewLabel}>MESSAGE</Text>
                    <Text style={styles.previewText} numberOfLines={8}>
                      {decodePersonalMessage(params[0])}
                    </Text>
                  </View>
                )}

                {method === 'eth_sendTransaction' && params?.[0] && (
                  <>
                    <BlindRow label="To" value={shortAddr(params[0].to ?? '')} />
                    <BlindRow label="Value" value={formatTxValue(params[0].value, chainId)} />
                    {params[0].data && params[0].data !== '0x' && (
                      <BlindRow label="Data" value={`${Math.floor((params[0].data.length - 2) / 2)} bytes`} />
                    )}
                  </>
                )}

                {method.includes('signTypedData') && params && (
                  <View style={styles.messagePreview}>
                    <Text style={styles.previewLabel}>TYPED DATA</Text>
                    <Text style={styles.previewText} numberOfLines={6}>
                      {parseTypedDataSummary(params)}
                    </Text>
                  </View>
                )}
              </View>

              {/* Blind sign warning */}
              {method === 'eth_sendTransaction' && params?.[0]?.data && params[0].data !== '0x' && !resolving && (
                <View style={styles.blindWarning}>
                  <Eye size={14} color={color.warning.base} strokeWidth={2} />
                  <Text style={styles.blindWarningText}>
                    Unable to decode this transaction. Review carefully before signing.
                  </Text>
                </View>
              )}
            </>
          )}

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
                title={isSigning ? 'Signing...' : 'Approve'}
                onPress={approveRequest}
                variant="accent"
                loading={isSigning || resolving}
                disabled={resolving}
                style={styles.buttonFlex}
              />
              <VelaButton
                title="Reject"
                onPress={rejectRequest}
                variant="secondary"
                disabled={isSigning}
                style={styles.buttonFlex}
              />
            </>
          )}
        </View>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Clear sign field row
// ---------------------------------------------------------------------------

function ClearSignFieldRow({ field }: { field: ClearSignField }) {
  const isWarning = !!field.warning;

  return (
    <View style={[styles.fieldRow, isWarning && styles.fieldRowWarning]}>
      <Text style={styles.fieldLabel}>{field.label}</Text>
      <Text
        style={[
          styles.fieldValue,
          isWarning && styles.fieldValueWarning,
          field.format === 'addressName' && styles.fieldValueMono,
        ]}
        numberOfLines={2}
      >
        {field.value}
      </Text>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Blind sign row
// ---------------------------------------------------------------------------

function BlindRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.detailRow}>
      <Text style={styles.detailLabel}>{label}</Text>
      <Text style={styles.detailValue} numberOfLines={1}>{value}</Text>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Intent icon mapping
// ---------------------------------------------------------------------------

function intentIcon(intent: string): React.ReactNode {
  const size = 24;
  const sw = 2;
  const i = intent.toLowerCase();
  if (i === 'swap' || i === 'exchange') return <ArrowRightLeft size={size} color={color.accent.base} strokeWidth={sw} />;
  if (i === 'send' || i === 'transfer') return <Send size={size} color={color.accent.base} strokeWidth={sw} />;
  if (i === 'approve' || i === 'permit') return <CheckCircle size={size} color={color.warning.base} strokeWidth={sw} />;
  if (i === 'stake' || i === 'deposit' || i === 'supply') return <Zap size={size} color={color.success.base} strokeWidth={sw} />;
  return <FileSignature size={size} color={color.info.base} strokeWidth={sw} />;
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

function parseTypedDataSummary(params: any[]): string {
  try {
    const data = typeof params[1] === 'string' ? JSON.parse(params[1]) : params[1];
    if (data?.primaryType) {
      const msg = data.message;
      if (msg) {
        const fields = Object.entries(msg).slice(0, 3).map(([k, v]) => `${k}: ${String(v).slice(0, 40)}`).join('\n');
        return `${data.primaryType}\n${fields}`;
      }
      return data.primaryType;
    }
    return 'Structured data';
  } catch {
    return 'Structured data';
  }
}

function methodDescription(m: string): string {
  if (m === 'eth_sendTransaction') return 'This app wants to send a transaction from your wallet.';
  if (m === 'personal_sign') return 'This app wants you to sign a message.';
  if (m.includes('signTypedData')) return 'This app wants you to sign structured data.';
  return 'This app is requesting a signature.';
}

function methodLabel(m: string): string {
  if (m === 'eth_sendTransaction') return 'Send Transaction';
  if (m === 'personal_sign') return 'Sign Message';
  if (m.includes('signTypedData')) return 'Sign Typed Data';
  return m;
}

function methodIcon(m: string): React.ReactNode {
  const size = 22;
  const sw = 2;
  if (m === 'eth_sendTransaction') return <Send size={size} color={color.accent.base} strokeWidth={sw} />;
  if (m === 'personal_sign') return <FileSignature size={size} color={color.info.base} strokeWidth={sw} />;
  if (m.includes('signTypedData')) return <FileText size={size} color={color.info.base} strokeWidth={sw} />;
  return <Shield size={size} color={color.fg.muted} strokeWidth={sw} />;
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  container: {
    flex: 1,
    padding: space['3xl'],
  },

  // ===== DApp banner =====
  dappBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginBottom: space['2xl'],
    paddingBottom: space.xl,
    borderBottomWidth: 1,
    borderColor: color.border.base,
  },
  dappIcon: {
    width: 36, height: 36, borderRadius: 10,
  },
  dappIconFallback: {
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  dappIconText: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.accent.base,
  },
  dappBannerText: { flex: 1, gap: 1 },
  dappName: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  dappDomain: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },

  // ===== Clear sign styles =====
  intentHeader: {
    alignItems: 'center',
    paddingVertical: space.xl,
    gap: space.md,
  },
  intentIconWrap: {
    width: 52, height: 52, borderRadius: 26,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
    marginBottom: space.sm,
  },
  intentText: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'center',
  },
  intentContract: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    textAlign: 'center',
  },

  // Context strip (network + account chips)
  contextStrip: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: space.md,
    marginBottom: space.xl,
    justifyContent: 'center',
  },
  contextChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingVertical: space.xs,
    paddingHorizontal: space.lg,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
  },
  contextChipText: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
  },
  miniAvatar: {
    width: 16, height: 16, borderRadius: 8,
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  miniAvatarText: {
    fontSize: 8,
    ...inter.bold,
    color: color.accent.base,
  },

  // Clear sign fields
  clearFields: {
    gap: space.sm,
    paddingVertical: space.lg,
    borderTopWidth: 1,
    borderBottomWidth: 1,
    borderColor: color.border.base,
    marginBottom: space.lg,
  },
  fieldRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
    borderRadius: radius.lg,
  },
  fieldRowWarning: {
    backgroundColor: color.warning.soft,
  },
  fieldLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    flexShrink: 0,
    marginRight: space.xl,
  },
  fieldValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    textAlign: 'right',
    flex: 1,
  },
  fieldValueWarning: {
    color: color.warning.base,
    ...inter.bold,
  },
  fieldValueMono: {
    fontFamily: font.mono,
    fontWeight: '500',
  },

  originSmall: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    marginBottom: space.lg,
  },

  // ===== Blind sign styles =====
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginBottom: space['2xl'],
  },
  headerText: { flex: 1, gap: 2 },
  methodName: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  origin: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },

  accountRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    marginBottom: space.lg,
  },
  accountAvatar: {
    width: 32, height: 32, borderRadius: 16,
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  accountAvatarText: { fontSize: text.base, ...inter.bold, color: color.accent.base },
  accountInfo: { flex: 1, gap: 1 },
  accountName: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  accountAddr: { fontSize: text.xs, fontWeight: '500', fontFamily: font.mono, color: color.fg.subtle },

  networkBadge: {
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'flex-start',
    gap: space.sm,
    paddingVertical: space.sm,
    paddingHorizontal: space.lg,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    marginBottom: space.lg,
  },
  networkText: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
  },

  details: {
    gap: space.md,
    paddingVertical: space.lg,
    borderTopWidth: 1,
    borderBottomWidth: 1,
    borderColor: color.border.base,
    marginBottom: space.lg,
  },
  description: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 20, marginBottom: space.sm },

  messagePreview: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    gap: space.sm,
  },
  previewLabel: {
    fontSize: text.xs, ...inter.semibold, color: color.fg.subtle,
    letterSpacing: 1, textTransform: 'uppercase' as const,
  },
  previewText: {
    fontSize: text.sm, fontWeight: '500', fontFamily: font.mono,
    color: color.fg.base, lineHeight: 18,
  },

  detailRow: { flexDirection: 'row', justifyContent: 'space-between', paddingVertical: space.sm },
  detailLabel: { fontSize: text.base, ...inter.regular, color: color.fg.muted },
  detailValue: {
    fontSize: text.base, fontWeight: '500', fontFamily: font.mono,
    color: color.fg.base, maxWidth: '60%' as any,
  },

  blindWarning: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
    marginBottom: space.lg,
  },
  blindWarningText: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.warning.base,
    flex: 1,
    lineHeight: 18,
  },

  // ===== Shared styles =====
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

  buttonRow: { flexDirection: 'row', gap: space.lg, paddingTop: space.lg },
  buttonFlex: { flex: 1 },
}));
