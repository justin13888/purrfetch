//! `purr self-upgrade`: re-run the bootstrap installer for script installs.
//!
//! Only compiled with the `self-upgrade` feature, which dist enables for the
//! release archives. Those archives are shared with Homebrew and manual
//! installs, so the command also checks at runtime that the running binary is
//! the one recorded in the cargo-dist install receipt the script installers
//! write, and refuses otherwise.
//!
//! The upgrade fetches the latest `install.sh` / `install.ps1` and runs it
//! only if its SHA-256 equals the one `build.rs` compiled in from
//! `scripts/install.*`. A different installer means it changed since this
//! binary was built, so the user is pointed at the manual one-liner instead of
//! running it unattended.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// SHA-256 of the bootstrap installer for this platform, from `build.rs`.
pub const INSTALLER_SHA256: &str = env!("PURR_INSTALLER_SHA256");

/// Where the latest release's assets are served from.
pub const DEFAULT_RELEASE_URL: &str =
    "https://github.com/justin13888/purrfetch/releases/latest/download";

/// Overrides [`DEFAULT_RELEASE_URL`]; shared with the bootstrap installers.
pub const RELEASE_URL_ENV: &str = "PURRFETCH_RELEASE_URL";

/// Bootstrap installer asset name for this platform.
#[cfg(windows)]
pub const INSTALLER_NAME: &str = "install.ps1";
#[cfg(not(windows))]
pub const INSTALLER_NAME: &str = "install.sh";

/// The documented manual install command for this platform.
#[cfg(windows)]
const MANUAL_INSTALL: &str = "powershell -ExecutionPolicy Bypass -c \"irm https://github.com/justin13888/purrfetch/releases/latest/download/install.ps1 | iex\"";
#[cfg(not(windows))]
const MANUAL_INSTALL: &str = "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/justin13888/purrfetch/releases/latest/download/install.sh | sh";

const APP_NAME: &str = "purrfetch";
const REPO_OWNER: &str = "justin13888";

#[derive(Debug, Error)]
pub enum SelfUpgradeError {
    #[error(
        "purr was not installed by the script installer (no install receipt at {}).\n\
         Upgrade it through the channel you installed it with, e.g. `brew upgrade purr` \
         or `cargo install --locked purrfetch`.",
        .0.display()
    )]
    NoReceipt(PathBuf),
    #[error("could not determine where the install receipt lives (neither {0} is set)")]
    NoReceiptLocation(&'static str),
    #[error("install receipt {} is unreadable: {reason}", .path.display())]
    InvalidReceipt { path: PathBuf, reason: String },
    #[error(
        "this purr ({}) is not the copy the script installer manages ({}).\n\
         Upgrade it through the channel you installed it with, or re-run the installer:\n  {MANUAL_INSTALL}",
        .exe.display(),
        .managed.display()
    )]
    NotManaged { exe: PathBuf, managed: PathBuf },
    #[error("failed to download {url}: {reason}")]
    Download { url: String, reason: String },
    #[error(
        "the installer at {url} has changed since this purr was built.\n\
         expected sha256: {expected}\n\
         actual sha256:   {actual}\n\
         Refusing to run it automatically. Review it and re-run the installer yourself:\n  {MANUAL_INSTALL}"
    )]
    ChecksumMismatch {
        url: String,
        expected: String,
        actual: String,
    },
    #[error("the installer exited with {0}")]
    InstallerFailed(ExitStatus),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// The fields of cargo-dist's `purrfetch-receipt.json` this command relies on.
#[derive(Debug, Deserialize)]
pub struct Receipt {
    pub install_prefix: PathBuf,
    pub source: ReceiptSource,
}

#[derive(Debug, Deserialize)]
pub struct ReceiptSource {
    pub app_name: String,
    pub owner: String,
}

impl Receipt {
    /// Parse a receipt and check it belongs to this app.
    pub fn parse(json: &str) -> Result<Self, String> {
        let receipt: Receipt = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if receipt.source.app_name != APP_NAME || receipt.source.owner != REPO_OWNER {
            return Err(format!(
                "it belongs to {}/{}, not {REPO_OWNER}/{APP_NAME}",
                receipt.source.owner, receipt.source.app_name
            ));
        }
        Ok(receipt)
    }

    /// Directory the installer put `purr` in. cargo-dist only writes a
    /// receipt for its cargo-home layout (unmanaged, flat installs get none),
    /// so this is always `<prefix>/bin`.
    pub fn bin_dir(&self) -> PathBuf {
        self.install_prefix.join("bin")
    }

    /// Environment that makes the installer reinstall into the same prefix,
    /// refresh the receipt, and leave PATH alone.
    pub fn installer_env(&self) -> Vec<(&'static str, &OsStr)> {
        vec![
            ("PURRFETCH_INSTALL_DIR", self.install_prefix.as_os_str()),
            ("PURRFETCH_NO_MODIFY_PATH", OsStr::new("1")),
        ]
    }
}

/// Installer variables that would stop it refreshing the receipt (an
/// unmanaged install writes none; a disabled updater writes it without the
/// update source), so they are never passed through from the caller.
pub const ALWAYS_SCRUBBED_ENV: [&str; 2] =
    ["PURRFETCH_UNMANAGED_INSTALL", "PURRFETCH_DISABLE_UPDATE"];

/// Installer variables that change where cargo-dist's installer downloads the
/// release archive from. They are dropped unless [`RELEASE_URL_ENV`] is set:
/// that override already fetches the installer from an unverified source, so
/// letting these through as well grants no additional trust.
pub const DOWNLOAD_SOURCE_ENV: [&str; 4] = [
    "PURRFETCH_DOWNLOAD_URL",
    "INSTALLER_DOWNLOAD_URL",
    "PURRFETCH_INSTALLER_GITHUB_BASE_URL",
    "PURRFETCH_INSTALLER_GHE_BASE_URL",
];

/// Caller environment variables to remove before running the installer.
pub fn scrubbed_env(release_url_override: bool) -> Vec<&'static str> {
    let mut vars = ALWAYS_SCRUBBED_ENV.to_vec();
    if !release_url_override {
        vars.extend(DOWNLOAD_SOURCE_ENV);
    }
    vars
}

/// The installer's environment: variables to set, and caller variables to
/// remove first.
#[derive(Debug)]
pub struct InstallerEnv<'a> {
    pub set: Vec<(&'static str, &'a OsStr)>,
    pub remove: Vec<&'static str>,
}

impl InstallerEnv<'_> {
    fn apply(&self, cmd: &mut Command) {
        for var in &self.remove {
            cmd.env_remove(var);
        }
        cmd.envs(self.set.iter().copied());
    }
}

/// Where cargo-dist's installers write the receipt, mirroring their logic:
/// `XDG_CONFIG_HOME`, else `LOCALAPPDATA` on Windows or `~/.config` elsewhere.
pub fn receipt_path(var: impl Fn(&str) -> Option<String>) -> Result<PathBuf, SelfUpgradeError> {
    let var = |k: &str| var(k).filter(|v| !v.is_empty());
    let config_home =
        match var("XDG_CONFIG_HOME") {
            Some(dir) => PathBuf::from(dir),
            #[cfg(windows)]
            None => PathBuf::from(var("LOCALAPPDATA").ok_or(
                SelfUpgradeError::NoReceiptLocation("XDG_CONFIG_HOME nor LOCALAPPDATA"),
            )?),
            #[cfg(not(windows))]
            None => PathBuf::from(var("HOME").ok_or(SelfUpgradeError::NoReceiptLocation(
                "XDG_CONFIG_HOME nor HOME",
            ))?)
            .join(".config"),
        };
    Ok(config_home
        .join(APP_NAME)
        .join(format!("{APP_NAME}-receipt.json")))
}

/// URL of the bootstrap installer under `base` (a release download URL).
pub fn installer_url(base: &str) -> String {
    format!("{}/{INSTALLER_NAME}", base.trim_end_matches('/'))
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Accept `script` only if it hashes to `expected`.
pub fn verify_installer(url: &str, script: &[u8], expected: &str) -> Result<(), SelfUpgradeError> {
    let actual = sha256_hex(script);
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(SelfUpgradeError::ChecksumMismatch {
            url: url.to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

/// Run `purr self-upgrade`.
pub fn run() -> Result<(), SelfUpgradeError> {
    upgrade(
        &env::current_exe()?,
        |k| env::var(k).ok(),
        download,
        run_installer,
    )
}

/// [`run`] with the running executable, environment, downloader, and
/// installer runner passed in, so its refusals can be tested in isolation.
fn upgrade(
    exe: &Path,
    var: impl Fn(&str) -> Option<String>,
    download: impl FnOnce(&str, &Path, bool) -> Result<(), SelfUpgradeError>,
    run_installer: impl FnOnce(&Path, &InstallerEnv, &Path) -> io::Result<ExitStatus>,
) -> Result<(), SelfUpgradeError> {
    let receipt_path = receipt_path(&var)?;
    let json = match fs::read_to_string(&receipt_path) {
        Ok(json) => json,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(SelfUpgradeError::NoReceipt(receipt_path));
        }
        Err(e) => return Err(e.into()),
    };
    let receipt = Receipt::parse(&json).map_err(|reason| SelfUpgradeError::InvalidReceipt {
        path: receipt_path.clone(),
        reason,
    })?;

    // Resolve symlinks so a link to the managed binary (e.g. from another
    // directory on PATH) is judged by the file it points at. If that can't be
    // resolved, refuse rather than guess.
    let exe = fs::canonicalize(exe)?;
    let managed = receipt.bin_dir();
    if !is_same_dir(exe.parent(), &managed) {
        return Err(SelfUpgradeError::NotManaged { exe, managed });
    }

    let override_base = var(RELEASE_URL_ENV).filter(|v| !v.is_empty());
    let url = installer_url(override_base.as_deref().unwrap_or(DEFAULT_RELEASE_URL));
    if let Some(base) = &override_base {
        eprintln!(
            "warning: {RELEASE_URL_ENV} is set; fetching installers from {base} instead of GitHub"
        );
    }

    let tmp = TempDir::new()?;
    let script_path = tmp.0.join(INSTALLER_NAME);
    println!("Downloading {url}");
    download(&url, &script_path, override_base.is_some())?;
    verify_installer(&url, &fs::read(&script_path)?, INSTALLER_SHA256)?;
    println!("Installer checksum verified ({INSTALLER_SHA256})");

    let installer_env = InstallerEnv {
        set: receipt.installer_env(),
        remove: scrubbed_env(override_base.is_some()),
    };
    let status = run_installer(&script_path, &installer_env, &exe)?;
    if !status.success() {
        return Err(SelfUpgradeError::InstallerFailed(status));
    }
    println!("purr has been upgraded; run `purr --version` to confirm.");
    Ok(())
}

/// Whether `dir` and `other` name the same existing directory.
fn is_same_dir(dir: Option<&Path>, other: &Path) -> bool {
    match (dir.map(fs::canonicalize), fs::canonicalize(other)) {
        (Some(Ok(a)), Ok(b)) => a == b,
        _ => false,
    }
}

/// Fetch `url` to `dest` with the platform's own tools, the same ones the
/// installers rely on, so the binary carries no HTTP or TLS stack.
fn download(url: &str, dest: &Path, allow_http: bool) -> Result<(), SelfUpgradeError> {
    let fail = |reason: String| SelfUpgradeError::Download {
        url: url.to_string(),
        reason,
    };

    #[cfg(windows)]
    let attempts = {
        let _ = allow_http;
        let mut ps = windows_powershell();
        ps.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$ErrorActionPreference = 'Stop'; \
             Invoke-WebRequest -UseBasicParsing -Uri $env:PURR_DOWNLOAD_URL -OutFile $env:PURR_DOWNLOAD_DEST",
        ])
        .env("PURR_DOWNLOAD_URL", url)
        .env("PURR_DOWNLOAD_DEST", dest);
        vec![("powershell", ps)]
    };

    #[cfg(not(windows))]
    let attempts = {
        let protocols = if allow_http { "=https,http" } else { "=https" };
        let mut curl = Command::new("curl");
        curl.args(["--proto", protocols, "--tlsv1.2", "-fsSL", "-o"])
            .arg(dest)
            .arg(url);
        // busybox wget has no protocol restriction flags; the checksum still
        // guards whatever it fetches.
        let mut wget = Command::new("wget");
        wget.arg("-q").arg("-O").arg(dest).arg(url);
        vec![("curl", curl), ("wget", wget)]
    };

    let mut missing = Vec::new();
    for (name, mut cmd) in attempts {
        match cmd.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => return Err(fail(format!("{name} exited with {status}"))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => missing.push(name),
            Err(e) => return Err(fail(format!("could not run {name}: {e}"))),
        }
    }
    Err(fail(format!("none of {} is available", missing.join(", "))))
}

#[cfg(not(windows))]
fn run_installer(script: &Path, env: &InstallerEnv, _exe: &Path) -> io::Result<ExitStatus> {
    let mut sh = Command::new("sh");
    sh.arg(script);
    env.apply(&mut sh);
    sh.status()
}

/// Windows can't overwrite a running executable but can rename it, so move
/// `purr.exe` aside to `purr.exe.old` and put a copy of it back for the
/// installer to overwrite. A working `purr.exe` thus exists throughout, even
/// if the upgrade is interrupted. If the installer fails, or reports success
/// but leaves no `purr.exe`, the previous binary is moved back. The `.old`
/// file left by a successful upgrade is removed by the next
/// `purr self-upgrade`.
///
/// `-ExecutionPolicy Bypass` is needed even beyond running the script file:
/// cargo-dist's installer itself refuses to run unless `Get-ExecutionPolicy`
/// is `Unrestricted`, `RemoteSigned`, or `Bypass`, and Windows defaults to
/// `Restricted`. A Group Policy-enforced policy still wins, as intended.
#[cfg(windows)]
fn run_installer(script: &Path, env: &InstallerEnv, exe: &Path) -> io::Result<ExitStatus> {
    let old = exe.with_extension("exe.old");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old)?;
    if let Err(e) = fs::copy(&old, exe) {
        let _ = fs::rename(&old, exe);
        return Err(e);
    }
    let mut ps = windows_powershell();
    ps.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(script);
    env.apply(&mut ps);
    let status = ps.status();
    let succeeded = matches!(&status, Ok(s) if s.success());
    if succeeded && exe.exists() {
        return status;
    }
    // A failed install may have left a partial purr.exe; replace it.
    fs::rename(&old, exe)?;
    if succeeded {
        return Err(io::Error::other(
            "the installer reported success but did not install purr.exe; restored the previous copy",
        ));
    }
    status
}

/// `powershell.exe` (Windows PowerShell 5.1) without the caller's
/// `PSModulePath`. Launched from PowerShell 7, it would inherit pwsh's module
/// path and fail to load its own core modules (e.g. `Get-ExecutionPolicy`,
/// which the installer calls).
#[cfg(windows)]
fn windows_powershell() -> Command {
    let mut cmd = Command::new("powershell");
    cmd.env_remove("PSModulePath");
    cmd
}

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> io::Result<Self> {
        // The counter keeps names unique within a process (e.g. parallel
        // tests) even when the clock is coarser than nanoseconds.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = env::temp_dir().join(format!(
            "purr-self-upgrade-{}-{nanos}-{n}",
            std::process::id()
        ));
        // Owner-only, so other local users can't swap the installer between
        // its checksum check and its execution.
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder
        };
        #[cfg(not(unix))]
        let builder = fs::DirBuilder::new();
        builder.create(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECEIPT: &str = r#"{"binaries":["purr"],"binary_aliases":{},"cdylibs":[],"cstaticlibs":[],"install_layout":"cargo-home","install_prefix":"/home/u/.cargo","modify_path":true,"provider":{"source":"cargo-dist","version":"0.32.0"},"source":{"app_name":"purrfetch","name":"purrfetch","owner":"justin13888","release_type":"github"},"version":"1.1.0"}"#;

    /// SHA-256 of the bootstrap installers that released binaries trust.
    /// Changing a script breaks `purr self-upgrade` for every binary released
    /// before the change, so update these only for a deliberate, release-noted
    /// break (see docs/releasing.md).
    const PINNED_INSTALL_SH_SHA256: &str =
        "b99891ff5fe8034270868f726d0303adc2b8971ae0151134e53c81d467977974";
    const PINNED_INSTALL_PS1_SHA256: &str =
        "463bc04ddd744f4399fa289d3651a368b8554ae8a64a1244fb592b0fafb969a0";

    fn bootstrap_script(name: &str) -> Vec<u8> {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("scripts")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn embedded_checksum_matches_bootstrap_script() {
        assert_eq!(
            sha256_hex(&bootstrap_script(INSTALLER_NAME)),
            INSTALLER_SHA256
        );
    }

    #[test]
    fn bootstrap_scripts_match_pinned_hashes() {
        assert_eq!(
            sha256_hex(&bootstrap_script("install.sh")),
            PINNED_INSTALL_SH_SHA256,
            "scripts/install.sh changed; released binaries will refuse to self-upgrade"
        );
        assert_eq!(
            sha256_hex(&bootstrap_script("install.ps1")),
            PINNED_INSTALL_PS1_SHA256,
            "scripts/install.ps1 changed; released binaries will refuse to self-upgrade"
        );
    }

    /// A fake script install under a temp dir: `config/` stands in for
    /// `XDG_CONFIG_HOME` and `prefix/bin/purr` for the managed binary.
    struct Install {
        root: TempDir,
    }

    impl Install {
        fn new() -> Self {
            let install = Self {
                root: TempDir::new().unwrap(),
            };
            fs::create_dir_all(install.prefix().join("bin")).unwrap();
            fs::create_dir_all(install.config().join(APP_NAME)).unwrap();
            fs::write(install.exe(), b"").unwrap();
            install
        }

        fn config(&self) -> PathBuf {
            self.root.0.join("config")
        }

        fn prefix(&self) -> PathBuf {
            self.root.0.join("prefix")
        }

        fn exe(&self) -> PathBuf {
            self.prefix()
                .join("bin")
                .join(format!("purr{}", env::consts::EXE_SUFFIX))
        }

        fn receipt_path(&self) -> PathBuf {
            self.config()
                .join(APP_NAME)
                .join(format!("{APP_NAME}-receipt.json"))
        }

        /// Write a valid receipt pointing at [`Install::prefix`].
        fn write_receipt(&self) {
            let mut json: serde_json::Value = serde_json::from_str(RECEIPT).unwrap();
            json["install_prefix"] = self.prefix().to_str().unwrap().into();
            fs::write(self.receipt_path(), json.to_string()).unwrap();
        }

        fn var(&self) -> impl Fn(&str) -> Option<String> + '_ {
            |k| (k == "XDG_CONFIG_HOME").then(|| self.config().to_str().unwrap().to_string())
        }
    }

    fn no_download(_: &str, _: &Path, _: bool) -> Result<(), SelfUpgradeError> {
        panic!("must not download the installer");
    }

    fn no_installer(_: &Path, _: &InstallerEnv, _: &Path) -> io::Result<ExitStatus> {
        panic!("must not run the installer");
    }

    fn exit_success() -> ExitStatus {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }

    #[test]
    fn upgrade_refuses_without_receipt() {
        let install = Install::new();
        match upgrade(&install.exe(), install.var(), no_download, no_installer) {
            Err(SelfUpgradeError::NoReceipt(path)) => assert_eq!(path, install.receipt_path()),
            other => panic!("expected NoReceipt, got {other:?}"),
        }
    }

    #[test]
    fn upgrade_refuses_invalid_receipt() {
        let install = Install::new();
        fs::write(install.receipt_path(), "not json").unwrap();
        match upgrade(&install.exe(), install.var(), no_download, no_installer) {
            Err(SelfUpgradeError::InvalidReceipt { path, .. }) => {
                assert_eq!(path, install.receipt_path())
            }
            other => panic!("expected InvalidReceipt, got {other:?}"),
        }
    }

    #[test]
    fn upgrade_refuses_copy_outside_managed_dir() {
        let install = Install::new();
        install.write_receipt();
        let other_dir = install.root.0.join("elsewhere");
        fs::create_dir(&other_dir).unwrap();
        let other = other_dir.join(install.exe().file_name().unwrap());
        fs::write(&other, b"").unwrap();
        match upgrade(&other, install.var(), no_download, no_installer) {
            Err(SelfUpgradeError::NotManaged { exe, managed }) => {
                assert_eq!(exe, fs::canonicalize(&other).unwrap());
                assert_eq!(managed, install.prefix().join("bin"));
            }
            other => panic!("expected NotManaged, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn upgrade_follows_symlink_to_managed_copy() {
        let install = Install::new();
        install.write_receipt();
        let link_dir = install.root.0.join("on-path");
        fs::create_dir(&link_dir).unwrap();
        let link = link_dir.join("purr");
        std::os::unix::fs::symlink(install.exe(), &link).unwrap();
        let script = bootstrap_script(INSTALLER_NAME);
        let mut ran_with = None;
        upgrade(
            &link,
            install.var(),
            |_, dest, _| Ok(fs::write(dest, &script)?),
            |_, _, exe| {
                ran_with = Some(exe.to_path_buf());
                Ok(exit_success())
            },
        )
        .unwrap();
        assert_eq!(ran_with, Some(fs::canonicalize(install.exe()).unwrap()));

        // A link that can't be resolved is refused rather than guessed at.
        let dangling = link_dir.join("dangling");
        std::os::unix::fs::symlink(install.root.0.join("missing"), &dangling).unwrap();
        assert!(matches!(
            upgrade(&dangling, install.var(), no_download, no_installer),
            Err(SelfUpgradeError::Io(_))
        ));
    }

    #[test]
    fn upgrade_refuses_changed_installer() {
        let install = Install::new();
        install.write_receipt();
        let result = upgrade(
            &install.exe(),
            install.var(),
            |_, dest, _| Ok(fs::write(dest, b"echo pwned")?),
            no_installer,
        );
        match result {
            Err(SelfUpgradeError::ChecksumMismatch {
                url,
                expected,
                actual,
            }) => {
                assert_eq!(url, installer_url(DEFAULT_RELEASE_URL));
                assert_eq!(expected, INSTALLER_SHA256);
                assert_eq!(actual, sha256_hex(b"echo pwned"));
            }
            other => panic!("expected ChecksumMismatch, got {other:?}"),
        }
    }

    #[test]
    fn upgrade_runs_verified_installer_into_receipt_prefix() {
        let install = Install::new();
        install.write_receipt();
        let script = bootstrap_script(INSTALLER_NAME);
        let mut fetched = None;
        let mut ran = None;
        upgrade(
            &install.exe(),
            install.var(),
            |url, dest, allow_http| {
                fetched = Some((url.to_string(), allow_http));
                Ok(fs::write(dest, &script)?)
            },
            |path, env, _| {
                let set: Vec<_> = env
                    .set
                    .iter()
                    .map(|&(k, v)| (k, v.to_os_string()))
                    .collect();
                ran = Some((sha256_hex(&fs::read(path)?), set, env.remove.clone()));
                Ok(exit_success())
            },
        )
        .unwrap();
        assert_eq!(fetched, Some((installer_url(DEFAULT_RELEASE_URL), false)));
        let (ran_hash, set, remove) = ran.expect("the installer should run");
        assert_eq!(ran_hash, INSTALLER_SHA256);
        assert!(set.contains(&("PURRFETCH_INSTALL_DIR", install.prefix().into_os_string())));
        assert!(set.contains(&("PURRFETCH_NO_MODIFY_PATH", "1".into())));
        assert_eq!(remove, scrubbed_env(false));
    }

    #[test]
    fn upgrade_with_release_url_keeps_download_source() {
        let install = Install::new();
        install.write_receipt();
        let script = bootstrap_script(INSTALLER_NAME);
        let mut fetched = None;
        let mut removed = None;
        upgrade(
            &install.exe(),
            |k| match k {
                RELEASE_URL_ENV => Some("http://127.0.0.1:8765/".to_string()),
                _ => install.var()(k),
            },
            |url, dest, allow_http| {
                fetched = Some((url.to_string(), allow_http));
                Ok(fs::write(dest, &script)?)
            },
            |_, env, _| {
                removed = Some(env.remove.clone());
                Ok(exit_success())
            },
        )
        .unwrap();
        assert_eq!(
            fetched,
            Some((installer_url("http://127.0.0.1:8765"), true))
        );
        assert_eq!(removed, Some(scrubbed_env(true)));
    }

    #[test]
    fn scrubbed_env_keeps_download_source_only_with_release_url() {
        let without = scrubbed_env(false);
        let with = scrubbed_env(true);
        for var in ALWAYS_SCRUBBED_ENV {
            assert!(without.contains(&var) && with.contains(&var), "{var}");
        }
        for var in DOWNLOAD_SOURCE_ENV {
            assert!(without.contains(&var), "{var} should be scrubbed");
            assert!(!with.contains(&var), "{var} should pass through");
        }
        assert!(!without.contains(&RELEASE_URL_ENV));
        assert!(!without.contains(&"PURRFETCH_NO_MODIFY_PATH"));
    }

    #[test]
    fn installer_env_removes_then_sets() {
        let env = InstallerEnv {
            set: vec![("PURRFETCH_INSTALL_DIR", OsStr::new("/p"))],
            remove: scrubbed_env(false),
        };
        let mut cmd = Command::new("sh");
        env.apply(&mut cmd);
        let applied: Vec<_> = cmd.get_envs().collect();
        for var in scrubbed_env(false) {
            assert!(applied.contains(&(OsStr::new(var), None)), "{var}");
        }
        assert!(applied.contains(&(OsStr::new("PURRFETCH_INSTALL_DIR"), Some(OsStr::new("/p")))));
    }

    #[test]
    fn sha256_hex_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verify_installer_accepts_match_and_rejects_change() {
        let expected = sha256_hex(b"echo install");
        assert!(verify_installer("u", b"echo install", &expected).is_ok());
        assert!(verify_installer("u", b"echo install", &expected.to_uppercase()).is_ok());
        match verify_installer("u", b"echo pwned", &expected) {
            Err(SelfUpgradeError::ChecksumMismatch {
                expected: e,
                actual,
                ..
            }) => {
                assert_eq!(e, expected);
                assert_eq!(actual, sha256_hex(b"echo pwned"));
            }
            other => panic!("expected a checksum mismatch, got {other:?}"),
        }
    }

    #[test]
    fn installer_url_joins_base() {
        assert_eq!(
            installer_url(DEFAULT_RELEASE_URL),
            format!("{DEFAULT_RELEASE_URL}/{INSTALLER_NAME}")
        );
        assert_eq!(
            installer_url("http://127.0.0.1:8765/"),
            format!("http://127.0.0.1:8765/{INSTALLER_NAME}")
        );
    }

    #[test]
    fn receipt_cargo_home_layout() {
        let receipt = Receipt::parse(RECEIPT).unwrap();
        assert_eq!(receipt.bin_dir(), Path::new("/home/u/.cargo").join("bin"));
        let env = receipt.installer_env();
        assert_eq!(
            env[0],
            ("PURRFETCH_INSTALL_DIR", OsStr::new("/home/u/.cargo"))
        );
        assert_eq!(env[1], ("PURRFETCH_NO_MODIFY_PATH", OsStr::new("1")));
    }

    #[test]
    fn receipt_for_another_app_is_rejected() {
        let json = RECEIPT.replace("\"app_name\":\"purrfetch\"", "\"app_name\":\"other\"");
        assert!(Receipt::parse(&json).is_err());
        assert!(Receipt::parse("not json").is_err());
    }

    #[test]
    fn receipt_path_prefers_xdg_config_home() {
        let path = receipt_path(|k| (k == "XDG_CONFIG_HOME").then(|| "/cfg".to_string())).unwrap();
        assert_eq!(
            path,
            Path::new("/cfg")
                .join("purrfetch")
                .join("purrfetch-receipt.json")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn receipt_path_falls_back_to_home_config() {
        let path = receipt_path(|k| match k {
            "XDG_CONFIG_HOME" => Some(String::new()),
            "HOME" => Some("/home/u".to_string()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            path,
            Path::new("/home/u/.config/purrfetch/purrfetch-receipt.json")
        );
        assert!(receipt_path(|_| None).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn receipt_path_falls_back_to_local_app_data() {
        let path = receipt_path(|k| {
            (k == "LOCALAPPDATA").then(|| r"C:\Users\u\AppData\Local".to_string())
        })
        .unwrap();
        assert_eq!(
            path,
            Path::new(r"C:\Users\u\AppData\Local\purrfetch\purrfetch-receipt.json")
        );
    }

    #[cfg(unix)]
    #[test]
    fn temp_dir_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let mode = fs::metadata(&tmp.0).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn same_dir_requires_existing_match() {
        let tmp = TempDir::new().unwrap();
        assert!(is_same_dir(Some(&tmp.0), &tmp.0.join(".")));
        assert!(!is_same_dir(Some(&tmp.0), &tmp.0.join("missing")));
        assert!(!is_same_dir(None, &tmp.0));
    }
}
