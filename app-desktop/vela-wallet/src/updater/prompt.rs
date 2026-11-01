//! The update prompt and the work behind its buttons.
//!
//! A card in the window's bottom-right corner — not a dialog: an update is
//! never in the way of a send. It says "Update available" with the two
//! versions, and answers Install or Later:
//!
//! - **Later** puts it away until the next launch, and stops a download that
//!   is under way (the partial file is removed).
//! - **Install** downloads the signed artifact with its progress
//!   ([`super::download`]), keeps it only if it is the signed bytes, hands it
//!   to the OS ([`super::install_macos`], [`super::install_windows`]) and
//!   relaunches — never while a submit is out (`relay::may_close`), which it
//!   waits for. A refusal at any step leaves the running app as it was: the
//!   card says why under its red bar, in words, and offers "Try again" when
//!   another attempt can go differently ([`Refusal::retryable`]); the
//!   detail is in the log.
//!
//! The state is a `Global` ([`UpdateState`]); its transitions are plain
//! methods, tested without a window.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Div, FontWeight, Global, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, relative,
};

use super::{PromptWords, Refusal, Release};
use crate::theme::{self, Theme};
use crate::ui::a11y::Accessible as _;

/// The first look after launch: late enough to keep it off the launch path.
const FIRST_CHECK: Duration = Duration::from_secs(8);
/// Then twice a day while the app runs.
const RECHECK: Duration = Duration::from_secs(12 * 60 * 60);
/// The card's width.
const CARD_W: f32 = 340.;

/// Where the prompt is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing on offer, or put away (Later).
    Hidden,
    /// "Update available": Install / Later.
    Offered,
    /// Install was pressed: this many bytes are in.
    Downloading { received: u64 },
    /// The download is the signed bytes; the OS is taking it.
    Installing,
    /// A step refused; the running app is unchanged. The refusal in words;
    /// Try again (when it can go differently) / Later.
    Failed(Refusal),
}

/// The prompt's state: a `Global`, so the root draws it over any screen.
pub struct UpdateState {
    pub release: Option<Release>,
    pub phase: Phase,
    /// This build's version, as the card says it.
    pub current: &'static str,
    words: Option<PromptWords>,
    /// Later was pressed: nothing is offered again this launch.
    dismissed: bool,
    /// The download under way is still wanted.
    wanted: Arc<AtomicBool>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            release: None,
            phase: Phase::Hidden,
            current: super::current_version(),
            words: None,
            dismissed: false,
            wanted: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Global for UpdateState {}

impl UpdateState {
    /// A verified release is on offer — unless the person already said
    /// Later this launch, or one is already being installed.
    pub fn offer(&mut self, release: Release) -> bool {
        if self.dismissed || !matches!(self.phase, Phase::Hidden | Phase::Offered) {
            return false;
        }
        self.release = Some(release);
        self.phase = Phase::Offered;
        true
    }

    /// Install (or Try again) was pressed: the download starts — not after
    /// a refusal that another attempt would only repeat.
    pub fn start(&mut self) -> Option<(Release, Arc<AtomicBool>)> {
        let may = match &self.phase {
            Phase::Offered => true,
            Phase::Failed(refusal) => refusal.retryable(),
            _ => false,
        };
        if !may {
            return None;
        }
        let release = self.release.clone()?;
        self.phase = Phase::Downloading { received: 0 };
        // A fresh flag per attempt: a Later that stopped the last one does
        // not stop this one.
        self.wanted = Arc::new(AtomicBool::new(true));
        Some((release, Arc::clone(&self.wanted)))
    }

    pub fn progress(&mut self, received: u64) {
        if let Phase::Downloading { received: shown } = &mut self.phase {
            *shown = (*shown).max(received);
        }
    }

    pub fn downloaded(&mut self) {
        if matches!(self.phase, Phase::Downloading { .. }) {
            self.phase = Phase::Installing;
        }
    }

    pub fn fail(&mut self, refusal: Refusal) {
        if !matches!(self.phase, Phase::Hidden) {
            self.phase = Phase::Failed(refusal);
        }
    }

    /// Later: away until the next launch, and a download under way stops.
    /// Not while the OS is taking it — that step has no half.
    pub fn later(&mut self) -> bool {
        if self.phase == Phase::Installing {
            return false;
        }
        self.wanted.store(false, Ordering::SeqCst);
        self.dismissed = true;
        self.phase = Phase::Hidden;
        true
    }

    /// The download's share, 0…1, while one is shown.
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        let size = self.release.as_ref()?.artifact.size.max(1);
        let received = match &self.phase {
            Phase::Downloading { received } => *received,
            Phase::Installing => size,
            _ => return None,
        };
        #[allow(clippy::cast_precision_loss, reason = "a progress bar's width")]
        Some((received.min(size) as f64 / size as f64) as f32)
    }
}

/// The card's progress line: `42% · 12.3 / 29.4 MB`.
#[must_use]
pub fn progress_text(received: u64, size: u64) -> String {
    #[allow(clippy::cast_precision_loss, reason = "a figure on a progress line")]
    let mb = |bytes: u64| bytes as f64 / 1_000_000.;
    let percent = received.min(size).saturating_mul(100) / size.max(1);
    format!("{percent}% · {:.1} / {:.1} MB", mb(received), mb(size))
}

/// Set the state up and, when this build may update, start looking.
/// Before the first window opens.
pub fn boot(cx: &mut App) {
    cx.set_global(UpdateState::default());
    #[cfg(any(test, debug_assertions, feature = "dev-fixtures"))]
    if preview(cx) {
        return;
    }
    if !super::enabled() {
        return;
    }
    crate::diag::vlog!("update", "running {}", super::current_version());
    cx.spawn(async move |cx| {
        let mut wait = FIRST_CHECK;
        #[cfg(any(test, debug_assertions, feature = "dev-fixtures"))]
        if let Some(ms) =
            crate::dev_env::var!("VELA_UPDATE_CHECK_AFTER_MS").and_then(|text| text.parse().ok())
        {
            wait = Duration::from_millis(ms);
        }
        loop {
            cx.background_executor().timer(wait).await;
            wait = RECHECK;
            let verdict = cx
                .background_executor()
                .spawn(async { super::check() })
                .await;
            let Ok(release) = verdict else { continue };
            let offered = cx.update(|cx| {
                let words = PromptWords::of(&crate::loc::Loc::from_env());
                let state = cx.global_mut::<UpdateState>();
                state.words = Some(words);
                let offered = state.offer(release);
                cx.refresh_windows();
                offered
            });
            #[cfg(any(test, debug_assertions, feature = "dev-fixtures"))]
            if offered && crate::dev_env::flag!("VELA_UPDATE_AUTO_INSTALL") {
                // The local proof's hand on Install (`scripts/update-e2e.sh`),
                // after the prompt has been on screen long enough to see.
                cx.background_executor().timer(Duration::from_secs(3)).await;
                cx.update(install);
            }
            let _ = offered;
        }
    })
    .detach();
}

/// `VELA_UPDATE_PREVIEW=offered|downloading|installing|failed`: the card in
/// one state, over a fixture release, with no feed — for screenshots.
#[cfg(any(test, debug_assertions, feature = "dev-fixtures"))]
fn preview(cx: &mut App) -> bool {
    let Some(which) = crate::dev_env::var!("VELA_UPDATE_PREVIEW") else {
        return false;
    };
    let size = 48_600_000;
    let release = Release {
        version: "0.9.9".to_owned(),
        issued_at: 0,
        notes_url: None,
        artifact: super::Artifact {
            url: "https://getvela.app/desktop/releases/VelaWallet-0.9.9-macos-arm64.zip".to_owned(),
            sha256: "00".repeat(32),
            size,
        },
    };
    let state = cx.global_mut::<UpdateState>();
    state.words = Some(PromptWords::of(&crate::loc::Loc::from_env()));
    state.offer(release);
    match which.as_str() {
        "downloading" => {
            let _ = state.start();
            state.progress(size * 42 / 100);
        }
        "installing" => {
            let _ = state.start();
            state.downloaded();
        }
        "failed" => {
            let _ = state.start();
            state.progress(size * 3 / 4);
            state.fail(Refusal::ArtifactMismatch);
        }
        _ => {}
    }
    true
}

/// Where a download is kept until the OS takes it.
fn download_path(release: &Release) -> std::path::PathBuf {
    let name = if cfg!(windows) {
        format!("VelaWallet-{}-setup.exe", release.version)
    } else {
        format!("VelaWallet-{}.zip", release.version)
    };
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("app.getvela.VelaWallet")
        .join("update")
        .join(name)
}

/// Install / Try again.
pub fn install(cx: &mut App) {
    let Some((release, wanted)) = cx.global_mut::<UpdateState>().start() else {
        return;
    };
    cx.refresh_windows();
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<u64>();
    cx.spawn(async move |cx| {
        use futures::StreamExt as _;
        let path = download_path(&release);
        let artifact = release.artifact.clone();
        let work = {
            let path = path.clone();
            let wanted = Arc::clone(&wanted);
            cx.background_executor().spawn(async move {
                let step = (artifact.size / 100).max(1);
                let last = std::cell::Cell::new(0u64);
                super::download::download(
                    &artifact,
                    &path,
                    &|received| {
                        // At most a hundred redraws, and the last byte.
                        if received - last.get() >= step || received == artifact.size {
                            last.set(received);
                            let _ = tx.unbounded_send(received);
                        }
                    },
                    &|| wanted.load(Ordering::SeqCst),
                )
            })
        };
        while let Some(received) = rx.next().await {
            cx.update(|cx| {
                cx.global_mut::<UpdateState>().progress(received);
                cx.refresh_windows();
            });
        }
        let downloaded = work.await;
        if !wanted.load(Ordering::SeqCst) {
            // Later: the card is already away.
            let _ = std::fs::remove_file(&path);
            return;
        }
        if let Err(refusal) = downloaded {
            crate::diag::vlog!("update", "download refused: {refusal:?}");
            cx.update(|cx| {
                cx.global_mut::<UpdateState>().fail(refusal);
                cx.refresh_windows();
            });
            return;
        }
        crate::diag::vlog!(
            "update",
            "download verified: {} bytes",
            release.artifact.size
        );
        cx.update(|cx| {
            cx.global_mut::<UpdateState>().downloaded();
            cx.refresh_windows();
        });
        // Never while a submit is out: the app is about to quit.
        while !crate::executor::relay::may_close(std::time::Instant::now()) {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
        }
        let handed = {
            let release = release.clone();
            let path = path.clone();
            cx.background_executor()
                .spawn(async move { hand_off(&path, &release) })
                .await
        };
        let _ = std::fs::remove_file(&path);
        match handed {
            Ok(()) => {
                crate::diag::vlog!("update", "installed {}; relaunching", release.version);
                cx.update(|cx| {
                    crate::window_state::flush();
                    cx.quit();
                });
            }
            Err(refusal) => {
                crate::diag::vlog!("update", "install refused: {refusal:?}");
                cx.update(|cx| {
                    cx.global_mut::<UpdateState>().fail(refusal);
                    cx.refresh_windows();
                });
            }
        }
    })
    .detach();
}

/// The OS's half, off the UI thread. `Ok` means the app should quit now:
/// its successor is in place and will start once this process is gone.
#[allow(
    clippy::unnecessary_wraps,
    reason = "a platform without a hand-off answers through the same shape"
)]
fn hand_off(path: &std::path::Path, release: &Release) -> Result<(), Refusal> {
    #[cfg(target_os = "macos")]
    {
        let bundle = super::install_macos::install(path, release)?;
        super::install_macos::relaunch(&bundle)
    }
    #[cfg(windows)]
    {
        use super::install_windows as win;
        let _ = release;
        let app_signed = std::env::current_exe()
            .map(|exe| win::authenticode_trusted(&exe))
            .unwrap_or(false);
        if !win::installer_acceptable(app_signed, win::authenticode_trusted(path)) {
            return Err(Refusal::SignatureMismatch);
        }
        win::hand_off(path).map_err(|error| Refusal::InstallFailed(error.to_string()))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (path, release);
        Err(Refusal::NotInstalled)
    }
}

/// Later.
pub fn later(cx: &mut App) {
    if cx.global_mut::<UpdateState>().later() {
        cx.refresh_windows();
    }
}

/// The card over `root`, when there is one to show.
pub fn overlay(root: Div, window: &Window, cx: &App) -> Div {
    let Some(state) = cx.try_global::<UpdateState>() else {
        return root;
    };
    let (Some(release), Some(words)) = (&state.release, &state.words) else {
        return root;
    };
    if state.phase == Phase::Hidden {
        return root;
    }
    let theme = Theme::of(theme::ThemeMode::detect(window));
    root.relative()
        .child(
            div()
                .absolute()
                .right(px(20.))
                .bottom(px(20.))
                .child(crate::ui::motion::card_in(
                    card(state, release, words, &theme),
                    "update-card-in",
                    crate::ui::motion::reduced(),
                )),
        )
}

fn card(
    state: &UpdateState,
    release: &Release,
    words: &PromptWords,
    theme: &Theme,
) -> gpui::Stateful<Div> {
    let versions = format!("{} → {}", state.current, release.version);
    let mut card = crate::ui::dialog::card("update-card", theme, CARD_W)
        .p(px(20.))
        .gap(px(12.))
        .a11y_dialog(words.title.clone())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(words.title.clone()),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(versions),
                ),
        );

    let shown = match &state.phase {
        Phase::Downloading { received } => Some(*received),
        Phase::Installing => Some(release.artifact.size),
        _ => None,
    };
    if let Some(received) = shown {
        card = card.child(progress(
            state.fraction().unwrap_or(0.),
            progress_text(received, release.artifact.size),
            false,
            theme,
        ));
    } else if let Phase::Failed(refusal) = &state.phase {
        card = card
            .child(progress(1., String::new(), true, theme))
            .child(refused_line(words.refusal(refusal), theme));
    }

    let actions = match &state.phase {
        Phase::Offered => Some(Some((words.install.clone(), true))),
        // A refusal another attempt would only repeat offers Later alone.
        Phase::Failed(refusal) => Some(refusal.retryable().then(|| (words.retry.clone(), true))),
        Phase::Downloading { .. } => Some(Some((words.install.clone(), false))),
        Phase::Installing | Phase::Hidden => None,
    };
    if let Some(answer) = actions {
        card = card.child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.))
                .child(button(
                    "update-later",
                    words.later.clone(),
                    false,
                    true,
                    theme,
                    |_, _, cx| {
                        later(cx);
                    },
                ))
                .when_some(answer, |row, (answer, live)| {
                    row.child(button(
                        "update-install",
                        answer,
                        true,
                        live,
                        theme,
                        |_, _, cx| {
                            install(cx);
                        },
                    ))
                }),
        );
    }
    card
}

/// Why the step was refused and what the person can do, under the red bar —
/// in the error colour, and read on its own by a screen reader.
fn refused_line(text: SharedString, theme: &Theme) -> gpui::Stateful<Div> {
    div()
        .id("update-refused")
        .role(gpui::Role::Label)
        .aria_label(text.clone())
        .text_size(theme::text_row_sub())
        .text_color(theme.error_base)
        .child(text)
}

/// The bar and the line under it. A refused step shows the bar in the
/// error colour, with no figure.
fn progress(fraction: f32, text: String, failed: bool, theme: &Theme) -> Div {
    let fill = if failed {
        theme.error_base
    } else {
        theme.fg_base
    };
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .h(px(6.))
                .w_full()
                .rounded(px(3.))
                .bg(theme.bg_sunken)
                .child(
                    div()
                        .h_full()
                        .w(relative(fraction.clamp(0., 1.)))
                        .rounded(px(3.))
                        .bg(fill),
                ),
        )
        .when(!text.is_empty(), |el| {
            el.child(
                div()
                    .font_family(theme::font_mono())
                    .text_size(theme::text_label())
                    .text_color(theme.fg_muted)
                    .child(text),
            )
        })
}

/// The card's two answers. Install is the neutral ink (it moves no money:
/// the accent is for that alone); Later is the outline. A control that is
/// not live (Install while its download runs) is dimmed and takes no click.
fn button(
    id: &'static str,
    label: SharedString,
    ink: bool,
    live: bool,
    theme: &Theme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div> {
    let base = div()
        .id(id)
        .a11y_button(label.clone())
        .min_h(px(36.))
        .px(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(10.))
        .text_size(theme::text_row_sub())
        .font_weight(FontWeight::SEMIBOLD)
        .child(label);
    let base = if ink {
        base.bg(theme.fg_base).text_color(theme.fg_inverse)
    } else {
        base.border_1()
            .border_color(theme.border_strong)
            .text_color(theme.fg_base)
    };
    if live {
        base.cursor_pointer()
            .hover(|el| el.opacity(0.9))
            .on_click(on_click)
    } else {
        base.opacity(theme::OPACITY_DISABLED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Release {
        Release {
            version: "0.9.9".to_owned(),
            issued_at: 1,
            notes_url: None,
            artifact: super::super::Artifact {
                url: "https://getvela.app/x.zip".to_owned(),
                sha256: "00".repeat(32),
                size: 1_000,
            },
        }
    }

    #[test]
    fn offer_install_progress_install() {
        let mut state = UpdateState::default();
        assert!(state.start().is_none(), "nothing offered");
        assert!(state.offer(release()));
        assert_eq!(state.phase, Phase::Offered);
        let (_, wanted) = state.start().unwrap_or_else(|| unreachable!());
        assert!(wanted.load(Ordering::SeqCst));
        state.progress(420);
        state.progress(100);
        assert_eq!(
            state.phase,
            Phase::Downloading { received: 420 },
            "never backwards"
        );
        assert_eq!(state.fraction(), Some(0.42));
        assert!(state.start().is_none(), "one download at a time");
        state.downloaded();
        assert_eq!(state.phase, Phase::Installing);
        assert_eq!(state.fraction(), Some(1.));
        assert!(!state.later(), "the swap has no half");
        assert!(!state.offer(release()), "not over an install");
    }

    #[test]
    fn a_refusal_can_be_tried_again_and_later_puts_it_away() {
        let mut state = UpdateState::default();
        state.offer(release());
        let (_, first) = state.start().unwrap_or_else(|| unreachable!());
        state.fail(Refusal::ArtifactMismatch);
        assert_eq!(state.phase, Phase::Failed(Refusal::ArtifactMismatch));
        let (_, second) = state.start().unwrap_or_else(|| unreachable!());
        assert_eq!(state.phase, Phase::Downloading { received: 0 });
        assert!(state.later());
        assert!(!second.load(Ordering::SeqCst), "Later stops the download");
        assert!(
            !Arc::ptr_eq(&first, &second),
            "each attempt has its own flag: a stopped one stops no other"
        );
        assert_eq!(state.phase, Phase::Hidden);
        assert!(!state.offer(release()), "not again this launch");
    }

    /// A refusal another attempt would only repeat (this copy is not an
    /// installed app; the update is not signed as this app) offers no Try
    /// again: Install does nothing, Later still puts it away.
    #[test]
    fn a_refusal_that_cannot_go_differently_is_not_tried_again() {
        for refusal in [
            Refusal::NotInstalled,
            Refusal::SignatureMismatch,
            Refusal::VersionMismatch,
        ] {
            let mut state = UpdateState::default();
            state.offer(release());
            let _ = state.start();
            state.fail(refusal.clone());
            assert!(state.start().is_none(), "{refusal:?}");
            assert_eq!(state.phase, Phase::Failed(refusal));
            assert!(state.later());
            assert_eq!(state.phase, Phase::Hidden);
        }
        let mut state = UpdateState::default();
        state.offer(release());
        let _ = state.start();
        state.fail(Refusal::InstallFailed("EACCES".to_owned()));
        assert!(
            state.start().is_some(),
            "an OS step that failed may not fail again"
        );
    }

    #[test]
    fn the_progress_line() {
        assert_eq!(progress_text(0, 48_600_000), "0% · 0.0 / 48.6 MB");
        assert_eq!(
            progress_text(20_412_000, 48_600_000),
            "42% · 20.4 / 48.6 MB"
        );
        assert_eq!(
            progress_text(48_600_000, 48_600_000),
            "100% · 48.6 / 48.6 MB"
        );
        assert_eq!(progress_text(5, 0), "0% · 0.0 / 0.0 MB");
    }
}
