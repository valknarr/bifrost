//! Wrapper around Sandboxie-Plus's CLI tools.
//!
//! We deliberately shell out to `SbieIni.exe` and `Start.exe` rather than
//! linking `SbieDll.dll`:
//!   1. GPL boundary stays clean (`SbieDll` is GPL-3; the CLIs are sibling
//!      tools we just invoke).
//!   2. The CLI surface is more stable than the DLL's ABI.
//!   3. Users can swap their Sandboxie-Plus version without us rebuilding.
//!
//! The provisioning verbs (templates, ConfigLevel, BorderColor, etc.)
//! mirror the EVE Frontier community's `CreateEFSandboxes.bat` /
//! `LaunchEFSandboxes.bat` reference workflow.

use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::cmd::{cli_failure_msg, no_window};
use crate::error::{BridgeError, Result};

/// One Sandboxie-Plus install. Holds resolved paths to the CLI tools.
/// `root` is retained for diagnostics / future helpers even though no
/// caller reads it today.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Sandboxie {
    pub root: PathBuf,
    pub sbie_ini: PathBuf,
    pub start_exe: PathBuf,
}

impl Sandboxie {
    /// Resolve from an install root (e.g. `C:\Program Files\Sandboxie-Plus`).
    /// Errors if the expected executables aren't present.
    pub fn at(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let sbie_ini = root.join("SbieIni.exe");
        let start_exe = root.join("Start.exe");
        if !sbie_ini.exists() || !start_exe.exists() {
            return Err(BridgeError::SandboxieMissing);
        }
        Ok(Self {
            root,
            sbie_ini,
            start_exe,
        })
    }

    /// Run `SbieIni.exe <verb> <box> <key> <value>`.
    async fn ini(&self, verb: &str, box_name: &str, key: &str, value: &str) -> Result<()> {
        let mut cmd = Command::new(&self.sbie_ini);
        cmd.args([verb, box_name, key, value]);
        no_window(&mut cmd);
        let out = cmd.output().await?;
        if !out.status.success() {
            return Err(BridgeError::SandboxieCommand(cli_failure_msg(
                &format!("SbieIni {verb} {box_name} {key} {value}"),
                out.status,
                &out.stderr,
            )));
        }
        Ok(())
    }

    pub async fn set(&self, box_name: &str, key: &str, value: &str) -> Result<()> {
        self.ini("set", box_name, key, value).await
    }

    pub async fn append(&self, box_name: &str, key: &str, value: &str) -> Result<()> {
        self.ini("append", box_name, key, value).await
    }

    /// Provision a `FrontierN`-style box with the templates the EVE Frontier
    /// community converged on in the original .bat workflow.
    pub async fn provision_frontier_box(&self, box_name: &str) -> Result<()> {
        // Required core settings.
        self.set(box_name, "Enabled", "y").await?;
        self.set(box_name, "ConfigLevel", "10").await?;
        self.set(box_name, "AutoRecover", "n").await?;
        self.set(box_name, "BlockNetworkFiles", "y").await?;

        // Visual hints — Bridge draws its own UI so we keep Sandboxie's
        // window decorations off. Sandboxie's standard-isolation tree
        // icon stays yellow regardless of BorderColor (that's a
        // product decision in Sandboxie-Plus, not something we can
        // override), and a per-window BorderColor outline on the game
        // didn't add enough value to justify keeping in sync.
        self.set(box_name, "BorderColor", "off").await?;
        self.set(box_name, "BoxNameTitle", "n").await?;

        // Recover folder hints (mirrors the legacy .bat).
        self.set(box_name, "RecoverFolder", "%Desktop%").await?;
        self.append(box_name, "RecoverFolder", "%Personal%").await?;
        self.append(
            box_name,
            "RecoverFolder",
            "%{374DE290-123F-4565-9164-39C4925E467B}%",
        )
        .await?;

        // Templates the legacy .bat stacks on every box.
        for tmpl in [
            "AutoRecoverIgnore",
            "LingerPrograms",
            "BlockPorts",
            "qWave",
            "FileCopy",
            "SkipHook",
            "OpenBluetooth",
        ] {
            self.append(box_name, "Template", tmpl).await?;
        }

        self.set(box_name, "UseFileDeleteV2", "y").await?;
        self.set(box_name, "UseRegDeleteV2", "y").await?;

        // NOTE: the legacy .bat workflow also runs `Start.exe /box:N reg add
        // HKLM\...\ComputerName` to give each box a distinct ComputerName.
        // We deliberately don't do that here:
        //   1. Bridge has no console (GUI subsystem), so launching reg.exe
        //      inside the sandbox via Start.exe /wait fails with
        //      0x40010004 (process-terminated-abnormally) due to absent
        //      stdio handles.
        //   2. Sandboxie already isolates HKCU + the filesystem; the HKLM
        //      ComputerName override only matters if the game fingerprints
        //      on the machine name, which has not been observed for Frontier.
        // If we discover Frontier needs distinct ComputerNames, we'll add
        // the writes back using a console-attached spawn path.
        Ok(())
    }

    /// `Start.exe /box:<box> /wait <program> <args...>`
    /// Currently unused — kept as a building block for future actions
    /// that need to run a command inside a sandbox and wait for it.
    #[allow(dead_code)]
    pub async fn run_in_box(&self, box_name: &str, program: &str, args: &[&str]) -> Result<()> {
        let mut cmd = Command::new(&self.start_exe);
        cmd.arg(format!("/box:{box_name}"))
            .arg("/wait")
            .arg(program)
            .args(args);
        no_window(&mut cmd);
        let out = cmd.output().await?;
        if !out.status.success() {
            return Err(BridgeError::SandboxieCommand(cli_failure_msg(
                &format!("Start.exe /box:{box_name} {program}"),
                out.status,
                &out.stderr,
            )));
        }
        Ok(())
    }

    /// `Start.exe /box:<box> <program> <args...>` — non-blocking. Returns
    /// once the process is spawned, not when it exits.
    pub async fn launch_in_box(
        &self,
        box_name: &str,
        program: &str,
        args: &[String],
    ) -> Result<()> {
        let mut cmd = Command::new(&self.start_exe);
        cmd.arg(format!("/box:{box_name}")).arg(program).args(args);
        no_window(&mut cmd);
        cmd.spawn()?;
        Ok(())
    }

    /// PIDs of every process currently running inside the named box.
    /// Empty vector means the box has no processes (whether or not it
    /// exists in Sandboxie.ini). Best-effort: if Start.exe can't be
    /// queried we treat the box as empty rather than erroring, so a
    /// reconcile pass never blocks the UI.
    pub async fn list_pids(&self, box_name: &str) -> Vec<u32> {
        let mut cmd = Command::new(&self.start_exe);
        cmd.arg(format!("/box:{box_name}")).arg("/listpids");
        no_window(&mut cmd);
        let Ok(out) = cmd.output().await else {
            return Vec::new();
        };
        if !out.status.success() {
            return Vec::new();
        }
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
            .collect()
    }

    /// Names of every user-defined box that currently has at least one
    /// process running inside it. Built off `Sandboxie.ini` so it covers
    /// boxes Bridge doesn't manage too (third-party / discovered boxes
    /// still hold the kernel driver open).
    ///
    /// Used by the uninstall pre-flight — the Inno uninstaller can't
    /// tear down `SbieDrv.sys` while any sandboxed process is holding it
    /// open, and a half-failed uninstall is painful to recover from
    /// (orphaned service entries, stuck driver, often needs a reboot
    /// plus manual cleanup). Bailing out early with a list of which
    /// boxes are busy is far cheaper than letting the user fall into
    /// that pit.
    pub async fn busy_box_names(&self) -> Vec<String> {
        let Ok(boxes) = crate::ini::list_user_boxes() else {
            return Vec::new();
        };
        let mut busy = Vec::new();
        for b in boxes {
            if !self.list_pids(&b.name).await.is_empty() {
                busy.push(b.name);
            }
        }
        busy
    }

    /// True if EVE Frontier itself is running in the named box. Filters
    /// out Sandboxie's helper processes by name so a box that's "active"
    /// only because the helpers haven't timed out yet doesn't count.
    pub async fn is_game_running(&self, box_name: &str, game_exe_name: &str) -> bool {
        let pids = self.list_pids(box_name).await;
        if pids.is_empty() {
            return false;
        }
        let names = process_names_for(&pids).await;
        names
            .values()
            .any(|n| n.eq_ignore_ascii_case(game_exe_name))
    }

    /// Tear down a sandbox: remove its config from Sandboxie.ini (or
    /// disable it as a fallback) and wipe its on-disk data directory at
    /// `C:\Sandbox\<user>\<box>\`. Best-effort throughout — partial
    /// failures only log a warning.
    pub async fn delete_box(&self, box_name: &str) -> Result<()> {
        // Try to remove the entire section.
        let mut cmd = Command::new(&self.sbie_ini);
        cmd.args(["delete", box_name]);
        no_window(&mut cmd);
        let removed = cmd
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Fallback: at least disable it so it can't auto-activate.
        if !removed {
            let mut cmd = Command::new(&self.sbie_ini);
            cmd.args(["set", box_name, "Enabled", "n"]);
            no_window(&mut cmd);
            let _ = cmd.output().await;
        }

        // Wipe the data dir.
        if let Ok(username) = std::env::var("USERNAME") {
            let box_root = PathBuf::from(format!(r"C:\Sandbox\{}\{}", username, box_name));
            if box_root.exists() {
                if let Err(e) = std::fs::remove_dir_all(&box_root) {
                    tracing::warn!("delete_box: could not wipe {}: {e}", box_root.display());
                }
            }
        }

        Ok(())
    }

    /// Terminate every process running in the given box.
    /// Uses `Start.exe /box:<box> /terminate` — this is scoped to the single
    /// named box. Do NOT use `/terminate_all`: that flag is global and kills
    /// every Sandboxie process across every box, ignoring `/box:`.
    pub async fn terminate_box(&self, box_name: &str) -> Result<()> {
        let mut cmd = Command::new(&self.start_exe);
        cmd.arg(format!("/box:{box_name}")).arg("/terminate");
        no_window(&mut cmd);
        let out = cmd.output().await?;
        if !out.status.success() {
            return Err(BridgeError::SandboxieCommand(cli_failure_msg(
                &format!("Start.exe /box:{box_name} /terminate"),
                out.status,
                &out.stderr,
            )));
        }
        Ok(())
    }
}

/// Resolve a set of PIDs to image names by parsing `tasklist /fo csv`.
/// Returns an empty map if tasklist is unavailable or the call fails —
/// callers treat that as "we don't know what's running."
async fn process_names_for(pids: &[u32]) -> std::collections::HashMap<u32, String> {
    use std::collections::{HashMap, HashSet};
    let wanted: HashSet<u32> = pids.iter().copied().collect();
    let mut cmd = Command::new("tasklist.exe");
    cmd.args(["/fo", "csv", "/nh"]);
    no_window(&mut cmd);
    let Ok(out) = cmd.output().await else {
        return HashMap::new();
    };
    if !out.status.success() {
        return HashMap::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        // tasklist /fo csv /nh format:
        //   "Image Name","PID","Session Name","Session#","Mem Usage"
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0].trim_matches('"').to_string();
        if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
            if wanted.contains(&pid) {
                map.insert(pid, name);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    //! `Sandboxie::at(path)` is the gating predicate used wherever
    //! Bridge needs to ask "is Sandboxie actually usable right now?"
    //! — including `commands::sandboxes::list_sandboxes`, which
    //! short-circuits to empty when the engine isn't installed (so
    //! stale boxes left behind by Inno's "preserve user data"
    //! uninstall behaviour don't show up on the Pilots view as
    //! phantom Discovered cards). These tests pin the contract.
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn at_returns_err_when_sbieini_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let result = Sandboxie::at(tmp.path());
        assert!(
            matches!(result, Err(BridgeError::SandboxieMissing)),
            "empty dir must surface as SandboxieMissing"
        );
    }

    #[test]
    fn at_returns_err_when_only_one_binary_present() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::write(tmp.path().join("SbieIni.exe"), b"").expect("write");
        // Start.exe is still missing — must NOT report Sandboxie as
        // usable just because half the surface exists.
        let result = Sandboxie::at(tmp.path());
        assert!(matches!(result, Err(BridgeError::SandboxieMissing)));
    }

    #[test]
    fn at_succeeds_when_both_binaries_present() {
        let tmp = TempDir::new().expect("tempdir");
        std::fs::write(tmp.path().join("SbieIni.exe"), b"").expect("write SbieIni");
        std::fs::write(tmp.path().join("Start.exe"), b"").expect("write Start");
        let sb = Sandboxie::at(tmp.path()).expect("should resolve");
        assert_eq!(sb.sbie_ini, tmp.path().join("SbieIni.exe"));
        assert_eq!(sb.start_exe, tmp.path().join("Start.exe"));
    }
}
