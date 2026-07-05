/**
 * One-click bug-report modal.
 *
 * Lets a user file a bug with no GitHub account: the report goes to the
 * getvela.app backend proxy (which opens/+1s a GitHub issue with a server-side
 * token). Nothing is sent until the user taps "Send report", and a collapsible
 * preview shows EXACTLY what will be sent — only coarse, scrubbed diagnostics,
 * never keys/seed/balances. On any backend failure it offers the prefilled
 * GitHub-URL fallback instead, and user input is never lost.
 */
import React, { useEffect, useState } from 'react';
import { Pressable, ScrollView, Text, useWindowDimensions, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronRight } from 'lucide-react-native';
import { AppModal } from './AppModal';
import { AutoGrowTextInput } from './AutoGrowTextInput';
import { VelaButton } from './VelaButton';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { hapticSuccess, openBrowser } from '@/services/platform';
import type { AppLanguage } from '@/i18n';
import { buildReportPreview, submitBugReport, type BugReportResult } from '@/services/bug-report';
import { buildBugReportURL } from '@/services/feedback';

// The description field is the sheet's one job — it must read as a comfortable
// writing surface, not a cramped strip. Taller screens get more resting room.
const WHAT_MIN_HEIGHT = 120;
const WHAT_MIN_HEIGHT_TALL = 160;
const TALL_SCREEN_HEIGHT = 700;
const STEPS_MIN_HEIGHT = 96;

export function BugReportModal({ visible, language, area, prefillWhat, onClose }: {
  visible: boolean;
  language: AppLanguage;
  area?: string;
  /** Seed the description when opened from a specific failure (e.g. a sync error). */
  prefillWhat?: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { height: windowHeight } = useWindowDimensions();
  const [what, setWhat] = useState('');
  const [steps, setSteps] = useState('');
  const [showSteps, setShowSteps] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<BugReportResult | null>(null);

  // When opened from a known failure, pre-fill the description so the user can
  // send it as-is or add detail. Only seeds an empty field, so it never clobbers
  // what they've typed; `close()` resets `what`, so each open re-seeds.
  useEffect(() => {
    if (visible && prefillWhat) setWhat((prev) => prev || prefillWhat);
  }, [visible, prefillWhat]);

  const close = () => {
    setWhat(''); setSteps(''); setShowSteps(false); setShowPreview(false); setSubmitting(false); setResult(null);
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
            <VelaButton title={t('componentsUi.bugReport.viewIssue')} variant="secondary" onPress={() => openBrowser(result.url!)} style={styles.btn} />
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
          <VelaButton title={t('componentsUi.bugReport.openGithub')} variant="accent" onPress={() => { void openBrowser(result.fallbackUrl!).finally(close); }} style={styles.btn} />
          <VelaButton title={t('componentsUi.bugReport.cancel')} variant="secondary" onPress={close} style={styles.btn} />
        </View>
      </AppModal>
    );
  }

  // --- Compose ---
  // One glance = one job: describe the problem, send. No visible field labels
  // (the subtitle is the prompt; label keys stay as screen-reader labels), steps
  // behind a quiet disclosure, no Cancel (the sheet dismisses via X/backdrop/drag),
  // preview + GitHub path demoted to the quietest footer elements.
  return (
    <AppModal visible={visible} onClose={close}>
      <ScrollView style={styles.container} keyboardShouldPersistTaps="handled">
        <Text style={styles.title}>{t('componentsUi.bugReport.title')}</Text>
        <Text style={styles.subtitle}>{t('componentsUi.bugReport.subtitle')}</Text>

        <AutoGrowTextInput
          style={styles.input}
          minHeight={windowHeight >= TALL_SCREEN_HEIGHT ? WHAT_MIN_HEIGHT_TALL : WHAT_MIN_HEIGHT}
          value={what}
          onChangeText={setWhat}
          placeholder={t('componentsUi.bugReport.whatPlaceholder')}
          placeholderTextColor={color.fg.subtle}
          accessibilityLabel={t('componentsUi.bugReport.whatLabel')}
        />

        {showSteps ? (
          <AutoGrowTextInput
            style={[styles.input, styles.stepsInput]}
            minHeight={STEPS_MIN_HEIGHT}
            value={steps}
            onChangeText={setSteps}
            placeholder={t('componentsUi.bugReport.stepsPlaceholder')}
            placeholderTextColor={color.fg.subtle}
            accessibilityLabel={t('componentsUi.bugReport.stepsLabel')}
            autoFocus
          />
        ) : (
          <Pressable
            onPress={() => setShowSteps(true)}
            accessibilityRole="button"
            accessibilityLabel={t('componentsUi.bugReport.addSteps')}
            hitSlop={{ top: space.md, bottom: space.md }}
            style={styles.addSteps}
          >
            <Text style={styles.addStepsText}>{t('componentsUi.bugReport.addSteps')}</Text>
          </Pressable>
        )}

        <VelaButton
          title={submitting ? t('componentsUi.bugReport.sending') : t('componentsUi.bugReport.send')}
          variant="accent"
          loading={submitting}
          disabled={!what.trim() || submitting}
          onPress={handleSend}
          style={styles.sendBtn}
        />
        <Text style={styles.consent}>{t('componentsUi.bugReport.consent')}</Text>

        <Pressable
          style={styles.previewToggle}
          onPress={() => setShowPreview((v) => !v)}
          accessibilityRole="button"
          accessibilityLabel={t('componentsUi.bugReport.previewToggle')}
          accessibilityState={{ expanded: showPreview }}
          hitSlop={{ top: space.md, bottom: space.md }}
        >
          {showPreview
            ? <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2} />
            : <ChevronRight size={16} color={color.fg.subtle} strokeWidth={2} />}
          <Text style={styles.previewToggleText}>{t('componentsUi.bugReport.previewToggle')}</Text>
        </Pressable>
        {showPreview && (
          <View style={styles.previewBox}>
            <Text style={styles.previewText}>{buildReportPreview({ what, steps, area, language })}</Text>
          </View>
        )}

        {/* Always-available direct path to the prefilled GitHub template form
            (supports screenshot upload). Power users / GitHub users can skip the
            in-app flow entirely. */}
        <Pressable
          onPress={() => openBrowser(buildBugReportURL(language))}
          disabled={submitting}
          accessibilityRole="link"
          accessibilityLabel={t('componentsUi.bugReport.openGithubForm')}
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
  input: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.base,
    textAlignVertical: 'top',
  },
  stepsInput: {
    marginTop: space.md,
  },
  addSteps: {
    alignSelf: 'flex-start',
    paddingVertical: space.md,
  },
  addStepsText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  previewToggle: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    marginTop: space.lg,
    paddingVertical: space.sm,
  },
  previewToggleText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
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
    marginTop: space.md,
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
  sendBtn: {
    marginTop: space.lg,
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
