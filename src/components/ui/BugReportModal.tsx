/**
 * One-click bug-report modal.
 *
 * Lets a user file a bug with no GitHub account: the report goes to the
 * getvela.app backend proxy (which opens/▲1s a GitHub issue with a server-side
 * token). Nothing is sent until the user taps "Send report", and a collapsible
 * preview shows EXACTLY what will be sent — only coarse, scrubbed diagnostics,
 * never keys/seed/balances. On any backend failure it offers the prefilled
 * GitHub-URL fallback instead, and user input is never lost.
 */
import React, { useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight } from 'lucide-react-native';
import { AppModal } from './AppModal';
import { AutoGrowTextInput } from './AutoGrowTextInput';
import { VelaButton } from './VelaButton';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { hapticSuccess, openURL } from '@/services/platform';
import type { AppLanguage } from '@/i18n';
import { buildReportPreview, submitBugReport, type BugReportResult } from '@/services/bug-report';
import { buildBugReportURL } from '@/services/feedback';

export function BugReportModal({ visible, language, area, onClose }: {
  visible: boolean;
  language: AppLanguage;
  area?: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [what, setWhat] = useState('');
  const [steps, setSteps] = useState('');
  const [showPreview, setShowPreview] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<BugReportResult | null>(null);

  const close = () => {
    setWhat(''); setSteps(''); setShowPreview(false); setSubmitting(false); setResult(null);
    onClose();
  };

  const handleSend = async () => {
    if (!what.trim() || submitting) return;
    setSubmitting(true);
    const r = await submitBugReport({ what, steps, area, language });
    setSubmitting(false);
    setResult(r); // keep `what`/`steps` so input is never lost on a fallback
    if (r.ok) hapticSuccess();
  };

  // --- Success ---
  if (result?.ok) {
    const body = result.deduped
      ? t('componentsUi.bugReport.successBodyDeduped', { number: result.number ?? '' })
      : t('componentsUi.bugReport.successBodyNew', { number: result.number ?? '' });
    return (
      <AppModal visible={visible} onClose={close}>
        <View style={styles.container}>
          <Text style={styles.title}>{t('componentsUi.bugReport.successTitle')}</Text>
          <Text style={styles.body}>{body}</Text>
          {result.url ? (
            <VelaButton title={t('componentsUi.bugReport.viewIssue')} variant="secondary" onPress={() => openURL(result.url!)} style={styles.btn} />
          ) : null}
          <VelaButton title={t('componentsUi.bugReport.done')} variant="accent" onPress={close} style={styles.btn} />
        </View>
      </AppModal>
    );
  }

  // --- Backend unavailable → GitHub URL fallback ---
  if (result && !result.ok && result.fallbackUrl) {
    return (
      <AppModal visible={visible} onClose={close}>
        <View style={styles.container}>
          <Text style={styles.title}>{t('componentsUi.bugReport.fallbackTitle')}</Text>
          <Text style={styles.body}>{t('componentsUi.bugReport.fallbackBody')}</Text>
          <VelaButton title={t('componentsUi.bugReport.openGithub')} variant="accent" onPress={() => { openURL(result.fallbackUrl!); close(); }} style={styles.btn} />
          <VelaButton title={t('componentsUi.bugReport.cancel')} variant="secondary" onPress={close} style={styles.btn} />
        </View>
      </AppModal>
    );
  }

  // --- Compose ---
  return (
    <AppModal visible={visible} onClose={close}>
      <ScrollView style={styles.container} keyboardShouldPersistTaps="handled">
        <Text style={styles.title}>{t('componentsUi.bugReport.title')}</Text>
        <Text style={styles.subtitle}>{t('componentsUi.bugReport.subtitle')}</Text>

        <Text style={styles.label}>{t('componentsUi.bugReport.whatLabel')}</Text>
        <AutoGrowTextInput
          style={styles.input}
          minHeight={64}
          value={what}
          onChangeText={setWhat}
          placeholder={t('componentsUi.bugReport.whatPlaceholder')}
          placeholderTextColor={color.fg.subtle}
        />

        <Text style={styles.label}>{t('componentsUi.bugReport.stepsLabel')}</Text>
        <AutoGrowTextInput
          style={styles.input}
          minHeight={64}
          value={steps}
          onChangeText={setSteps}
          placeholder={t('componentsUi.bugReport.stepsPlaceholder')}
          placeholderTextColor={color.fg.subtle}
        />

        <Pressable style={styles.previewToggle} onPress={() => setShowPreview((v) => !v)}>
          {showPreview
            ? <ChevronDown size={16} color={color.fg.muted} strokeWidth={2} />
            : <ChevronRight size={16} color={color.fg.muted} strokeWidth={2} />}
          <Text style={styles.previewToggleText}>{t('componentsUi.bugReport.previewToggle')}</Text>
        </Pressable>
        {showPreview && (
          <View style={styles.previewBox}>
            <Text style={styles.previewText}>{buildReportPreview({ what, steps, area, language })}</Text>
          </View>
        )}

        <Text style={styles.consent}>{t('componentsUi.bugReport.consent')}</Text>

        <VelaButton
          title={submitting ? t('componentsUi.bugReport.sending') : t('componentsUi.bugReport.send')}
          variant="accent"
          loading={submitting}
          disabled={!what.trim() || submitting}
          onPress={handleSend}
          style={styles.btn}
        />
        <VelaButton
          title={t('componentsUi.bugReport.cancel')}
          variant="secondary"
          disabled={submitting}
          onPress={close}
          style={styles.btn}
        />

        {/* Always-available direct path to the prefilled GitHub template form
            (supports screenshot upload). Power users / GitHub users can skip the
            in-app flow entirely. */}
        <Pressable
          onPress={() => openURL(buildBugReportURL(language))}
          disabled={submitting}
          style={styles.githubLink}
        >
          <Text style={styles.githubLinkText}>{t('componentsUi.bugReport.openGithubForm')}</Text>
        </Pressable>
      </ScrollView>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: color.bg.base,
    paddingHorizontal: space['2xl'],
    paddingTop: space.xl,
    paddingBottom: space.lg,
  },
  title: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.xs,
  },
  subtitle: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    marginBottom: space.lg,
  },
  label: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    marginTop: space.md,
    marginBottom: space.xs,
  },
  input: {
    minHeight: 64,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.md,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.md,
    paddingVertical: space.sm,
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.base,
    textAlignVertical: 'top',
  },
  previewToggle: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    marginTop: space.lg,
    marginBottom: space.xs,
  },
  previewToggleText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  previewBox: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.md,
    padding: space.md,
  },
  previewText: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.muted,
  },
  consent: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.lg,
    marginBottom: space.md,
  },
  body: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    marginBottom: space.lg,
  },
  btn: {
    marginTop: space.sm,
  },
  githubLink: {
    alignItems: 'center',
    paddingVertical: space.md,
    marginTop: space.xs,
  },
  githubLinkText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
    textDecorationLine: 'underline',
  },
}));
