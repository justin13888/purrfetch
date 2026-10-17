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
    let receipt_path = receipt_path(|k| env::var(k).ok())?;
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
    let exe = fs::canonicalize(env::current_exe()?)?;
    let managed = receipt.bin_dir();
    if !is_same_dir(exe.parent(), &managed) {
        return Err(SelfUpgradeError::NotManaged { exe, managed });
    }

    let override_base = env::var(RELEASE_URL_ENV).ok().filter(|v| !v.is_empty());
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

    let status = run_installer(&script_path, &receipt, &exe)?;
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
fn run_installer(script: &Path, receipt: &Receipt, _exe: &Path) -> io::Result<ExitStatus> {
    Command::new("sh")
        .arg(script)
        .envs(receipt.installer_env())
        .status()
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
fn run_installer(script: &Path, receipt: &Receipt, exe: &Path) -> io::Result<ExitStatus> {
    let old = exe.with_extension("exe.old");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old)?;
    if let Err(e) = fs::copy(&old, exe) {
        let _ = fs::rename(&old, exe);
        return Err(e);
    }
    let status = windows_powershell()
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .envs(receipt.installer_env())
        .status();
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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = env::temp_dir().join(format!("purr-self-upgrade-{}-{nanos}", std::process::id()));
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

    #[test]
    fn embedded_checksum_matches_bootstrap_script() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join(INSTALLER_NAME);
        assert_eq!(sha256_hex(&fs::read(script).unwrap()), INSTALLER_SHA256);
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
