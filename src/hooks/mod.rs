mod constants;
mod mkinitcpio;
mod quicknet;
mod replace_token;
mod uncomment;
mod wrappers;

pub use self::constants::hook_keys::*;

use std::collections::HashSet;

use colored::Colorize;
use serde::{
    Deserialize,
    Serialize,
};

use crate::errors::AliError;

/// All hook actions stores JSON string representation of the hook.
/// The reason being we want to hide hook implementation from outside code.
#[derive(Debug, Clone, Serialize, Deserialize)]

/// A report of hook actions, preferably in JSON or other serialized strings.
pub enum ActionHook {
    QuickNet(String),
    ReplaceToken(String),
    Uncomment(String),
    Mkinitcpio(String),
}

/// Entrypoint for hooks.
/// Some hooks may prefer to be called by certain callers.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Caller {
    ManifestChroot,
    ManifestPostInstall,
    Cli,
}

/// ModeHook represents whether this hook command is print-only
#[derive(Clone)]
enum ModeHook {
    /// May write changes to disk
    Normal,
    /// Print-only, i.e. idempotent
    Print,
}

/// HookWrapper wraps an inner hook, and provides this module with per-hook information
/// which this module needs in order to validate hooks and helps with control flow.
///
/// By convention, a new HookWrapper is created via with a _key_ string -
/// this allows HookWrapper to determine [`ModeHook`](ModeHook), which
/// is later accessed via [`mode()`](Self::mode), as with [`quicknet::new`](crate::hooks::quicknet::new).
///
/// The newly created HookWrapper is then fed a command string
/// via [`try_parse`](Self::try_parse).
///
/// HookWrapper is responsible for parsing the hook command string
/// and returning the [Hook](Hook) implementation via [`Self::advance`](Self::advanced),
/// populating any information the Hook implementation might require.
trait HookWrapper {
    /// (Default) Prints help to output
    fn help(&self) {
        println!(
            "{}",
            format!("{}: {}", self.hook_key(), self.usage()).green(),
        );
    }
    /// (Default) Prints yellow warning text to output
    fn eprintln_warn(&self, msg: &str) {
        eprintln!(
            "### {} ###",
            format!("{} WARN: {msg}", self.base_key()).yellow()
        );
    }

    /// (Default) Wraps error in hook with some string prefix
    fn hook_error(&self, msg: &str) -> AliError {
        AliError::HookError(format!("{}: {msg}", self.hook_key()))
    }

    /// (Default) Full key of the hook
    fn hook_key(&self) -> String {
        match self.mode() {
            ModeHook::Normal => self.base_key().to_string(),
            ModeHook::Print => format!("{}-print", self.base_key()),
        }
    }

    /// Base hook key (no `-print` suffix)
    fn base_key(&self) -> &'static str;

    /// Returns usage string for Self.help
    fn usage(&self) -> &'static str;

    /// Returns ModeHook parsed
    fn mode(&self) -> ModeHook;

    /// Returns whether this hook should be run inside chroot
    /// (warning only)
    fn should_chroot(&self) -> bool;

    /// Returns a set of callers the hook expects to be called from
    fn preferred_callers(&self) -> HashSet<Caller>;

    /// Returns if we should abort if no mountpoint is found
    /// (i.e. root_location or mountpoint == /)
    fn abort_if_no_mount(&self) -> bool;

    /// Tries parsing `s` and returns error if any
    /// Implementation should use this chance to populate parsed data
    /// (hence `&mut self`) so that we only parse once.
    ///
    /// Nonetheless, implementation may parse s later with Self.try_parse,
    fn try_parse(&mut self, s: &str) -> Result<(), AliError>;

    /// Runs the inner hook
    fn run_hook(
        &self,
        caller: &Caller,
        root_location: &str,
    ) -> Result<ActionHook, AliError>;
}

/// Parses hook_cmd from manifest or CLI to hooks,
/// into some HookWrapper, and validates it before finally
/// executing the hook.
pub fn apply_hook(
    cmd: &str,
    caller: Caller,
    root_location: &str,
) -> Result<ActionHook, AliError> {
    let hook_meta = parse_validate(cmd, &caller, root_location)?;
    hook_meta.run_hook(&caller, root_location)
}

/// Validates if hook_cmd is valid for its caller and mountpoint
pub fn validate_hook(
    cmd: &str,
    caller: &Caller,
    root_location: &str,
) -> Result<(), AliError> {
    _ = parse_validate(cmd, caller, root_location)?;

    Ok(())
}

pub fn is_hook(cmd: &str) -> bool {
    cmd.starts_with('@')
}

pub fn extract_key_and_parts(
    cmd: &str,
) -> Result<(String, Vec<String>), AliError> {
    let parts = cmd.split_whitespace().collect::<Vec<_>>();
    if parts.first().is_none() {
        return Err(AliError::AliRsBug("@mnt: got 0 part".to_string()));
    }

    let key = parts.first().unwrap();

    Ok((
        key.to_string(),
        parts.into_iter().map(|s| s.to_string()).collect(),
    ))
}

fn parse_validate(
    cmd: &str,
    caller: &Caller,
    root_location: &str,
) -> Result<Box<dyn HookWrapper>, AliError> {
    let hook_parts = cmd.split_whitespace().collect::<Vec<_>>();

    if hook_parts.is_empty() {
        return Err(AliError::BadManifest("empty hook".to_string()));
    }

    let key = *hook_parts.first().unwrap();

    let mut h = init_blank_hook(key)?;

    if let Err(err) = h.try_parse(cmd) {
        h.help();
        return Err(err);
    }

    if h.should_chroot() {
        handle_no_mountpoint(h.as_ref(), caller, root_location)?;
    }

    Ok(h)
}

fn init_blank_hook(k: &str) -> Result<Box<dyn HookWrapper>, AliError> {
    match k {
        KEY_WRAPPER_MNT | KEY_WRAPPER_NO_MNT => {
            Ok(wrappers::new(k)) //
        }

        KEY_QUICKNET | KEY_QUICKNET_PRINT => {
            Ok(quicknet::new(k)) //
        }

        KEY_MKINITCPIO | KEY_MKINITCPIO_PRINT => {
            Ok(mkinitcpio::new(k)) //
        }

        KEY_REPLACE_TOKEN | KEY_REPLACE_TOKEN_PRINT => {
            Ok(replace_token::new(k))
        }

        KEY_UNCOMMENT
        | KEY_UNCOMMENT_PRINT
        | KEY_UNCOMMENT_ALL
        | KEY_UNCOMMENT_ALL_PRINT => Ok(uncomment::new(k)),

        key => Err(AliError::BadArgs(format!("unknown hook key: {key}"))),
    }
}

impl std::fmt::Display for Caller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestChroot => write!(f, "manifest key `chroot`"),
            Self::ManifestPostInstall => {
                write!(f, "manifest key `postinstall`")
            }
            Self::Cli => {
                write!(f, "subcommand `hooks`")
            }
        }
    }
}

fn all_callers() -> HashSet<Caller> {
    HashSet::from([
        Caller::ManifestChroot,
        Caller::ManifestPostInstall,
        Caller::Cli,
    ])
}

fn handle_no_mountpoint(
    hook: &dyn HookWrapper,
    caller: &Caller,
    mountpoint: &str,
) -> Result<(), AliError> {
    if mountpoint == "/" {
        hook.eprintln_warn("got / as mountpoint");

        match caller {
            Caller::Cli => {
                hook.eprintln_warn(
                    "hint: use --mountpoint flag to specify non-/ mountpoint",
                )
            }
            Caller::ManifestPostInstall | Caller::ManifestChroot => {
                return Err(AliError::AliRsBug(format!(
                    "Got / as mountpoint for hook {}",
                    hook.hook_key(),
                )))
            }
        }

        if hook.abort_if_no_mount() {
            return Err(AliError::BadHookCmd(format!(
                "hook {} is to be run with a mountpoint",
                hook.hook_key()
            )));
        }
    }

    let preferred_callers = hook.preferred_callers();
    if !preferred_callers.contains(caller) {
        hook.eprintln_warn("non-preferred caller {caller}");
        hook.eprintln_warn("preferred callers: {preferred_callers:?}");
    }

    Ok(())
}
