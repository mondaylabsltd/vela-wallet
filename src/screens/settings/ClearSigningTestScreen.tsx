/**
 * Clear Signing test screen — preview all signing modal scenarios.
 *
 * Triggers mock signing requests to test the SigningRequestModal UI
 * with different ERC-7730 scenarios.
 */
import React, { useState, useCallback } from 'react';
import { View, Text, ScrollView, Pressable } from 'react-native';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { color, text, inter, space, createStyles } from '@/constants/theme';
import {
  ArrowRightLeft, Send, CheckCircle, Pen, FileText, ShieldAlert,
  Zap, ChevronLeft,
} from 'lucide-react-native';
import { useRouter } from 'expo-router';
import type { BLEIncomingRequest } from '@/models/types';
import { useTranslation } from 'react-i18next';

// ---------------------------------------------------------------------------
// Mock data for each scenario
// ---------------------------------------------------------------------------

const SCENARIOS: {
  id: string;
  labelKey: string;
  subtitleKey: string;
  icon: React.ReactNode;
  iconBg: string;
  request: BLEIncomingRequest;
}[] = [
  {
    id: 'erc20-transfer',
    labelKey: 'clearSigning.scenarioErc20Transfer',
    subtitleKey: 'clearSigning.scenarioErc20TransferSub',
    icon: <Send size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-erc20-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        data: '0xa9059cbb000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa9604500000000000000000000000000000000000000000000000000000000003b9aca00',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'erc20-approve',
    labelKey: 'clearSigning.scenarioErc20Approve',
    subtitleKey: 'clearSigning.scenarioErc20ApproveSub',
    icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
    request: {
      id: 'test-erc20-approve',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        data: '0x095ea7b3000000000000000000000000111111125421ca6dc452d289314280a0f8842a65ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eth-transfer',
    labelKey: 'clearSigning.scenarioEthTransfer',
    subtitleKey: 'clearSigning.scenarioEthTransferSub',
    icon: <Send size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-eth-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
        data: '0x',
        value: '0x2386f26fc10000', // 0.01 ETH
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'personal-sign',
    labelKey: 'clearSigning.scenarioPersonalSign',
    subtitleKey: 'clearSigning.scenarioPersonalSignSub',
    icon: <Pen size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
    request: {
      id: 'test-personal-sign',
      method: 'personal_sign',
      params: [
        '0x' + Array.from(new TextEncoder().encode(
          'Welcome to OpenSea!\n\nClick to sign in and accept the OpenSea Terms of Service.\n\nThis request will not trigger a blockchain transaction or cost any gas fees.\n\nNonce: 8a3b9f2c'
        )).map(b => b.toString(16).padStart(2, '0')).join(''),
        '0x0000000000000000000000000000000000000000',
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eip712-permit',
    labelKey: 'clearSigning.scenarioEip712Permit',
    subtitleKey: 'clearSigning.scenarioEip712PermitSub',
    icon: <FileText size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
    request: {
      id: 'test-eip712-permit',
      method: 'eth_signTypedData_v4',
      params: [
        '0x0000000000000000000000000000000000000000',
        JSON.stringify({
          types: {
            EIP712Domain: [
              { name: 'name', type: 'string' },
              { name: 'chainId', type: 'uint256' },
              { name: 'verifyingContract', type: 'address' },
            ],
            Permit: [
              { name: 'owner', type: 'address' },
              { name: 'spender', type: 'address' },
              { name: 'value', type: 'uint256' },
              { name: 'nonce', type: 'uint256' },
              { name: 'deadline', type: 'uint256' },
            ],
          },
          primaryType: 'Permit',
          domain: {
            name: 'USD Coin',
            chainId: 1,
            verifyingContract: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
          },
          message: {
            owner: '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
            spender: '0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD',
            value: '1000000000', // 1000 USDC (6 decimals)
            nonce: '0',
            deadline: '1750000000',
          },
        }),
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'eip712-unknown',
    labelKey: 'clearSigning.scenarioEip712Unknown',
    subtitleKey: 'clearSigning.scenarioEip712UnknownSub',
    icon: <FileText size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
    request: {
      id: 'test-eip712-unknown',
      method: 'eth_signTypedData_v4',
      params: [
        '0x0000000000000000000000000000000000000000',
        JSON.stringify({
          types: {
            EIP712Domain: [
              { name: 'name', type: 'string' },
              { name: 'verifyingContract', type: 'address' },
            ],
            CustomOrder: [
              { name: 'maker', type: 'address' },
              { name: 'amount', type: 'uint256' },
              { name: 'expiry', type: 'uint256' },
              { name: 'salt', type: 'bytes32' },
            ],
          },
          primaryType: 'CustomOrder',
          domain: {
            name: 'Unknown Protocol',
            verifyingContract: '0x1234567890abcdef1234567890abcdef12345678',
          },
          message: {
            maker: '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
            amount: '5000000000000000000',
            expiry: '1750000000',
            salt: '0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890',
          },
        }),
      ],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'blind-tx',
    labelKey: 'clearSigning.scenarioBlindTx',
    subtitleKey: 'clearSigning.scenarioBlindTxSub',
    icon: <ShieldAlert size={18} color="#d43a2a" strokeWidth={2} />,
    iconBg: '#FEF2F2',
    request: {
      id: 'test-blind-tx',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D',
        data: '0x38ed173900000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000de0b6b3a764000000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045000000000000000000000000000000000000000000000000000000006789abcd',
        value: '0x6f05b59d3b20000', // 0.5 ETH
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: '1inch-swap',
    labelKey: 'clearSigning.scenario1inchSwap',
    subtitleKey: 'clearSigning.scenario1inchSwapSub',
    icon: <ArrowRightLeft size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-1inch',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x111111125421cA6dc452d289314280a0f8842A65',
        data: '0x12aa3caf000000000000000000000000111111125421ca6dc452d289314280a0f8842a65000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000003b9aca000000000000000000000000000000000000000000000000000de0b6b3a7640000',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-721 NFT scenarios ---
  {
    id: 'nft-transfer',
    labelKey: 'clearSigning.scenarioNftTransfer',
    subtitleKey: 'clearSigning.scenarioNftTransferSub',
    icon: <Send size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
    request: {
      id: 'test-nft-transfer',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D', // BAYC
        // transferFrom(address,address,uint256) = 0x23b872dd
        data: '0x23b872dd000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000000001981',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'nft-approve-all',
    labelKey: 'clearSigning.scenarioNftApproveAll',
    subtitleKey: 'clearSigning.scenarioNftApproveAllSub',
    icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
    request: {
      id: 'test-nft-approve-all',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D', // BAYC
        // setApprovalForAll(address,bool) = 0xa22cb465
        data: '0xa22cb4650000000000000000000000001e0049783f008a0085193e00003d00cd54003c710000000000000000000000000000000000000000000000000000000000000001',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-4626 Vault scenarios ---
  {
    id: 'vault-deposit',
    labelKey: 'clearSigning.scenarioVaultDeposit',
    subtitleKey: 'clearSigning.scenarioVaultDepositSub',
    icon: <Zap size={18} color="#22a456" strokeWidth={2} />,
    iconBg: '#EDFAF2',
    request: {
      id: 'test-vault-deposit',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xae78736Cd615f374D3085123A210448E74Fc6393', // rETH (used as example vault)
        // deposit(uint256,address) = 0x6e553f65
        data: '0x6e553f650000000000000000000000000000000000000000000000001bc16d674ec80000000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
  {
    id: 'vault-withdraw',
    labelKey: 'clearSigning.scenarioVaultWithdraw',
    subtitleKey: 'clearSigning.scenarioVaultWithdrawSub',
    icon: <Zap size={18} color="#6c5ce7" strokeWidth={2} />,
    iconBg: '#EEF0FF',
    request: {
      id: 'test-vault-withdraw',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xae78736Cd615f374D3085123A210448E74Fc6393',
        // withdraw(uint256,address,address) = 0xb460af94
        data: '0xb460af940000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-20 transferFrom ---
  {
    id: 'erc20-transferFrom',
    labelKey: 'clearSigning.scenarioErc20TransferFrom',
    subtitleKey: 'clearSigning.scenarioErc20TransferFromSub',
    icon: <Send size={18} color="#E8572A" strokeWidth={2} />,
    iconBg: '#FFF0EB',
    request: {
      id: 'test-erc20-transferFrom',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xdAC17F958D2ee523a2206206994597C13D831ec7', // USDT
        // transferFrom(address,address,uint256) = 0x23b872dd
        data: '0x23b872dd000000000000000000000000af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000000000005f5e100',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- Hex message (non-readable personal_sign) ---
  {
    id: 'hex-message',
    labelKey: 'clearSigning.scenarioHexMessage',
    subtitleKey: 'clearSigning.scenarioHexMessageSub',
    icon: <Pen size={18} color="#d4890a" strokeWidth={2} />,
    iconBg: '#FFF8F0',
    request: {
      id: 'test-hex-msg',
      method: 'personal_sign',
      params: [
        '0xdeadbeefcafebabe0102030405060708091011121314151617181920212223242526272829303132',
        '0x0000000000000000000000000000000000000000',
      ],
      origin: 'clear-signing-test',
    },
  },

  // --- Large ETH value send ---
  {
    id: 'large-eth-send',
    labelKey: 'clearSigning.scenarioLargeEthSend',
    subtitleKey: 'clearSigning.scenarioLargeEthSendSub',
    icon: <Send size={18} color="#d43a2a" strokeWidth={2} />,
    iconBg: '#FEF2F2',
    request: {
      id: 'test-large-eth',
      method: 'eth_sendTransaction',
      params: [{
        to: '0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D',
        data: '0x',
        value: '0x8ac7230489e80000', // 10 ETH
      }],
      origin: 'clear-signing-test',
    },
  },

  // --- ERC-20 limited approve (not unlimited) ---
  {
    id: 'erc20-approve-limited',
    labelKey: 'clearSigning.scenarioErc20ApproveLimited',
    subtitleKey: 'clearSigning.scenarioErc20ApproveLimitedSub',
    icon: <CheckCircle size={18} color="#22a456" strokeWidth={2} />,
    iconBg: '#EDFAF2',
    request: {
      id: 'test-erc20-limited-approve',
      method: 'eth_sendTransaction',
      params: [{
        to: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC
        // approve(address,uint256) with 500 USDC (500 * 1e6 = 0x1DCD6500)
        data: '0x095ea7b30000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad000000000000000000000000000000000000000000000000000000001dcd6500',
        value: '0x0',
      }],
      origin: 'clear-signing-test',
    },
  },
];

// ---------------------------------------------------------------------------
// Main screen
// ---------------------------------------------------------------------------

export default function ClearSigningTestScreen() {
  const { t } = useTranslation();
  const router = useRouter();

  // Tapping a scenario opens a local modal that drives the real <SigningSheet>.
  const [mockRequest, setMockRequest] = useState<BLEIncomingRequest | null>(null);

  const handleScenario = useCallback((scenario: typeof SCENARIOS[number]) => {
    setMockRequest(scenario.request);
  }, []);

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.backBtn}>
            <ChevronLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <View>
            <Text style={styles.title}>{t('clearSigning.title')}</Text>
            <Text style={styles.subtitle}>{t('clearSigning.subtitle')}</Text>
          </View>
        </View>

        {/* Scenarios */}
        <VelaCard style={styles.card}>
          {SCENARIOS.map((scenario, i) => (
            <React.Fragment key={scenario.id}>
              <Pressable
                style={styles.row}
                onPress={() => handleScenario(scenario)}
              >
                <View style={[styles.iconWrap, { backgroundColor: scenario.iconBg }]}>
                  {scenario.icon}
                </View>
                <View style={styles.rowInfo}>
                  <Text style={styles.rowTitle}>{t(scenario.labelKey, { defaultValue: scenario.labelKey })}</Text>
                  <Text style={styles.rowSub}>{t(scenario.subtitleKey, { defaultValue: scenario.subtitleKey })}</Text>
                </View>
              </Pressable>
              {i < SCENARIOS.length - 1 && <View style={styles.divider} />}
            </React.Fragment>
          ))}
        </VelaCard>

        <Text style={styles.hint}>{t('clearSigning.hint')}</Text>
      </ScrollView>

      {/* Mock signing modal — renders independently from DAppConnection */}
      {mockRequest && (
        <MockSigningModal
          request={mockRequest}
          onClose={() => setMockRequest(null)}
        />
      )}
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Mock Signing Modal — drives the REAL <SigningSheet> with mock data.
// One rendering path: production and this harness render the same component
// (no passkey / no transport), so the harness can never drift from production.
// ---------------------------------------------------------------------------

import { AppModal } from "@/components/ui/AppModal";
import { SigningSheet } from "@/components/SigningRequestModal";
import { showAlert } from "@/services/platform";
import { useWallet } from "@/models/wallet-state";

function MockSigningModal({ request, onClose }: {
  request: BLEIncomingRequest;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { activeAccount } = useWallet();
  return (
    <AppModal visible={true} onClose={onClose}>
      <SigningSheet
        request={request}
        chainId={1}
        account={activeAccount ?? { name: "Wallet" }}
        dappInfo={{ name: "Test dApp", url: "https://clear-signing-test" }}
        isSigning={false}
        signError={null}
        pendingOpHash={null}
        onApprove={() => { showAlert(t("clearSigning.alertSignedTitle"), t("clearSigning.alertSignedBody")); onClose(); }}
        onReject={onClose}
        onDismiss={onClose}
      />
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  scrollContent: { paddingBottom: space['5xl'] },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginTop: space.xl,
    marginBottom: space['2xl'],
  },
  backBtn: {
    width: 36, height: 36, borderRadius: 18,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  subtitle: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, marginTop: 2 },
  card: { padding: 0, overflow: 'hidden' },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.xl,
    paddingHorizontal: space['2xl'],
  },
  iconWrap: {
    width: 40, height: 40, borderRadius: 12,
    alignItems: 'center', justifyContent: 'center',
  },
  rowInfo: { flex: 1, gap: 2 },
  rowTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  rowSub: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },
  divider: { height: 1, backgroundColor: color.border.base, marginHorizontal: space['2xl'] },
  hint: {
    fontSize: text.sm, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', marginTop: space['2xl'], lineHeight: 18,
    paddingHorizontal: space.xl,
  },
}));
