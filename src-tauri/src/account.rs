//! The personal API key's account (v0.3): who the stored key belongs to, and
//! what the key's extras last heard from Nexus (tracked mods, endorsements).
//!
//! **The standing** (`NexusSummary.key`): `none` without a key; `unchecked`
//! while a stored key waits for its validation (a thread of its own validates
//! it at the wake, without waiting for the first scan) or when Nexus was out
//! of reach; `connected` once
//! `/v1/users/validate.json` answered with the account (it spends no quota);
//! `rejected` when Nexus refused the key at its last validation. A rejected
//! key stays stored: the player replaces it, the stone never forgets it on
//! its own.
//!
//! **The epoch.** Every key change bumps it. A validation, or a standings
//! answer, that started under an older key is never applied: the key the
//! player set a moment later decides.
//!
//! The account's name is the player's own: it reaches the window, never a
//! log line. Neither the key nor the e-mail is ever read into it.

use std::sync::atomic::Ordering;

use rift_core::RiftError;
use rift_nexus::{EndorseStatus, NexusAccount, NexusError, NexusMod};

use crate::dto::{NexusAccountView, NexusKeyStanding};
use crate::events;
use crate::persist;
use crate::state::{lock, read, AppState};

/// What the spine believes about the stored key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountWord {
    pub standing: NexusKeyStanding,
    /// Set exactly when `standing` is `connected`.
    pub known: Option<KnownAccount>,
    /// Bumped by every key change.
    pub epoch: u64,
}

/// The account a validation answered with, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownAccount {
    pub account: NexusAccount,
    pub checked_ms: u64,
}

/// What the window can tell apart about the standing: a change of it is
/// worth a `rift://updates-changed`, a fresh `checkedMs` alone is not.
type Seen = (NexusKeyStanding, Option<(String, bool, bool)>);

impl AccountWord {
    /// At launch: a stored key waits for its validation.
    pub fn at_launch(has_key: bool) -> AccountWord {
        AccountWord {
            standing: if has_key {
                NexusKeyStanding::Unchecked
            } else {
                NexusKeyStanding::None
            },
            known: None,
            epoch: 0,
        }
    }

    /// The validated account, when the key is connected.
    pub fn connected(&self) -> Option<&NexusAccount> {
        match self.standing {
            NexusKeyStanding::Connected => self.known.as_ref().map(|known| &known.account),
            _ => None,
        }
    }

    /// The account as the window shows it: its name and whether it is
    /// premium or a supporter; never the key, never the e-mail.
    pub fn view(&self) -> Option<NexusAccountView> {
        let known = self.known.as_ref()?;
        (self.standing == NexusKeyStanding::Connected).then(|| NexusAccountView {
            name: known.account.name.clone(),
            premium: known.account.is_premium,
            supporter: known.account.is_supporter,
            checked_ms: known.checked_ms,
        })
    }

    fn seen(&self) -> Seen {
        (
            self.standing,
            self.connected().map(|account| {
                (
                    account.name.clone(),
                    account.is_premium,
                    account.is_supporter,
                )
            }),
        )
    }

    fn connect(&mut self, account: NexusAccount, now_ms: u64) {
        self.standing = NexusKeyStanding::Connected;
        self.known = Some(KnownAccount {
            account,
            checked_ms: now_ms,
        });
    }

    fn settle(&mut self, standing: NexusKeyStanding) {
        self.standing = standing;
        self.known = None;
    }
}

/// What the key's extras last heard (`get_nexus_standings`), for the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandingsCache {
    /// The key epoch it was heard under.
    pub epoch: u64,
    /// The tracked mods in Nexus's order, each with what the v2 lookup said
    /// about it (`None`: the lookup did not return it).
    pub tracked: Vec<(u64, Option<NexusMod>)>,
    /// The decided endorsements, in Nexus's order.
    pub endorsements: Vec<(u64, EndorseStatus)>,
    /// When the spine last asked Nexus.
    pub checked_ms: u64,
}

/// Validate the stored key now (`/v1/users/validate.json`, no quota spent):
/// a key Nexus accepts is `connected` with its account, a refused one
/// `rejected` (that is no error), and a validation that could not finish
/// (offline, an odd answer) leaves the key `unchecked` and says why. Without
/// a key, nothing is asked. The window hears `rift://updates-changed` when
/// the standing changed.
pub fn validate(state: &AppState) -> Result<(), RiftError> {
    // The epoch first, then the client: a key set in between is either the
    // one validated here or one this answer is not applied over.
    let epoch = lock(&state.account).epoch;
    let client = read(&state.nexus_client).clone();
    if !client.has_api_key() || !state.has_api_key.load(Ordering::SeqCst) {
        return Ok(());
    }
    let answer = client.validate_key();
    let now = persist::now_ms();
    let (changed, result) = {
        let mut word = lock(&state.account);
        if word.epoch != epoch {
            // The key changed while Nexus answered: its own word stands.
            return Ok(());
        }
        let before = word.seen();
        let result = match answer {
            Ok(account) => {
                word.connect(account, now);
                Ok(())
            }
            Err(NexusError::KeyRejected) => {
                word.settle(NexusKeyStanding::Rejected);
                Ok(())
            }
            Err(err) => {
                word.settle(NexusKeyStanding::Unchecked);
                Err(RiftError::from(err))
            }
        };
        (word.seen() != before, result)
    };
    if changed {
        state.events.changed(events::UPDATES_CHANGED);
    }
    result
}

/// The launch validation (startup step 7): a stored key is validated at the
/// wake, on the account thread, without waiting for the first scan; what
/// goes wrong is logged (the overview says `unchecked`, and
/// `refresh_nexus_account` tries again).
pub fn validate_at_wake(state: &AppState) {
    if !state.has_api_key.load(Ordering::SeqCst) {
        return;
    }
    if let Err(err) = validate(state) {
        log::warn!("the stored Nexus Mods key could not be validated now: {err}");
    }
}

/// A new key was validated and stored (`set_nexus_key`), or the key was
/// cleared (`None`): the word starts over under a new epoch, and the
/// standings of the old key are forgotten. `true` when the window should
/// hear it (`rift://updates-changed`).
pub fn key_changed(state: &AppState, account: Option<NexusAccount>) -> bool {
    let changed = {
        let mut word = lock(&state.account);
        let before = word.seen();
        word.epoch += 1;
        match account {
            Some(account) => word.connect(account, persist::now_ms()),
            None => word.settle(NexusKeyStanding::None),
        }
        word.seen() != before
    };
    *lock(&state.standings) = None;
    changed
}

/// The validated account, validating the stored key first when it has not
/// been this session (a download needs to know whether it is premium). A
/// rejected key and a missing one are refusals in Nexus's own words.
pub fn account_now(state: &AppState) -> Result<NexusAccount, RiftError> {
    if !state.has_api_key.load(Ordering::SeqCst) {
        return Err(NexusError::NoApiKey.into());
    }
    if let Some(account) = lock(&state.account).connected().cloned() {
        return Ok(account);
    }
    validate(state)?;
    let word = lock(&state.account);
    match (word.standing, word.connected()) {
        (_, Some(account)) => Ok(account.clone()),
        (NexusKeyStanding::Rejected, None) => Err(NexusError::KeyRejected.into()),
        _ => Err(NexusError::NoApiKey.into()),
    }
}

/// Did Nexus refuse the stored key at its last validation?
pub fn key_rejected(state: &AppState) -> bool {
    lock(&state.account).standing == NexusKeyStanding::Rejected
}

/// The validated account's numeric id, when the stone knows it (an nxm link
/// for another account is refused against it).
pub fn account_user_id(state: &AppState) -> Option<u64> {
    lock(&state.account)
        .connected()
        .and_then(|account| account.user_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(premium: bool) -> NexusAccount {
        NexusAccount {
            name: "Arisen of Vernworth".into(),
            user_id: Some(7_700_123),
            is_premium: premium,
            is_supporter: false,
        }
    }

    #[test]
    fn a_stored_key_waits_unchecked_until_nexus_answers() {
        assert_eq!(
            AccountWord::at_launch(false).standing,
            NexusKeyStanding::None
        );
        let mut word = AccountWord::at_launch(true);
        assert_eq!(word.standing, NexusKeyStanding::Unchecked);
        assert_eq!(word.view(), None, "no account before the validation");
        assert_eq!(word.connected(), None);

        word.connect(account(true), 42);
        let view = word.view().expect("connected: the account shows");
        assert_eq!(
            (
                view.name.as_str(),
                view.premium,
                view.supporter,
                view.checked_ms
            ),
            ("Arisen of Vernworth", true, false, 42)
        );
        assert_eq!(word.connected().and_then(|a| a.user_id), Some(7_700_123));

        word.settle(NexusKeyStanding::Rejected);
        assert_eq!(word.view(), None, "a rejected key shows no account");
        assert_eq!(word.known, None);
    }

    #[test]
    fn only_what_the_window_can_tell_apart_counts_as_a_change() {
        let mut word = AccountWord::at_launch(true);
        word.connect(account(false), 1);
        let before = word.seen();
        word.connect(account(false), 2);
        assert_eq!(word.seen(), before, "a fresh checkedMs alone is no change");
        word.connect(account(true), 3);
        assert_ne!(word.seen(), before, "premium now: a change");
    }
}
