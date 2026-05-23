//! Shared helpers for shelling out to external commands.
//!
//! Bifrost invokes several CLI tools (Sandboxie's `SbieIni.exe` /
//! `Start.exe`, the Sandboxie installer's silent flags, the Inno-Setup
//! uninstaller, PowerShell for elevation). Three patterns recur across
//! all of them:
//!
//!   1. Suppress the console flash a GUI-subsystem process gets when it
//!      spawns a CLI tool (`CREATE_NO_WINDOW`).
//!   2. Format a uniform `"<label> exited with <status> — <stderr>"`
//!      error message when a tool fails.
//!   3. Elevate a silent installer via PowerShell's
//!      `Start-Process -Verb RunAs -Wait`, mapping the UAC-declined
//!      exit code to a friendly message.
//!
//! Centralising these here keeps the call sites focused on what they're
//! actually doing — and means future tweaks (new exit-code mappings,
//! logging hooks, retry policy) only have to land in one place.

use std::path::Path;
use std::process::ExitStatus;

use tokio::process::Command;

use crate::error::{BifrostError, Result};

/// Windows process-creation flag that hides the console window we'd
/// otherwise inherit when spawning a CLI tool from Bifrost's
/// GUI-subsystem process. See:
/// <https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows `ERROR_CANCELLED`. PowerShell propagates this exit code
/// when the user declines a `RunAs` UAC prompt; [`run_elevated_silent`]
/// detects it to surface "UAC declined" instead of the raw status.
pub const UAC_DECLINED_EXIT_CODE: i32 = 1223;

/// Apply [`CREATE_NO_WINDOW`] to a tokio Command. No-op on non-Windows
/// so call sites can stay platform-agnostic. `creation_flags` is an
/// inherent method on `tokio::process::Command` on Windows builds, so
/// no extra `use` is needed.
#[cfg(windows)]
pub fn no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn no_window(_cmd: &mut Command) {}

/// Build the standard "exited with status N — stderr" message used
/// whenever an external CLI call fails. Trims the stderr and omits the
/// "— …" suffix when nothing was written.
pub fn cli_failure_msg(label: &str, status: ExitStatus, stderr: &[u8]) -> String {
    let stderr_trimmed = String::from_utf8_lossy(stderr).trim().to_string();
    if stderr_trimmed.is_empty() {
        format!("{label} exited with {status}")
    } else {
        format!("{label} exited with {status} — {stderr_trimmed}")
    }
}

/// Run an installer-style executable elevated and silently.
///
/// Wraps `Start-Process -FilePath <exe> -ArgumentList <args>
/// -Verb RunAs -Wait -PassThru` in a PowerShell one-liner. The one
/// unavoidable side effect is the Windows UAC prompt; everything else
/// (installer GUI, log windows) is suppressed via the silent flags
/// each caller passes in `args` (e.g. `/verysilent
/// /suppressmsgboxes /norestart` for Inno Setup).
///
/// `label` is folded into error messages so a failure surfaces as
/// e.g. "Sandboxie installer exited with 2" rather than something
/// generic about `powershell.exe`. UAC declined ([`UAC_DECLINED_EXIT_CODE`])
/// is mapped to a friendlier "cancelled — UAC prompt declined." error.
///
/// **Argument contract**: neither `exe` nor any `arg` may contain
/// newlines (`\r`/`\n`) or NUL. The function single-quote-escapes
/// quotes correctly, but newlines would terminate the PowerShell
/// one-liner mid-command and a NUL would break the IPC encoding.
/// Today every caller passes either a known-safe Windows file path
/// or a static silent-install flag, but rejecting the bad shape
/// up-front is cheaper than relying on every future caller to
/// remember the invariant.
pub async fn run_elevated_silent(exe: &Path, args: &[&str], label: &str) -> Result<()> {
    let exe_str = exe.to_string_lossy();
    if has_forbidden_char(&exe_str) {
        return Err(BifrostError::Other(format!(
            "{label}: executable path contains a newline or NUL ({exe_str:?})"
        )));
    }
    for arg in args {
        if has_forbidden_char(arg) {
            return Err(BifrostError::Other(format!(
                "{label}: argument contains a newline or NUL ({arg:?})"
            )));
        }
    }

    // PowerShell single-quote escape: a literal single quote inside a
    // single-quoted string is two single quotes.
    let exe_q = exe_str.replace('\'', "''");
    let args_q: Vec<String> = args
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "''")))
        .collect();
    let args_list = args_q.join(",");

    let ps_cmd = format!(
        "$ErrorActionPreference='Stop'; \
         $p = Start-Process -FilePath '{exe_q}' \
                            -ArgumentList {args_list} \
                            -Verb RunAs -Wait -PassThru; \
         if ($p.ExitCode -ne 0) {{ exit $p.ExitCode }}"
    );

    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd]);
    no_window(&mut cmd);

    let out = cmd
        .output()
        .await
        .map_err(|e| BifrostError::Other(format!("failed to spawn {label}: {e}")))?;

    if !out.status.success() {
        if matches!(out.status.code(), Some(UAC_DECLINED_EXIT_CODE)) {
            return Err(BifrostError::Other(format!(
                "{label} cancelled — UAC prompt declined."
            )));
        }
        return Err(BifrostError::Other(cli_failure_msg(
            label,
            out.status,
            &out.stderr,
        )));
    }
    Ok(())
}

/// True when `s` contains any character that would break the
/// PowerShell one-liner constructed by [`run_elevated_silent`].
/// Newlines terminate the command mid-statement; NUL breaks the
/// process-creation IPC encoding entirely.
fn has_forbidden_char(s: &str) -> bool {
    s.contains(['\n', '\r', '\0'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_int;

    /// Build an ExitStatus from a raw code so we can exercise the
    /// `cli_failure_msg` formatter without spawning a real process.
    /// Cross-platform via `from_raw` on Unix and bit-pattern on
    /// Windows (the actual value isn't relied on — we only read it
    /// back through `Display`).
    #[cfg(unix)]
    fn make_status(code: c_int) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    fn make_status(code: c_int) -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code as u32)
    }

    #[test]
    fn failure_msg_includes_stderr_when_present() {
        let msg = cli_failure_msg("frobnicator", make_status(2), b"  bad input  \n");
        assert!(msg.contains("frobnicator"));
        assert!(msg.contains("bad input"));
        // stderr trim removed surrounding whitespace
        assert!(!msg.contains("  bad input"));
    }

    #[test]
    fn failure_msg_omits_stderr_separator_when_empty() {
        let msg = cli_failure_msg("frobnicator", make_status(1), b"");
        assert!(msg.contains("frobnicator"));
        assert!(
            !msg.contains(" — "),
            "should not emit a '— ' separator when stderr is empty"
        );
    }

    #[test]
    fn failure_msg_omits_stderr_separator_when_whitespace_only() {
        let msg = cli_failure_msg("frobnicator", make_status(1), b"   \n\r\n  ");
        assert!(
            !msg.contains(" — "),
            "whitespace-only stderr must be treated as empty"
        );
    }

    #[test]
    fn uac_declined_exit_code_is_pinned_at_1223() {
        // Compile-time pin. The Inno-Setup uninstaller, Sandboxie
        // installer, and any future elevated launch all rely on this
        // constant matching Windows' ERROR_CANCELLED.
        assert_eq!(UAC_DECLINED_EXIT_CODE, 1223);
    }

    /// `run_elevated_silent` argument validation. The function
    /// constructs a PowerShell one-liner; a stray newline mid-argument
    /// would terminate the command early, a NUL would break process
    /// creation. Both must be rejected upstream of the spawn.
    #[test]
    fn has_forbidden_char_rejects_control_chars() {
        assert!(has_forbidden_char("hello\nworld"));
        assert!(has_forbidden_char("hello\rworld"));
        assert!(has_forbidden_char("hello\0world"));
        assert!(has_forbidden_char("\n"));
    }

    #[test]
    fn has_forbidden_char_accepts_normal_paths_and_args() {
        // The typical inputs run_elevated_silent receives: Windows
        // file paths (spaces, backslashes, single quotes via escape)
        // and Inno Setup silent flags.
        assert!(!has_forbidden_char(
            r"C:\Program Files\Sandboxie\unins000.exe"
        ));
        assert!(!has_forbidden_char("/verysilent"));
        assert!(!has_forbidden_char("/suppressmsgboxes"));
        assert!(!has_forbidden_char(r"C:\Users\valknarr's path\setup.exe"));
    }
}
