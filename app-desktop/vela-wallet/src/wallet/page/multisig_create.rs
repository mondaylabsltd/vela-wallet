//! The create sheet (AUDIT M1; the web's `CreateMultisigSheet.svelte`):
//! a name and its members (this device's Vela accounts first, then an
//! address or an ENS name, each outside member's kind declared), how many
//! must approve, and a review with the address-to-be before one passkey
//! proves it. Then the created multisig, its record said as it stands.
//!
//! Everything decided is the accounts machine's (`new_draft`, `draft`,
//! `check_networks`, `keep_draft` / `discard_draft`, `read_kinds` /
//! `pick_kind`, `resolve_ens`, `create`) or a core rule (`member_problem`,
//! `valid_name`, `networks.likely` / `networks.firstDefault`): this file
//! holds only what the person typed and picked, and draws the machine's
//! `create` view.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use vela_core::app::multisig_accounts::{
    self as ms_accounts, AccountRow, CreateChoice, CreateDraft, EnsAnswer, KindAsk, Phase,
};
use vela_core::app::multisig_members::{MemberProblem, member_problem};
use vela_core::app::multisig_shell::MemberInfo;
use vela_core::app::multisig_signers::{KindChoice, KindNote, MemberKind};

use super::multisig::{Tone as MsTone, ms_label, ms_note, network, same};
use super::*;
use crate::multisig::words;

/// Where the create sheet is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CreateStep {
    #[default]
    Members,
    Threshold,
    Review,
    /// The passkey answered: the multisig is kept here.
    Created,
}

/// What the person typed and picked on the create sheet. What the machine
/// holds (the draft's address, the networks' verdicts, the kept draft, the
/// members' kinds, the outcome) is read from its view, never copied here.
pub(crate) struct CreateForm {
    pub step: CreateStep,
    /// The members, in their order (the order is part of the address).
    pub owners: Vec<String>,
    /// What the person said each member is called, by lowercase address.
    pub said: BTreeMap<String, String>,
    pub name: String,
    /// Next was pressed with no name: the field says so.
    pub name_asked: bool,
    pub typed: String,
    pub typed_name: String,
    pub typed_error: Option<String>,
    /// The ENS name asked about (its answer is the machine's `add.ens`).
    pub ens: Option<String>,
    pub threshold: u64,
    /// The member count the threshold was last set for.
    pub threshold_for: usize,
    pub chain_id: Option<u64>,
    /// The person picked the network: what lands later never picks over them.
    pub chose: bool,
    pub all_networks: bool,
    pub creator: Option<String>,
    /// The machine's kept draft was looked at (once).
    pub draft_read: bool,
    /// The sheet opened on a kept draft.
    pub resumed: bool,
    /// `address:chains` already asked of the machine (`read_kinds`).
    pub kinds_asked: BTreeSet<String>,
    /// Create was pressed: the machine's phase is this sheet's.
    pub pressed: bool,
    pub name_focus: FocusHandle,
    pub address_focus: FocusHandle,
    pub member_name_focus: FocusHandle,
}

impl CreateForm {
    pub(crate) fn new(owners: Vec<String>, cx: &mut Context<WalletPage>) -> Self {
        Self {
            step: CreateStep::Members,
            owners,
            said: BTreeMap::new(),
            name: String::new(),
            name_asked: false,
            typed: String::new(),
            typed_name: String::new(),
            typed_error: None,
            ens: None,
            threshold: 1,
            threshold_for: 0,
            chain_id: None,
            chose: false,
            all_networks: false,
            creator: None,
            draft_read: false,
            resumed: false,
            kinds_asked: BTreeSet::new(),
            pressed: false,
            name_focus: cx.focus_handle(),
            address_focus: cx.focus_handle(),
            member_name_focus: cx.focus_handle(),
        }
    }

    /// Nothing typed yet: a kept draft may be opened over it.
    fn blank(&self) -> bool {
        self.name.trim().is_empty() && self.owners.len() <= 1
    }

    /// What the sheet holds, as the machine keeps it (r7-9).
    fn draft(&self) -> CreateDraft {
        CreateDraft {
            owners: self.owners.clone(),
            members: self
                .owners
                .iter()
                .filter_map(|o| {
                    self.said.get(&o.to_ascii_lowercase()).map(|name| {
                        (
                            o.clone(),
                            MemberInfo {
                                name: Some(name.clone()),
                                ..MemberInfo::default()
                            },
                        )
                    })
                })
                .collect(),
            name: self.name.clone(),
            threshold: self.threshold,
            chain_id: if self.chose { self.chain_id } else { None },
        }
    }
}

// -- the pure rules this sheet draws by ----------------------------------------

/// A sensible first threshold for `members` (the web's `toThreshold`): all
/// of two, else all but one — one lost passkey is survivable.
pub(crate) fn default_threshold(members: usize) -> u64 {
    let n = u64::try_from(members).unwrap_or(u64::MAX).max(1);
    if n <= 2 { n } else { n - 1 }
}

/// The stepper's next value: one more or one fewer, never below 1 nor above
/// the members.
pub(crate) fn step_threshold(threshold: u64, members: usize, up: bool) -> u64 {
    let n = u64::try_from(members).unwrap_or(u64::MAX).max(1);
    let next = if up {
        threshold.saturating_add(1)
    } else {
        threshold.saturating_sub(1)
    };
    next.clamp(1, n)
}

/// What is wrong with a name, said of the name (r4a-14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NameProblem {
    Needed,
    /// Over the registry's bytes (its size in bytes).
    TooLong(usize),
    Invalid,
}

/// The core's name rule (`valid_name`), on the name as it would be kept.
pub(crate) fn name_problem(name: &str) -> Option<NameProblem> {
    let clean = name.trim();
    if clean.is_empty() {
        return Some(NameProblem::Needed);
    }
    if clean.len() > vela_core::multisig::MAX_NAME_BYTES {
        return Some(NameProblem::TooLong(clean.len()));
    }
    (!vela_core::multisig::valid_name(clean)).then_some(NameProblem::Invalid)
}

/// What the threshold means, in which words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThresholdNote {
    Solo,
    All,
    One,
    /// This many can lose access and the funds still move.
    Spare(usize),
}

pub(crate) fn threshold_note(threshold: u64, members: usize) -> ThresholdNote {
    let n = u64::try_from(members).unwrap_or(u64::MAX);
    if members <= 1 {
        ThresholdNote::Solo
    } else if threshold >= n {
        ThresholdNote::All
    } else if threshold <= 1 {
        ThresholdNote::One
    } else {
        ThresholdNote::Spare(usize::try_from(n - threshold).unwrap_or(usize::MAX))
    }
}

/// The networks in view (the web's `NetworkChoice`): the first `shown`, the
/// one picked kept in view wherever it is; all of them once asked, or when
/// only one more would be hidden.
pub(crate) fn visible_networks(
    networks: &[u64],
    chosen: Option<u64>,
    all: bool,
    shown: usize,
) -> Vec<u64> {
    if all || networks.len() <= shown + 1 {
        return networks.to_vec();
    }
    let mut first: Vec<u64> = networks[..shown].to_vec();
    if let Some(pick) = chosen.filter(|c| networks.contains(c) && !first.contains(c)) {
        first.push(pick);
    }
    first
}

/// A pure rule of the core's (`multisig_link::call_json`), its answer read.
fn rule<T: serde::de::DeserializeOwned>(method: &str, args: &Value) -> Option<T> {
    vela_core::multisig_link::call_json(method, &args.to_string())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// A member's short form, as the core's naming rule writes it
/// (`names.short`) — what `names.label` falls back to.
pub(crate) fn names_short(address: &str) -> String {
    rule("names.short", &json!({ "address": address })).unwrap_or_else(|| words::short(address))
}

/// `networks`, the likely ones first (`networks.likely`).
pub(crate) fn likely_networks(networks: &[u64], hints: &Value) -> Vec<u64> {
    rule(
        "networks.likely",
        &json!({"networks": networks, "hints": hints}),
    )
    .unwrap_or_else(|| networks.to_vec())
}

/// The first network, only when something says which (`networks.firstDefault`).
pub(crate) fn first_network(networks: &[u64], hints: &Value) -> Option<u64> {
    rule::<Option<u64>>(
        "networks.firstDefault",
        &json!({"networks": networks, "hints": hints}),
    )
    .flatten()
}

/// The networks the multisigs here were first used on, newest first
/// (`networks.lastUsed`).
pub(crate) fn last_used(rows: &[AccountRow]) -> Vec<u64> {
    let accounts: Vec<Value> = rows
        .iter()
        .map(|r| json!({"chainIds": r.account.chain_ids, "addedMs": r.account.added_ms}))
        .collect();
    rule("networks.lastUsed", &json!({ "accounts": accounts })).unwrap_or_default()
}

/// What a member typed cannot be, in the corpus' words.
fn problem_words(loc: &Loc, problem: MemberProblem) -> String {
    match problem {
        MemberProblem::InvalidAddress => words::t(loc, "send.invalidAddress"),
        MemberProblem::Checksum => words::t(loc, "send.invalidChecksum"),
        MemberProblem::NotMember => words::t(loc, "create.notMember"),
        MemberProblem::Duplicate => words::t(loc, "create.duplicate"),
    }
}

/// What an ENS answer says, and the address it gives when it found one.
pub(crate) fn ens_words(loc: &Loc, answer: &EnsAnswer) -> (String, Option<String>) {
    match answer {
        EnsAnswer::Found { name, address } => (
            words::tv(loc, "create.ensResolved", &[("name", name)]),
            Some(vela_core::multisig_link::eip55(address)),
        ),
        EnsAnswer::None { name } => (
            words::tv(loc, "create.ensNotFound", &[("name", name)]),
            None,
        ),
        EnsAnswer::Unavailable { name } => (
            words::tv(loc, "create.ensUnavailable", &[("name", name)]),
            None,
        ),
        EnsAnswer::Unsupported { .. } => (words::t(loc, "create.ensUnsupported"), None),
    }
}

fn ens_name(answer: &EnsAnswer) -> &str {
    match answer {
        EnsAnswer::Found { name, .. }
        | EnsAnswer::None { name }
        | EnsAnswer::Unavailable { name }
        | EnsAnswer::Unsupported { name } => name,
    }
}

/// The ENS answer for `asked`, once it is here.
pub(crate) fn ens_answer<'a>(
    view: &'a ms_accounts::AddView,
    asked: Option<&str>,
) -> Option<&'a EnsAnswer> {
    let asked = asked?;
    view.ens
        .as_ref()
        .filter(|a| ens_name(a).eq_ignore_ascii_case(asked))
}

impl WalletPage {
    /// `VELA_MULTISIG_SHEET=create|add` (developer builds only — the
    /// `dev_env::var!` read compiles to `None` in a release build): the
    /// signed-in page opens with that multisig sheet up, for a screenshot
    /// pass that cannot click to it.
    ///
    /// First, what a sheet carried across the page being rebuilt for the
    /// multisig it just made the one shown ([`super::multisig_links::Carry`]).
    pub(crate) fn pin_multisig_sheet(&mut self, cx: &mut Context<Self>) {
        self.ms_restore_carried(cx);
        // Once a run: the page built again for a multisig just made is not
        // pinned over.
        static PINNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if PINNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        match crate::dev_env::var!("VELA_MULTISIG_SHEET").as_deref() {
            Some("create") => self.open_multisig_create(cx),
            Some("add") => self.open_multisig_add(None, cx),
            _ => {}
        }
    }

    /// The sheet opened: a fresh form seeded with the account shown (the
    /// machine's `new_draft` was sent), and which of the space's networks
    /// hold the Safe contracts (`check_networks`).
    pub(crate) fn ms_create_opened(&mut self, cx: &mut Context<Self>) {
        let session = session::view(cx);
        let seed: Vec<String> = if session.address.is_empty() {
            session
                .accounts
                .first()
                .map(|r| r.account.address.clone())
                .into_iter()
                .collect()
        } else {
            vec![session.address.clone()]
        };
        self.multisig.create_form = Some(CreateForm::new(seed, cx));
        let chain_ids: Vec<u64> = crate::network_space::space_chain_ids()
            .into_iter()
            .map(u64::from)
            .collect();
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.accounts(ms_accounts::Event::CheckNetworks { chain_ids }, cx);
        });
    }

    /// The create sheet closed: what it holds is kept (r7-9) by the
    /// machine (`keep_draft` on every edit), so closing loses nothing.
    pub(crate) fn close_multisig_create(&mut self, cx: &mut Context<Self>) {
        super::multisig_links::Carry::take_create(cx);
        self.multisig.create = false;
        self.multisig.create_form = None;
        cx.notify();
    }

    /// Every edit is the machine's to keep (`keep_draft`), and the draft's
    /// address follows the members, the rule and the name (`draft`).
    fn ms_create_sync(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.multisig.create_form.as_ref() else {
            return;
        };
        if !form.draft_read || form.step == CreateStep::Created {
            return;
        }
        let keep = form.draft();
        let draft = (!form.owners.is_empty() && name_problem(&form.name).is_none()).then(|| {
            ms_accounts::Event::Draft {
                owners: form.owners.clone(),
                threshold: form.threshold,
                name: form.name.trim().to_owned(),
            }
        });
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            if let Some(draft) = draft {
                host.accounts(draft, cx);
            }
            host.accounts(ms_accounts::Event::KeepDraft { draft: keep }, cx);
        });
        cx.notify();
    }

    /// Change the form, then hand the change to the machine.
    fn ms_create_edit(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut CreateForm)) {
        if let Some(form) = self.multisig.create_form.as_mut() {
            edit(form);
        }
        self.ms_create_sync(cx);
    }

    /// r7-9: start over — the kept draft goes, the sheet is fresh.
    fn ms_create_discard(&mut self, cx: &mut Context<Self>) {
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.accounts(ms_accounts::Event::DiscardDraft, cx)
        });
        let session = session::view(cx);
        let seed: Vec<String> = if session.address.is_empty() {
            session
                .accounts
                .first()
                .map(|r| r.account.address.clone())
                .into_iter()
                .collect()
        } else {
            vec![session.address.clone()]
        };
        if let Some(form) = self.multisig.create_form.as_mut() {
            form.owners = seed;
            form.said.clear();
            form.name.clear();
            form.threshold = 1;
            form.threshold_for = 0;
            form.name_asked = false;
            form.chose = false;
            form.chain_id = None;
            form.resumed = false;
        }
        cx.notify();
    }

    /// Add `address` (any case) as a member, checksummed, with what is said
    /// of it — or say why it cannot be one.
    fn ms_create_add_member(
        &mut self,
        address: &str,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let loc = self.ms_loc();
        let Some(form) = self.multisig.create_form.as_mut() else {
            return false;
        };
        if let Some(problem) = member_problem(address, &form.owners) {
            form.typed_error = Some(problem_words(&loc, problem));
            cx.notify();
            return false;
        }
        let member = vela_core::multisig_link::eip55(address.trim());
        let clean: String = name
            .unwrap_or_default()
            .trim()
            .chars()
            .take(vela_core::multisig_link::MEMBER_NAME_MAX)
            .collect();
        if !clean.is_empty() {
            form.said.insert(member.to_ascii_lowercase(), clean);
        }
        form.owners.push(member);
        form.typed.clear();
        form.typed_name.clear();
        form.typed_error = None;
        form.ens = None;
        self.ms_create_sync(cx);
        true
    }

    /// The typed member: an ENS name is read on Ethereum first (shown as its
    /// address, which is what is added); an address is added.
    fn ms_create_add_typed(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.multisig.create_form.as_mut() else {
            return;
        };
        let value = form.typed.trim().to_owned();
        if value.is_empty() {
            return;
        }
        if ms_accounts::looks_like_ens(&value) {
            form.ens = Some(value.clone());
            form.typed_error = None;
            let host = self.ms_host(cx);
            host.update(cx, |host, cx| {
                host.accounts(ms_accounts::Event::ResolveEns { name: value }, cx)
            });
            cx.notify();
            return;
        }
        let name = Some(form.typed_name.clone());
        self.ms_create_add_member(&value, name, cx);
    }

    fn ms_create_toggle(&mut self, address: String, cx: &mut Context<Self>) {
        self.ms_create_edit(cx, |form| {
            if form.owners.iter().any(|o| same(o, &address)) {
                form.owners.retain(|o| !same(o, &address));
            } else {
                form.owners.push(address);
            }
        });
    }

    /// Next from the members: the name is asked for; the threshold starts
    /// sensible, unless the person already set it for these members.
    fn ms_create_to_threshold(&mut self, locals: usize, cx: &mut Context<Self>) {
        let Some(form) = self.multisig.create_form.as_mut() else {
            return;
        };
        if form.owners.is_empty() || locals == 0 || name_problem(&form.name).is_some() {
            form.name_asked = true;
            cx.notify();
            return;
        }
        if form.threshold_for != form.owners.len() {
            form.threshold = default_threshold(form.owners.len());
            form.threshold_for = form.owners.len();
        }
        form.step = CreateStep::Threshold;
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        self.ms_create_sync(cx);
    }

    /// The draft kept from before opens the sheet — once the machine has
    /// read the store, and only over a sheet nobody has typed in yet.
    fn ms_create_read_kept(
        &mut self,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.multisig.create_form.as_mut() else {
            return;
        };
        if form.draft_read || !view.loaded {
            return;
        }
        form.draft_read = true;
        if let Some(kept) = view.create.kept.clone().filter(|_| form.blank()) {
            form.owners = kept.owners.clone();
            form.said = kept
                .members
                .iter()
                .filter_map(|(a, i)| i.name.clone().map(|n| (a.to_ascii_lowercase(), n)))
                .collect();
            form.name = kept.name.clone();
            form.threshold = kept.threshold.max(1);
            form.threshold_for = kept.owners.len();
            if let Some(chain_id) = kept.chain_id {
                form.chain_id = Some(chain_id);
                form.chose = true;
            }
            form.resumed = true;
        }
        let page = cx.entity();
        cx.defer(move |cx| page.update(cx, |this, cx| this.ms_create_sync(cx)));
    }

    /// The create sheet: the machine's `create` view, and the person's form.
    pub(crate) fn ms_create_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.multisig.create_form.is_none() {
            self.ms_create_opened(cx);
        }
        let loc = self.ms_loc();
        let host = self.ms_host(cx);
        let view = host.read(cx).machines().accounts_view.clone();
        self.ms_create_read_kept(&view, cx);

        // The networks offered: the space's, but one found without the Safe
        // contracts; the likely ones first (r4a-14, r5reg-19).
        let space: Vec<u64> = crate::network_space::space_chain_ids()
            .into_iter()
            .map(u64::from)
            .collect();
        let without: Vec<u64> = view
            .create
            .networks
            .iter()
            .filter(|(_, v)| v.get("kind").and_then(Value::as_str) == Some("missing"))
            .map(|(id, _)| *id)
            .collect();
        let candidates: Vec<u64> = space
            .into_iter()
            .filter(|id| !without.contains(id))
            .collect();
        let held: Vec<u64> = {
            let tokens = crate::network_space::balance_view(cx).tokens;
            rule("networks.held", &json!({ "tokens": tokens })).unwrap_or_default()
        };
        let hints = json!({
            "here": self.chain_filter.map(u64::from).into_iter().collect::<Vec<_>>(),
            "used": last_used(&view.accounts),
            "held": held,
        });
        let offered = likely_networks(&candidates, &hints);
        let likely = first_network(&offered, &hints);

        let locals: Vec<(String, String)> = session::view(cx)
            .accounts
            .iter()
            .map(|r| {
                (
                    r.account.address.clone(),
                    crate::multisig::shown::name_or(&r.account.address, &r.account.name),
                )
            })
            .collect();
        {
            let form = self.multisig.create_form.as_mut()?;
            if form.chain_id.is_some_and(|c| !offered.contains(&c)) {
                form.chain_id = None;
            }
            if !form.chose && form.chain_id.is_none() {
                form.chain_id = likely;
            }
            // The passkey answered: the multisig is kept here.
            if form.pressed && view.create.address.is_some() && form.step != CreateStep::Created {
                form.step = CreateStep::Created;
                self.multisig.scroll = crate::ui::SmoothScroll::new();
            }
        }
        self.ms_create_ask_kinds(&offered, &locals, cx);

        let form = self.multisig.create_form.as_ref()?;
        let step = form.step;
        let body = match step {
            CreateStep::Members => self.ms_create_members(theme, window, &view, &locals, cx),
            CreateStep::Threshold => self.ms_create_threshold(theme, cx),
            CreateStep::Review => self.ms_create_review(theme, &view, &locals, &offered, cx),
            CreateStep::Created => self.ms_create_created(theme, &view, cx),
        };
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        Some(
            crate::ui::dialog::dialog(
                "ms-create",
                theme,
                window,
                words::t(&loc, "create.title").into(),
                None,
                close_icon,
                body,
                &self.multisig.scroll,
                cx.listener(|this, _, _, cx| this.close_multisig_create(cx)),
            )
            .into_any_element(),
        )
    }

    /// r7: each member not on this device is read once for the networks
    /// offered (`read_kinds`): its kind choice is the machine's.
    fn ms_create_ask_kinds(
        &mut self,
        offered: &[u64],
        locals: &[(String, String)],
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.multisig.create_form.as_mut() else {
            return;
        };
        let chains = offered
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut asks = Vec::new();
        for owner in &form.owners {
            if locals.iter().any(|(a, _)| same(a, owner)) {
                continue;
            }
            let key = format!("{}:{chains}", owner.to_ascii_lowercase());
            if form.kinds_asked.insert(key) {
                asks.push(KindAsk {
                    address: owner.clone(),
                    chain_ids: offered.to_vec(),
                });
            }
        }
        if asks.is_empty() {
            return;
        }
        let host = self.ms_host(cx);
        cx.defer(move |cx| {
            host.update(cx, |host, cx| {
                host.accounts(ms_accounts::Event::ReadKinds { members: asks }, cx)
            });
        });
    }

    /// How a member is named on the sheet: this device's account, what the
    /// person said, else what the multisigs here keep (`names.label`).
    fn ms_create_label(
        &self,
        address: &str,
        locals: &[(String, String)],
        view: &ms_accounts::MultisigAccountsView,
    ) -> String {
        let own = locals
            .iter()
            .find(|(a, _)| same(a, address))
            .map(|(_, n)| n.clone());
        let kept = self
            .multisig
            .create_form
            .as_ref()
            .and_then(|f| f.said.get(&address.to_ascii_lowercase()).cloned())
            .or_else(|| {
                view.known_members
                    .get(&address.to_ascii_lowercase())
                    .and_then(|m| m.name.clone())
            });
        rule::<String>(
            "names.label",
            &json!({"address": address, "own": own, "kept": kept}),
        )
        .unwrap_or_else(|| words::short(address))
    }

    // -- step 1: the name and the members --------------------------------------

    fn ms_create_members(
        &mut self,
        theme: &Theme,
        window: &Window,
        view: &ms_accounts::MultisigAccountsView,
        locals: &[(String, String)],
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let Some(form) = self.multisig.create_form.as_ref() else {
            return div();
        };
        let (name, typed, typed_name) = (
            form.name.clone(),
            form.typed.clone(),
            form.typed_name.clone(),
        );
        let (name_focus, address_focus, member_focus) = (
            form.name_focus.clone(),
            form.address_focus.clone(),
            form.member_name_focus.clone(),
        );
        let owners = form.owners.clone();
        let resumed = form.resumed;
        let typed_error = form.typed_error.clone();
        let name_asked = form.name_asked;
        let ens = ens_answer(&view.add, form.ens.as_deref()).cloned();
        let ens_busy = form.ens.is_some() && ens.is_none();
        let local_count = owners
            .iter()
            .filter(|o| locals.iter().any(|(a, _)| same(a, o)))
            .count();

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(crate::ui::dialog::dialog_body(
                theme,
                words::t(&loc, "create.intro"),
            ));
        if resumed {
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(words::t(&loc, "create.draftKept")),
                    )
                    .child(
                        text_action(
                            "ms-create-discard",
                            theme,
                            &mut self.icons,
                            None,
                            words::t(&loc, "create.discardDraft").into(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.ms_create_discard(cx))),
                    ),
            );
        }

        // The name: part of the address, every member sees it.
        let problem = name_problem(&name);
        let name_error = match problem {
            Some(NameProblem::Needed) if name_asked => Some(words::t(&loc, "create.nameNeeded")),
            Some(NameProblem::TooLong(bytes)) => Some(words::tv(
                &loc,
                "create.nameTooLong",
                &[
                    ("bytes", &bytes.to_string()),
                    ("limit", &vela_core::multisig::MAX_NAME_BYTES.to_string()),
                ],
            )),
            Some(NameProblem::Invalid) => Some(words::t(&loc, "create.nameInvalid")),
            _ => None,
        };
        let bytes = name.trim().len();
        let limit = vela_core::multisig::MAX_NAME_BYTES;
        let name_strings = crate::ui::NameFieldStrings {
            label: words::t(&loc, "create.name").into(),
            placeholder: words::t(&loc, "create.namePlaceholder").into(),
            helper: if name_error.is_some() {
                SharedString::default()
            } else {
                words::t(&loc, "create.nameHint").into()
            },
            too_long_hint: name_error.clone().unwrap_or_default().into(),
        };
        let page = cx.entity();
        body = body.child(crate::ui::text_field(
            "ms-create-name",
            theme,
            &name_strings,
            &name,
            name_error.is_some(),
            false,
            &name_focus,
            window,
            move |text: String, _: &mut Window, cx: &mut gpui::App| {
                page.update(cx, |this: &mut Self, cx| {
                    this.ms_create_edit(cx, |form| form.name = text);
                });
            },
        ));
        // S4: the byte count only where it matters — near or over the limit.
        if bytes + 8 >= limit {
            body = body.child(
                div()
                    .text_size(theme::text_label())
                    .text_color(if bytes > limit {
                        theme.error_base
                    } else {
                        theme.fg_subtle
                    })
                    .child(words::tv(
                        &loc,
                        "create.nameBytes",
                        &[("bytes", &bytes.to_string()), ("limit", &limit.to_string())],
                    )),
            );
        }

        // This device's Vela accounts, offered first.
        body = body.child(ms_label(theme, &words::t(&loc, "create.yourAccounts")));
        for (i, (address, account_name)) in locals.iter().enumerate() {
            let on = owners.iter().any(|o| same(o, address));
            let label = if account_name.is_empty() {
                words::short(address)
            } else {
                account_name.clone()
            };
            let target = address.clone();
            let mut row = div()
                .id(ElementId::from(("ms-create-own", i)))
                .a11y_toggle(SharedString::from(label.clone()), on)
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(8.))
                .py(px(8.))
                .rounded(px(10.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    address,
                    24.,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .truncate()
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                // r14-11a: a short address is one line, never
                                // broken after its "…".
                                .whitespace_nowrap()
                                .child(crate::wallet::live::shorten_address(address)),
                        ),
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.ms_create_toggle(target.clone(), cx)),
                );
            row = row.child(
                div()
                    .size(px(20.))
                    .flex_none()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if on {
                        theme.fg_base
                    } else {
                        theme.border_strong
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(on, |el| {
                        el.child(icon_img(
                            &mut self.icons,
                            Icon::Check,
                            false,
                            theme.fg_base,
                            14.,
                        ))
                    }),
            );
            body = body.child(row);
        }

        // A member by address or ENS name, with the name the person knows them by.
        body = body.child(ms_label(theme, &words::t(&loc, "create.byAddress")));
        let address_strings = crate::ui::NameFieldStrings {
            label: SharedString::default(),
            placeholder: words::t(&loc, "create.addressPlaceholder").into(),
            helper: SharedString::default(),
            too_long_hint: SharedString::default(),
        };
        let member_strings = crate::ui::NameFieldStrings {
            label: SharedString::default(),
            placeholder: words::t(&loc, "create.memberNamePlaceholder").into(),
            helper: SharedString::default(),
            too_long_hint: SharedString::default(),
        };
        let page = cx.entity();
        let address_field = crate::ui::text_field(
            "ms-create-address",
            theme,
            &address_strings,
            &typed,
            typed_error.is_some(),
            false,
            &address_focus,
            window,
            move |text: String, _: &mut Window, cx: &mut gpui::App| {
                page.update(cx, |this: &mut Self, cx| {
                    if let Some(form) = this.multisig.create_form.as_mut() {
                        form.typed = text;
                        form.typed_error = None;
                        form.ens = None;
                    }
                    cx.notify();
                });
            },
        );
        let page = cx.entity();
        let member_field = crate::ui::text_field(
            "ms-create-member-name",
            theme,
            &member_strings,
            &typed_name,
            false,
            false,
            &member_focus,
            window,
            move |text: String, _: &mut Window, cx: &mut gpui::App| {
                page.update(cx, |this: &mut Self, cx| {
                    if let Some(form) = this.multisig.create_form.as_mut() {
                        form.typed_name = text;
                    }
                    cx.notify();
                });
            },
        );
        let add_state = if ens_busy {
            crate::ui::ButtonState::Busy
        } else {
            crate::ui::ButtonState::from_enabled(!typed.trim().is_empty())
        };
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                // Enter adds the member typed.
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter" {
                        this.ms_create_add_typed(cx);
                        cx.stop_propagation();
                    }
                }))
                .child(address_field)
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap(px(8.))
                        .child(div().flex_1().min_w(px(0.)).child(member_field))
                        .child(
                            crate::ui::vela_button_state(
                                "ms-create-add-member",
                                crate::ui::ButtonVariant::Secondary,
                                words::t(&loc, "create.addMember").into(),
                                add_state,
                                theme,
                                cx.listener(|this, _, _, cx| this.ms_create_add_typed(cx)),
                            )
                            .w_auto(),
                        ),
                ),
        );
        if let Some(error) = &typed_error {
            body = body.child(ms_note(theme, error, MsTone::Danger));
        }
        if let Some(answer) = &ens {
            // r4D-5: the name's address, in full and first; the name is only its label.
            let (line, found) = ens_words(&loc, answer);
            let mut card = div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(10.))
                .bg(theme.bg_sunken)
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(crate::ui::prose(line)),
                );
            if let Some(address) = found {
                let label = typed_name.trim().to_owned();
                let ens_label = ens_name(answer).to_owned();
                card = card
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(crate::ui::address_text(address.clone())),
                    )
                    .child(
                        crate::ui::vela_button(
                            "ms-create-add-ens",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "create.addMember").into(),
                            theme,
                            cx.listener(move |this, _, _, cx| {
                                let name = if label.is_empty() {
                                    ens_label.clone()
                                } else {
                                    label.clone()
                                };
                                this.ms_create_add_member(&address, Some(name), cx);
                            }),
                        )
                        .w_auto(),
                    );
            }
            body = body.child(card);
        }

        // The members, in their order.
        if !owners.is_empty() {
            body = body.child(ms_label(
                theme,
                &format!("{} · {}", words::t(&loc, "create.members"), owners.len()),
            ));
            for (i, owner) in owners.iter().enumerate() {
                let label = self.ms_create_label(owner, locals, view);
                let local = locals.iter().any(|(a, _)| same(a, owner));
                // This device's accounts read as the list above writes them.
                let short = if local {
                    crate::wallet::live::shorten_address(owner)
                } else {
                    names_short(owner)
                };
                let doomed = owner.clone();
                let mut row = div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(theme.divider)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .w(px(16.))
                                    .text_size(theme::text_label())
                                    .text_color(theme.fg_subtle)
                                    .child((i + 1).to_string()),
                            )
                            .child(crate::wallet::components::identicon_avatar(
                                &mut self.identicons,
                                owner,
                                24.,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(theme::text_row_title())
                                            .text_color(theme.fg_base)
                                            .truncate()
                                            .child(label.clone()),
                                    )
                                    .when(label != short, |el| {
                                        el.child(
                                            div()
                                                .text_size(theme::text_label())
                                                .text_color(theme.fg_subtle)
                                                .child(short.clone()),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .id(ElementId::from(("ms-create-remove", i)))
                                    .a11y_button(SharedString::from(format!(
                                        "{} · {label}",
                                        words::t(&loc, "action.removeOwner")
                                    )))
                                    .p(px(4.))
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .hover(|el| el.bg(theme.bg_sunken))
                                    .child(icon_img(
                                        &mut self.icons,
                                        Icon::X,
                                        false,
                                        theme.fg_subtle,
                                        16.,
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let doomed = doomed.clone();
                                        this.ms_create_edit(cx, |form| {
                                            form.owners.retain(|o| !same(o, &doomed))
                                        });
                                    })),
                            ),
                    );
                if let Some(choice) = view
                    .member_kinds
                    .get(&owner.to_ascii_lowercase())
                    .copied()
                    .filter(|_| !local)
                {
                    row = row.child(div().pl(px(26.)).child(self.ms_kind_choice(
                        theme,
                        "ms-create-kind",
                        i,
                        owner,
                        &label,
                        choice,
                        cx,
                    )));
                }
                body = body.child(row);
            }
            body = body.child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(crate::ui::prose(words::t(&loc, "create.order"))),
            );
        }
        if owners.len() == 1 && local_count == 1 {
            body = body.child(ms_note(
                theme,
                &words::t(&loc, "create.solo"),
                MsTone::Neutral,
            ));
        }
        if !owners.is_empty() && local_count == 0 {
            body = body.child(ms_note(
                theme,
                &words::t(&loc, "create.needLocal"),
                MsTone::Danger,
            ));
        }
        let enabled = !owners.is_empty() && local_count > 0;
        body.child(
            crate::ui::dialog::dialog_actions().child(
                crate::ui::vela_button_state(
                    "ms-create-next",
                    crate::ui::ButtonVariant::Secondary,
                    words::t(&loc, "create.next").into(),
                    crate::ui::ButtonState::from_enabled(enabled),
                    theme,
                    cx.listener(move |this, _, _, cx| this.ms_create_to_threshold(local_count, cx)),
                )
                .w_auto(),
            ),
        )
    }

    // -- step 2: how many must approve -------------------------------------------

    fn ms_create_threshold(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let loc = self.ms_loc();
        let Some(form) = self.multisig.create_form.as_ref() else {
            return div();
        };
        let (m, n) = (form.threshold, form.owners.len());
        let n64 = u64::try_from(n).unwrap_or(u64::MAX);
        let note = match threshold_note(m, n) {
            ThresholdNote::Solo => words::t(&loc, "create.solo"),
            ThresholdNote::All => words::t(&loc, "create.thresholdAll"),
            ThresholdNote::One => words::t(&loc, "create.thresholdOne"),
            ThresholdNote::Spare(count) => words::tc(&loc, "create.thresholdSpare", count),
        };
        let risky = n > 1 && (m == n64 || m == 1);
        let stepper_button =
            |id: &'static str, label: String, glyph: &'static str, enabled: bool| {
                div()
                    .id(id)
                    .a11y_button(SharedString::from(label))
                    .size(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(theme.border_strong)
                    .text_size(theme::text_row_title())
                    .text_color(if enabled {
                        theme.fg_base
                    } else {
                        theme.fg_subtle
                    })
                    .when(enabled, |el| {
                        el.cursor_pointer().hover(|el| el.bg(theme.bg_sunken))
                    })
                    .when(!enabled, |el| el.opacity(0.4))
                    .child(glyph)
            };
        let fewer = stepper_button(
            "ms-create-fewer",
            words::t(&loc, "create.fewer"),
            "−",
            m > 1,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.ms_create_edit(cx, |form| {
                form.threshold = step_threshold(form.threshold, form.owners.len(), false)
            });
        }));
        let more = stepper_button(
            "ms-create-more",
            words::t(&loc, "create.more"),
            "+",
            m < n64,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.ms_create_edit(cx, |form| {
                form.threshold = step_threshold(form.threshold, form.owners.len(), true)
            });
        }));
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(ms_label(theme, &words::t(&loc, "create.threshold")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(fewer)
                    .child(
                        div()
                            .min_w(px(80.))
                            .flex()
                            .justify_center()
                            .text_size(theme::text_panel_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(words::tv(
                                &loc,
                                "create.thresholdValue",
                                &[("m", &m.to_string()), ("n", &n.to_string())],
                            )),
                    )
                    .child(more),
            )
            .child(ms_note(
                theme,
                &note,
                if risky {
                    MsTone::Caution
                } else {
                    MsTone::Neutral
                },
            ));
        // Passkey-native recovery: a lost passkey's member is replaced by the others.
        if m < n64 {
            body = body.child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(crate::ui::prose(words::tv(
                        &loc,
                        "members.recovery",
                        &[("m", &m.to_string()), ("n", &n.to_string())],
                    ))),
            );
        }
        body.child(
            crate::ui::dialog::dialog_actions()
                .child(
                    crate::ui::vela_button(
                        "ms-create-back",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(&loc, "create.back").into(),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            if let Some(form) = this.multisig.create_form.as_mut() {
                                form.step = CreateStep::Members;
                            }
                            cx.notify();
                        }),
                    )
                    .w_auto(),
                )
                .child(
                    crate::ui::vela_button(
                        "ms-create-to-review",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(&loc, "create.next").into(),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            if let Some(form) = this.multisig.create_form.as_mut() {
                                form.step = CreateStep::Review;
                            }
                            this.multisig.scroll = crate::ui::SmoothScroll::new();
                            cx.notify();
                        }),
                    )
                    .w_auto(),
                ),
        )
    }

    // -- step 3: the review, and the one passkey ------------------------------------

    fn ms_create_review(
        &mut self,
        theme: &Theme,
        view: &ms_accounts::MultisigAccountsView,
        locals: &[(String, String)],
        offered: &[u64],
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let Some(form) = self.multisig.create_form.as_ref() else {
            return div();
        };
        let owners = form.owners.clone();
        let clean = form.name.trim().to_owned();
        let threshold = form.threshold;
        let chain_id = form.chain_id;
        let all = form.all_networks;
        let pressed = form.pressed;
        let name_ok = name_problem(&form.name).is_none();
        let local_owners: Vec<(String, String)> = owners
            .iter()
            .filter_map(|o| locals.iter().find(|(a, _)| same(a, o)).cloned())
            .collect();
        let creator = form
            .creator
            .clone()
            .filter(|c| local_owners.iter().any(|(a, _)| same(a, c)))
            .or_else(|| local_owners.first().map(|(a, _)| a.clone()));
        let draft = view
            .create
            .draft_address
            .clone()
            .filter(|_| name_ok && !owners.is_empty());
        let working = pressed && view.create.phase == Phase::Working;
        let n = owners.len();

        let mut body = div().flex().flex_col().gap(px(12.));
        // The address-to-be, before anything is signed.
        if let Some(address) = &draft {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .py(px(8.))
                    .child(crate::wallet::components::identicon_avatar(
                        &mut self.identicons,
                        address,
                        56.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_panel_title())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg_base)
                            .child(clean.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(words::rule_title(&loc, threshold, n as u64)),
                    )
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_base)
                            .child(crate::ui::address_text(vela_core::multisig_link::eip55(
                                address,
                            ))),
                    )
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(crate::ui::prose(words::t(&loc, "create.sameEverywhere"))),
                    ),
            );
        } else if let Some(error) = &view.create.draft_error {
            body = body.child(ms_note(theme, error, MsTone::Danger));
        }

        // The members: one from outside this device reads in full, here,
        // where it is checked before the passkey signs (r4a-14).
        body = body.child(ms_label(theme, &words::t(&loc, "create.members")));
        for (i, owner) in owners.iter().enumerate() {
            let label = self.ms_create_label(owner, locals, view);
            let mine = locals.iter().any(|(a, _)| same(a, owner));
            let sub = if mine {
                crate::wallet::live::shorten_address(owner)
            } else {
                vela_core::multisig_link::eip55(owner)
            };
            body = body.child(
                div()
                    .id(ElementId::from(("ms-create-review-member", i)))
                    .flex()
                    .gap(px(10.))
                    .child(
                        div()
                            .w(px(16.))
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child((i + 1).to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .text_color(theme.fg_base)
                                    .child(label.clone()),
                            )
                            .when(!mine || label != sub, |el| {
                                el.child(
                                    div()
                                        .font_family(theme::font_mono())
                                        .text_size(theme::text_label())
                                        .text_color(theme.fg_subtle)
                                        .child(crate::ui::address_text(sub.clone())),
                                )
                            }),
                    ),
            );
        }

        // The first network: the likely ones first, the rest one press away.
        body = body
            .child(ms_label(theme, &words::t(&loc, "create.network")))
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(crate::ui::prose(words::t(&loc, "create.networkHint"))),
            )
            .child(self.ms_network_choice(
                theme,
                "ms-create-net",
                offered,
                chain_id,
                all,
                !working,
                cx,
                |this, id, cx| {
                    this.ms_create_edit(cx, |form| {
                        form.chose = true;
                        form.chain_id = Some(id);
                    });
                },
                |this, cx| {
                    if let Some(form) = this.multisig.create_form.as_mut() {
                        form.all_networks = true;
                    }
                    cx.notify();
                },
            ));
        if chain_id.is_none() && !offered.is_empty() {
            body = body.child(ms_note(
                theme,
                &words::t(&loc, "create.networkNeeded"),
                MsTone::Neutral,
            ));
        }
        if offered.is_empty() {
            body = body.child(ms_note(
                theme,
                &words::t(&loc, "create.noSafeNetworks"),
                MsTone::Danger,
            ));
        }
        // r7-9: the other space's networks are not offered here — said, with the way there.
        let other: Vec<String> = view
            .create
            .other_space
            .iter()
            .map(|id| network(*id))
            .collect();
        if !other.is_empty() {
            let testnet = crate::network_space::testnet();
            let space = self
                .loc
                .t(if testnet {
                    "networkSpace.mainnet"
                } else {
                    "networkSpace.testnet"
                })
                .to_string();
            let switch = self.loc.t(if testnet {
                "networkSpace.toMainnet"
            } else {
                "networkSpace.toTestnet"
            });
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(4.))
                    .child(
                        div()
                            .w_full()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(crate::ui::prose(words::tv(
                                &loc,
                                "create.otherSpace",
                                &[("space", &space), ("networks", &loc.list(&other))],
                            ))),
                    )
                    .child(
                        text_action("ms-create-space", theme, &mut self.icons, None, switch)
                            // The draft is kept: the sheet steps aside for the switch.
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_multisig_create(cx);
                                this.space_prompt = true;
                                cx.notify();
                            })),
                    ),
            );
        }
        // Several of this device's accounts are members: which one's passkey proves it.
        if local_owners.len() > 1 {
            body = body.child(ms_label(theme, &words::t(&loc, "create.creator")));
            let mut row = div().flex().flex_wrap().gap(px(6.));
            for (i, (address, name)) in local_owners.iter().enumerate() {
                let on = creator.as_deref().is_some_and(|c| same(c, address));
                let pick = address.clone();
                row = row.child(
                    div()
                        .id(ElementId::from(("ms-create-creator", i)))
                        .a11y_toggle(SharedString::from(name.clone()), on)
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(if on { theme.fg_base } else { theme.divider })
                        .bg(if on { theme.bg_sunken } else { theme.bg_base })
                        .text_size(theme::text_label())
                        .text_color(theme.fg_base)
                        .cursor_pointer()
                        .hover(|el| el.bg(theme.bg_sunken))
                        .child(name.clone())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(form) = this.multisig.create_form.as_mut() {
                                form.creator = Some(pick.clone());
                            }
                            cx.notify();
                        })),
                );
            }
            body = body.child(row);
        }
        // What the last press came to.
        if pressed && view.create.address.is_none() {
            // Nothing was made: nothing to carry to another page.
            if matches!(view.create.phase, Phase::Failed | Phase::Cancelled) {
                super::multisig_links::Carry::take_create(cx);
            }
            match view.create.phase {
                Phase::Failed => {
                    let detail = view.create.error.clone().unwrap_or_default();
                    let text = self.ms_failure_words(&detail, chain_id.unwrap_or_default(), cx);
                    body = body.child(ms_note(theme, &text, MsTone::Danger));
                }
                Phase::Cancelled => {
                    body = body.child(ms_note(
                        theme,
                        &words::t(&loc, "proposal.reason.passkeyClosed"),
                        MsTone::Caution,
                    ));
                }
                _ => {}
            }
        }
        let ready =
            creator.is_some() && draft.is_some() && !offered.is_empty() && chain_id.is_some();
        let state = if working {
            crate::ui::ButtonState::Busy
        } else {
            crate::ui::ButtonState::from_enabled(ready)
        };
        let members_info: BTreeMap<String, MemberInfo> = owners
            .iter()
            .map(|o| {
                let said = self
                    .multisig
                    .create_form
                    .as_ref()
                    .and_then(|f| f.said.get(&o.to_ascii_lowercase()).cloned());
                (
                    o.clone(),
                    MemberInfo {
                        name: said,
                        ..MemberInfo::default()
                    },
                )
            })
            .collect();
        let choice = creator
            .clone()
            .zip(chain_id)
            .map(|(creator, chain_id)| CreateChoice {
                owners: owners.clone(),
                threshold,
                name: clean.clone(),
                chain_id,
                creator,
                members: members_info,
            });
        body.child(
            crate::ui::dialog::dialog_actions()
                .child(
                    crate::ui::vela_button_state(
                        "ms-create-review-back",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(&loc, "create.back").into(),
                        crate::ui::ButtonState::from_enabled(!working),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            if let Some(form) = this.multisig.create_form.as_mut() {
                                form.step = CreateStep::Threshold;
                            }
                            cx.notify();
                        }),
                    )
                    .w_auto(),
                )
                .child(
                    // One passkey: the creator's proof. The web's `primary`
                    // (CreateMultisigSheet): creating moves no value, so the
                    // ink, never the accent (vis-6).
                    crate::ui::vela_button_state(
                        "ms-create-create",
                        crate::ui::ButtonVariant::Primary,
                        words::t(&loc, "create.create").into(),
                        state,
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            let Some(choice) = choice.clone() else {
                                return;
                            };
                            if let Some(form) = this.multisig.create_form.as_mut() {
                                form.pressed = true;
                            }
                            // Created, it is the multisig shown: the page is
                            // built again for it, and the sheet carried over.
                            super::multisig_links::Carry::set_create(choice.name.clone(), cx);
                            let host = this.ms_host(cx);
                            host.update(cx, |host, cx| {
                                // The creator's passkey is this device's: the
                                // machine is told who is here first.
                                host.refresh_facts(cx);
                                host.accounts(ms_accounts::Event::Create { choice }, cx);
                            });
                            cx.notify();
                        }),
                    )
                    .w_auto(),
                ),
        )
    }

    // -- created -----------------------------------------------------------------

    fn ms_create_created(
        &mut self,
        theme: &Theme,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let Some(address) = view.create.address.clone() else {
            return div();
        };
        let name = self
            .multisig
            .create_form
            .as_ref()
            .map(|f| f.name.trim().to_owned())
            .unwrap_or_default();
        let row = view
            .accounts
            .iter()
            .find(|r| same(&r.account.address, &address));
        let solo = row.is_some_and(|r| r.account.owners.len() == 1);
        // r4a-12, r4a-13: kept here first; shared when the service has it — said as it is.
        let (state, pending) = match row {
            Some(r) if r.shared => (
                words::t(
                    &loc,
                    if solo {
                        "create.sharedSolo"
                    } else {
                        "create.shared"
                    },
                ),
                false,
            ),
            _ => match (&view.create.refused, &view.create.phase) {
                (Some(reason), _) => (
                    words::tv(&loc, "create.refused", &[("reason", reason)]),
                    true,
                ),
                (None, Phase::Working) => (words::t(&loc, "create.savedSharing"), true),
                (None, Phase::Failed) => {
                    let detail = view.create.error.clone().unwrap_or_default();
                    (
                        words::tv(&loc, "create.refused", &[("reason", &detail)]),
                        true,
                    )
                }
                _ => (words::t(&loc, "create.savedLater"), true),
            },
        };
        let invite = address.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .py(px(8.))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::Check,
                        false,
                        theme.fg_base,
                        28.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(words::t(&loc, "create.created")),
                    )
                    .when(!name.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(theme::text_panel_title())
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.fg_base)
                                .child(name.clone()),
                        )
                    })
                    .child(crate::wallet::components::identicon_avatar(
                        &mut self.identicons,
                        &address,
                        56.,
                    ))
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_base)
                            .child(crate::ui::address_text(vela_core::multisig_link::eip55(
                                &address,
                            ))),
                    ),
            )
            .child(ms_note(
                theme,
                &state,
                if pending {
                    MsTone::Caution
                } else {
                    MsTone::Neutral
                },
            ))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(crate::ui::prose(words::t(
                        &loc,
                        if solo {
                            "create.createdBodySolo"
                        } else {
                            "create.createdBody"
                        },
                    ))),
            )
            .child(
                crate::ui::dialog::dialog_actions()
                    .child(
                        crate::ui::vela_button(
                            "ms-create-invite",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "panel.invite").into(),
                            theme,
                            cx.listener(move |this, _, _, cx| {
                                this.close_multisig_create(cx);
                                this.multisig.invite = Some(invite.clone());
                                this.multisig.scroll = crate::ui::SmoothScroll::new();
                                cx.notify();
                            }),
                        )
                        .w_auto(),
                    )
                    .child(
                        crate::ui::vela_button(
                            "ms-create-done",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "create.done").into(),
                            theme,
                            cx.listener(|this, _, _, cx| this.close_multisig_create(cx)),
                        )
                        .w_auto(),
                    ),
            )
    }

    // -- shared pieces: a network to pick, a member's kind -----------------------------

    /// A network to pick, as a list on the sheet (the web's `NetworkChoice`):
    /// the likely ones first and in view, the rest one press away. Picking
    /// selects — it never acts.
    #[allow(clippy::too_many_arguments, reason = "one picker, named parts")]
    pub(crate) fn ms_network_choice(
        &mut self,
        theme: &Theme,
        id: &'static str,
        networks: &[u64],
        chosen: Option<u64>,
        all: bool,
        enabled: bool,
        cx: &mut Context<Self>,
        on_pick: impl Fn(&mut Self, u64, &mut Context<Self>) + Clone + 'static,
        on_more: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Div {
        let visible = visible_networks(networks, chosen, all, 4);
        let mut list = div().flex().flex_col().gap(px(4.));
        for (i, chain_id) in visible.iter().copied().enumerate() {
            let on = chosen == Some(chain_id);
            let name = network(chain_id);
            let pick = on_pick.clone();
            list = list.child(
                div()
                    .id(ElementId::from((id, i)))
                    .a11y_toggle(SharedString::from(name.clone()), on)
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if on { theme.fg_base } else { theme.divider })
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .when(enabled, |el| {
                        el.cursor_pointer().hover(|el| el.bg(theme.bg_sunken))
                    })
                    .child(name)
                    .when(on, |el| {
                        el.child(icon_img(
                            &mut self.icons,
                            Icon::Check,
                            false,
                            theme.fg_base,
                            14.,
                        ))
                    })
                    .when(enabled, |el| {
                        el.on_click(cx.listener(move |this, _, _, cx| pick(this, chain_id, cx)))
                    }),
            );
        }
        if visible.len() < networks.len() {
            let more = format!(
                "{} · {}",
                words::t(&self.loc, "add.moreNetworks"),
                networks.len() - visible.len()
            );
            list = list.child(
                text_action(
                    SharedString::from(format!("{id}-more")),
                    theme,
                    &mut self.icons,
                    None,
                    more.into(),
                )
                .on_click(cx.listener(move |this, _, _, cx| on_more(this, cx))),
            );
        }
        list
    }

    /// r7: a member's kind (the web's `MemberKindChoice`): shown as found
    /// when evidence says what it is, else "Vela account" / "Other wallet",
    /// the hint drawn as a hint. What it shows is the core's `KindChoice`;
    /// a pick goes back to the accounts machine (`pick_kind`).
    #[allow(clippy::too_many_arguments, reason = "one choice, named parts")]
    pub(crate) fn ms_kind_choice(
        &mut self,
        theme: &Theme,
        id: &'static str,
        index: usize,
        address: &str,
        name: &str,
        choice: KindChoice,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let note = |key: &str| {
            div()
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(crate::ui::prose(words::t(&loc, key)))
        };
        if choice.locked {
            let contract = choice.kind == Some(MemberKind::Contract);
            let mut known = div().flex().items_center().gap(px(6.));
            if choice.kind == Some(MemberKind::Vela) {
                known = known.child(icon_img(
                    &mut self.icons,
                    Icon::Check,
                    false,
                    theme.fg_base,
                    14.,
                ));
            }
            known = known.child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(words::t(
                        &loc,
                        if contract {
                            "members.kindContract"
                        } else {
                            "members.kindVela"
                        },
                    )),
            );
            let line = if choice.note == Some(KindNote::Contract) {
                "members.kindContractNote"
            } else {
                "members.kindKnown"
            };
            return div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(known)
                .child(note(line));
        }
        let mut row = div()
            .id(ElementId::from((id, 10_000 + index)))
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .a11y_item(SharedString::from(format!(
                "{} · {name}",
                words::t(&loc, "members.kind")
            )));
        for (j, (kind, key)) in [
            (MemberKind::Vela, "members.kindVela"),
            (MemberKind::Wallet, "members.kindWallet"),
        ]
        .into_iter()
        .enumerate()
        {
            let on = choice.kind == Some(kind);
            let hint = choice.suggested == Some(kind);
            let target = address.to_owned();
            row = row.child(
                div()
                    .id(ElementId::from((id, index * 2 + j)))
                    .a11y_toggle(SharedString::from(words::t(&loc, key)), on)
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(if on {
                        theme.fg_base
                    } else if hint {
                        theme.border_strong
                    } else {
                        theme.divider
                    })
                    .bg(if on { theme.bg_sunken } else { theme.bg_base })
                    .text_size(theme::text_label())
                    .text_color(if on || hint {
                        theme.fg_base
                    } else {
                        theme.fg_muted
                    })
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(words::t(&loc, key))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let host = this.ms_host(cx);
                        let address = target.clone();
                        host.update(cx, |host, cx| {
                            host.accounts(
                                ms_accounts::Event::PickKind {
                                    address,
                                    kind: Some(kind),
                                },
                                cx,
                            );
                        });
                        // A member of the draft: its kind is kept with it.
                        this.ms_create_sync(cx);
                    })),
            );
        }
        let mut col = div().flex().flex_col().gap(px(4.)).child(row);
        match choice.note {
            Some(KindNote::Likely) => col = col.child(note("members.kindLikely")),
            Some(KindNote::Ask) => col = col.child(note("members.kindAsk")),
            _ => {}
        }
        col
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_threshold_starts_sensible_and_steps_within_the_members() {
        assert_eq!(default_threshold(1), 1);
        assert_eq!(default_threshold(2), 2);
        assert_eq!(default_threshold(3), 2);
        assert_eq!(default_threshold(5), 4);
        assert_eq!(step_threshold(1, 3, false), 1, "never below one");
        assert_eq!(step_threshold(3, 3, true), 3, "never above the members");
        assert_eq!(step_threshold(2, 3, true), 3);
        assert_eq!(step_threshold(2, 3, false), 1);
        assert_eq!(
            step_threshold(5, 2, false),
            2,
            "a threshold past fewer members comes back within them"
        );
        assert_eq!(step_threshold(1, 0, true), 1);
    }

    #[test]
    fn the_spare_members_are_counted_in_the_words() {
        let loc = Loc::for_tag("en");
        let line = words::tc(&loc, "create.thresholdSpare", 2);
        assert!(line.contains('2') && !line.contains("{{"), "{line}");
    }

    #[test]
    fn what_the_threshold_means() {
        assert_eq!(threshold_note(1, 1), ThresholdNote::Solo);
        assert_eq!(threshold_note(3, 3), ThresholdNote::All);
        assert_eq!(threshold_note(1, 3), ThresholdNote::One);
        assert_eq!(threshold_note(2, 4), ThresholdNote::Spare(2));
    }

    #[test]
    fn the_name_is_said_of_the_name() {
        assert_eq!(name_problem("   "), Some(NameProblem::Needed));
        assert_eq!(name_problem("Team fund"), None);
        assert_eq!(name_problem(" Team fund "), None, "kept trimmed");
        assert_eq!(
            name_problem(&"é".repeat(40)),
            Some(NameProblem::TooLong(80))
        );
        assert_eq!(name_problem("dnuf\u{202E}maeT"), Some(NameProblem::Invalid));
    }

    #[test]
    fn the_networks_offered_keep_the_pick_in_view() {
        let all = [1, 100, 8453, 10, 42_161, 137];
        assert_eq!(
            visible_networks(&all, None, false, 4),
            vec![1, 100, 8453, 10]
        );
        assert_eq!(
            visible_networks(&all, Some(137), false, 4),
            vec![1, 100, 8453, 10, 137]
        );
        assert_eq!(
            visible_networks(&all, Some(100), false, 4),
            vec![1, 100, 8453, 10]
        );
        assert_eq!(visible_networks(&all, None, true, 4), all.to_vec());
        assert_eq!(
            visible_networks(&all[..5], None, false, 4),
            all[..5].to_vec(),
            "one more is shown, not hidden"
        );
    }

    #[test]
    fn the_likely_networks_come_from_the_core() {
        let hints = json!({ "held": [100], "here": [8453] });
        assert_eq!(&likely_networks(&[1, 100, 8453], &hints)[..2], &[8453, 100]);
        assert_eq!(first_network(&[1, 100, 8453], &hints), Some(8453));
        assert_eq!(
            first_network(&[1, 100], &json!({})),
            None,
            "nothing picked when nothing says"
        );
    }

    #[test]
    fn an_ens_answer_is_matched_to_the_name_asked() {
        let view = ms_accounts::AddView {
            ens: Some(EnsAnswer::None {
                name: "bob.eth".into(),
            }),
            ..ms_accounts::AddView::default()
        };
        assert!(ens_answer(&view, Some("Bob.eth")).is_some());
        assert!(ens_answer(&view, Some("alice.eth")).is_none());
        assert!(ens_answer(&view, None).is_none());
    }
}
