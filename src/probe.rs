use std::{
    fmt::{self, Display, Formatter},
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Command,
    sync::OnceLock,
};

use tracing::{debug_span, instrument};

#[cfg(not(target_os = "macos"))]
use libmacchina::traits::{ShellFormat, ShellKind};
use libmacchina::{
    BatteryReadout, GeneralReadout, KernelReadout, MemoryReadout, NetworkReadout, PackageReadout,
    ProductReadout,
    traits::{BatteryState, ReadoutError},
};
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, Users};
use thiserror::Error;

use crate::config::{
    CoresMode, CpuOptions, DeOptions, DiskOptions, DistroOptions, GpuOptions, GpuType,
    KernelOptions, MemoryOptions, PackagesOptions, ShellOptions, SongOptions, SpeedType,
    UptimeOptions,
};

pub fn battery_readout() -> &'static BatteryReadout {
    use libmacchina::traits::BatteryReadout as _;
    static COMPUTATION: OnceLock<BatteryReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "battery").entered();
        BatteryReadout::new()
    })
}

pub fn kernel_readout() -> &'static KernelReadout {
    use libmacchina::traits::KernelReadout as _;
    static COMPUTATION: OnceLock<KernelReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "kernel").entered();
        KernelReadout::new()
    })
}

pub fn memory_readout() -> &'static MemoryReadout {
    use libmacchina::traits::MemoryReadout as _;
    static COMPUTATION: OnceLock<MemoryReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "memory").entered();
        MemoryReadout::new()
    })
}

pub fn general_readout() -> &'static GeneralReadout {
    use libmacchina::traits::GeneralReadout as _;
    static COMPUTATION: OnceLock<GeneralReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "general").entered();
        GeneralReadout::new()
    })
}

pub fn product_readout() -> &'static ProductReadout {
    use libmacchina::traits::ProductReadout as _;
    static COMPUTATION: OnceLock<ProductReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "product").entered();
        ProductReadout::new()
    })
}

pub fn package_readout() -> &'static PackageReadout {
    use libmacchina::traits::PackageReadout as _;
    static COMPUTATION: OnceLock<PackageReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "package").entered();
        PackageReadout::new()
    })
}

pub fn network_readout() -> &'static NetworkReadout {
    use libmacchina::traits::NetworkReadout as _;
    static COMPUTATION: OnceLock<NetworkReadout> = OnceLock::new();
    COMPUTATION.get_or_init(|| {
        let _span = debug_span!("init_readout", kind = "network").entered();
        NetworkReadout::new()
    })
}

/// Run `gsettings get <schema> <key>` and return the trimmed output with surrounding quotes removed.
fn gsettings_get(schema: &str, key: &str) -> Result<String, ProbeError> {
    let _span = debug_span!("subprocess", cmd = "gsettings").entered();
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .map_err(|_| ProbeError::MetricsUnavailable)?;
    if !output.status.success() {
        return Err(ProbeError::MetricsUnavailable);
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('\'')
        .to_string();
    if value.is_empty() {
        return Err(ProbeError::MetricsUnavailable);
    }
    Ok(value)
}

/// Run a command and return stdout trimmed, or Err if it fails or produces empty output.
fn run_command(cmd: &str, args: &[&str]) -> Result<String, ProbeError> {
    let _span = debug_span!("subprocess", cmd = %cmd).entered();
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|_| ProbeError::MetricsUnavailable)?;
    if !output.status.success() {
        return Err(ProbeError::MetricsUnavailable);
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Err(ProbeError::MetricsUnavailable)
    } else {
        Ok(value)
    }
}

// ── macOS subprocess wrappers (used by several probes) ───────────────────
/// `sysctl -n <key>`.
#[cfg(target_os = "macos")]
fn sysctl(key: &str) -> Result<String, ProbeError> {
    run_command("sysctl", &["-n", key])
}

/// `sw_vers <flag>`, e.g. `-productVersion` / `-buildVersion`.
#[cfg(target_os = "macos")]
fn sw_vers(flag: &str) -> Result<String, ProbeError> {
    run_command("sw_vers", &[flag])
}

/// Read a global preference via `defaults read -g <key>`. Returns `None` when the
/// key is unset (the command exits non-zero), mirroring neofetch's fallbacks.
#[cfg(target_os = "macos")]
fn defaults_read_global(key: &str) -> Option<String> {
    run_command("defaults", &["read", "-g", key]).ok()
}

// ── Pure, unit-testable parsers (no platform calls) ──────────────────────
/// Reduce libmacchina's resolution string to neofetch's logical resolution:
/// "3456x2234@120fps (as 1728x1117)" -> "1728x1117"; a plain "1920x1080" is
/// passed through; multiple (newline-separated) displays rejoin with ", ".
#[cfg(any(target_os = "macos", test))]
fn scaled_resolution(raw: &str) -> String {
    raw.lines()
        .map(|seg| {
            let seg = seg.trim();
            if let Some(start) = seg.find("(as ") {
                let rest = &seg[start + 4..];
                rest.split(')').next().unwrap_or(rest).trim().to_string()
            } else {
                seg.split('@').next().unwrap_or(seg).trim().to_string()
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Map a macOS `AppleAccentColor` value to neofetch's colour name.
#[cfg(any(target_os = "macos", test))]
fn map_accent(value: Option<&str>) -> &'static str {
    match value.map(str::trim) {
        Some("-1") => "Graphite",
        Some("0") => "Red",
        Some("1") => "Orange",
        Some("2") => "Yellow",
        Some("3") => "Green",
        Some("5") => "Purple",
        Some("6") => "Pink",
        _ => "Blue",
    }
}

/// Map `$TERM_PROGRAM` to neofetch's terminal name (case preserved for the
/// generic `.app` strip).
#[cfg(any(target_os = "macos", test))]
fn map_terminal(term_program: &str) -> String {
    match term_program {
        "iTerm.app" => "iTerm2".to_string(),
        "Terminal.app" | "Apple_Terminal" => "Apple Terminal".to_string(),
        "Hyper" => "HyperTerm".to_string(),
        other => other.trim_end_matches(".app").to_string(),
    }
}

/// Extract "Chipset Model:" values from `system_profiler SPDisplaysDataType`.
#[cfg(any(target_os = "macos", test))]
fn parse_chipset_models(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("Chipset Model:")
                .map(|v| v.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse `(wired, active, compressed)` page counts from `vm_stat` output.
#[cfg(any(target_os = "macos", test))]
fn parse_vm_stat_pages(output: &str) -> Option<(u64, u64, u64)> {
    let get = |needle: &str| -> Option<u64> {
        output.lines().find_map(|line| {
            line.trim().strip_prefix(needle).and_then(|rest| {
                let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
                digits.parse::<u64>().ok()
            })
        })
    };
    Some((
        get("Pages wired down:")?,
        get("Pages active:")?,
        get("Pages occupied by compressor:")?,
    ))
}

/// Parse the page size (bytes) from `vm_stat`'s header line
/// ("...(page size of 16384 bytes)").
#[cfg(any(target_os = "macos", test))]
fn parse_vm_stat_page_size(output: &str) -> Option<u64> {
    let line = output.lines().next()?;
    let rest = &line[line.find("page size of ")? + "page size of ".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u64>().ok()
}

/// Per-manager package count for `port installed` output, matching neofetch's
/// `${#pkgs[@]}` (every non-empty line, including the header line).
#[cfg(any(target_os = "macos", test))]
fn parse_port_count(output: &str) -> usize {
    output.lines().filter(|l| !l.trim().is_empty()).count()
}

/// Count Homebrew formulae as the number of (non-hidden) directories under the
/// Cellar, matching neofetch's `dir "$(brew --cellar)"/*` (casks excluded).
#[cfg(target_os = "macos")]
fn count_cellar_formulae() -> usize {
    let cellars: Vec<String> = match run_command("brew", &["--cellar"]) {
        Ok(path) => vec![path],
        Err(_) => vec![
            "/opt/homebrew/Cellar".to_string(),
            "/usr/local/Cellar".to_string(),
        ],
    };
    cellars
        .iter()
        .filter_map(|p| std::fs::read_dir(p).ok())
        .flat_map(|rd| rd.flatten())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .count()
}

/// macOS package counts mirroring neofetch: MacPorts then Homebrew (Cellar),
/// labelled "port"/"brew"; cargo and other language managers are not counted.
#[cfg(target_os = "macos")]
fn count_packages_macos() -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    if let Ok(out) = run_command("port", &["installed"]) {
        let n = parse_port_count(&out);
        if n > 0 {
            counts.push(("port".to_string(), n));
        }
    }
    let brew = count_cellar_formulae();
    if brew > 0 {
        counts.push(("brew".to_string(), brew));
    }
    counts
}

/// GPU names for this platform: `system_profiler` chipset models on macOS,
/// libmacchina's `gpus()` elsewhere.
#[cfg(target_os = "macos")]
fn gpus_list() -> Result<Vec<String>, ProbeError> {
    let out = run_command("system_profiler", &["SPDisplaysDataType"])?;
    let models = parse_chipset_models(&out);
    if models.is_empty() {
        Err(ProbeError::MetricsUnavailable)
    } else {
        Ok(models)
    }
}

#[cfg(not(target_os = "macos"))]
fn gpus_list() -> Result<Vec<String>, ProbeError> {
    use libmacchina::traits::GeneralReadout as _;
    Ok(general_readout().gpus()?)
}

/// Parse a key from an INI-style config file (searches all sections).
#[instrument(level = "debug", fields(path = %path.display(), key))]
fn parse_ini_key(path: &std::path::Path, key: &str) -> Result<String, ProbeError> {
    let file = std::fs::File::open(path).map_err(|_| ProbeError::MetricsUnavailable)?;
    let needle = format!("{}=", key);
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix(&needle) {
            let v = value.trim().to_string();
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    Err(ProbeError::MetricsUnavailable)
}

/// Resolve a GTK theme/icon setting and append neofetch's `[GTK…]` version tag
/// (`get_style` with the default `gtk_shorthand=off`). GNOME-style DEs expose a
/// single value via gsettings that neofetch treats as both GTK2 and GTK3
/// (`[GTK2/3]`); the gtk-3.0 ini fallback yields `[GTK3]`.
fn gtk_style(gsettings_key: &str, ini_key: &str) -> Result<String, ProbeError> {
    if let Ok(v) = gsettings_get("org.gnome.desktop.interface", gsettings_key) {
        return Ok(with_gtk_tag(&v, true));
    }
    let home = std::env::var("HOME").map_err(|_| ProbeError::MetricsUnavailable)?;
    let v = parse_ini_key(
        std::path::Path::new(&format!("{home}/.config/gtk-3.0/settings.ini")),
        ini_key,
    )?;
    Ok(with_gtk_tag(&v, false))
}

/// Append the GTK version tag: `[GTK2/3]` when the value came from gsettings,
/// else `[GTK3]`.
fn with_gtk_tag(value: &str, from_gsettings: bool) -> String {
    let tag = if from_gsettings { "[GTK2/3]" } else { "[GTK3]" };
    format!("{value} {tag}")
}

/// Apply neofetch's window-manager renames (`*GNOME*Shell*` -> "Mutter",
/// `*WINDOWMAKER*` -> "wmaker"). Done case-insensitively so libmacchina's
/// lowercase process name "gnome-shell" maps to "Mutter" like neofetch.
fn normalize_wm(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.contains("gnome") && lower.contains("shell") {
        "Mutter".to_string()
    } else if lower.contains("windowmaker") {
        "wmaker".to_string()
    } else {
        name.to_string()
    }
}

#[instrument(level = "debug")]
fn detect_terminal_font() -> Result<String, ProbeError> {
    use libmacchina::traits::GeneralReadout as _;
    let terminal = general_readout()
        .terminal()
        .map_err(|_| ProbeError::MetricsUnavailable)?
        .trim()
        .to_lowercase();

    match terminal.as_str() {
        "gnome-terminal-server" | "gnome-terminal" => {
            // Get the default profile ID then query the font setting
            let profile = gsettings_get("org.gnome.Terminal.ProfilesList", "default")?;
            let schema = format!(
                "org.gnome.Terminal.Legacy.Profile:/org/gnome/terminal/legacy/profiles:/:{}/",
                profile
            );
            // Only use custom font if use-custom-command is false (i.e., custom font is enabled)
            let use_system =
                gsettings_get(&schema, "use-system-font").unwrap_or_else(|_| "true".to_string());
            if use_system == "true" {
                return Err(ProbeError::MetricsUnavailable);
            }
            gsettings_get(&schema, "font")
        }
        "kgx" | "gnome-console" => {
            // GNOME Console uses system font by default; only has a font if customized
            let use_system = gsettings_get("org.gnome.Console", "use-system-font")
                .unwrap_or_else(|_| "true".to_string());
            if use_system == "true" {
                return Err(ProbeError::MetricsUnavailable);
            }
            gsettings_get("org.gnome.Console", "custom-font")
        }
        "alacritty" => {
            let home = std::env::var("HOME").map_err(|_| ProbeError::MetricsUnavailable)?;
            // Try TOML config first, then YAML
            for config_path in &[
                format!("{}/.config/alacritty/alacritty.toml", home),
                format!("{}/.alacritty.toml", home),
                format!("{}/.config/alacritty/alacritty.yml", home),
                format!("{}/.alacritty.yml", home),
            ] {
                let path = std::path::Path::new(config_path);
                if path.exists()
                    && let Ok(family) = parse_ini_key(path, "family")
                {
                    return Ok(family);
                }
            }
            Err(ProbeError::MetricsUnavailable)
        }
        "kitty" => {
            let home = std::env::var("HOME").map_err(|_| ProbeError::MetricsUnavailable)?;
            let config_path = format!("{}/.config/kitty/kitty.conf", home);
            parse_ini_key(std::path::Path::new(&config_path), "font_family")
        }
        _ => Err(ProbeError::MetricsUnavailable),
    }
}

// TODO: Complete the rest of doc comments for this enum vv
pub enum ProbeValue {
    /// Hostname (username@hostname)
    /// e.g. ("justin13888", "purr")
    Host(String, String),
    /// e.g. "Ubuntu 22.04.4 LTS (Jammy Jellyfish)"
    OS(String),
    /// OS Distribution
    /// E.g.
    Distro(String),
    /// Model
    // (Vendor, Product)
    /// e.g. ("Dell Inc.", "XPS 15 9510")
    Model(String, String),
    /// e.g. "6.8.4-cachyos"
    Kernel(String),
    /// Uptime in seconds
    /// E.g. 123
    Uptime(usize),
    /// Number of packages installed
    /// Vec<(package manager, count)>
    /// E.g. [("dpkg", 123)]
    Packages(Vec<(String, usize)>),
    /// E.g. "zsh 5.8.1"
    /// Shell name + optional version, e.g. ("zsh", Some("5.9")).
    Shell(String, Option<String>),
    /// E.g. "vim 8.2" // TODO: CHECK THIS example
    Editor(String),
    /// E.g. "1920x1080"
    Resolution(String),
    /// E.g. "GNOME", "hyprland", "Fluent" (Windows)
    /// Desktop environment name + optional version, e.g. ("GNOME", Some("46.0")).
    DE(String, Option<String>),
    /// E.g. "Mutter"
    WM(String),
    /// E.g. "Adwaita" // TODO: CHECK
    WMTheme(String),
    /// E.g. "Adwaita-dark" // TODO: CHECK
    Theme(String),
    Icons(String),
    Cursor(String),
    Terminal(String),
    TerminalFont(String),
    /// E.g. "Intel Core i7-11800H"
    CPU(String),
    /// E.g. "NVIDIA GeForce RTX 4090", "Intel(R) UHD Graphics"
    GPU(String),
    /// Amount of memory (in MiB)
    /// (used, total)
    /// E.g. (46863, 64290)
    Memory(u64, u64),
    Network(String),
    Bluetooth(String),
    BIOS(String),
    /// E.g. "bochs-drm"
    GPUDriver(String),
    /// CPU usage percentage
    /// E.g. 12
    CPUUsage(usize),
    /// Disk usage (in bytes)
    /// (mountpoint, device name, used, total)
    Disk(PathBuf, String, u64, u64),
    /// Battery percentage
    /// E.g. 86
    Battery(u8), // TODO: CHECK
    PowerAdapter(String), // TODO: CHECK
    Font(String),
    Song(String),
    LocalIP(String),  // TODO: CHECK
    PublicIP(String), // TODO: CHECK
    /// List of users
    Users(Vec<String>),
    /// E.g. "en_US.UTF-8"
    Locale(String),
    /// Java version
    /// E.g. "OpenJDK 11.0.12"
    Java(String),
    /// Python version
    /// E.g. "Python 3.9.7"
    Python(String),
    /// NodeJS version
    /// E.g. "20.9.0"
    Node(String),
    /// Rust version
    /// E.g. "rustc 1.57.0"
    Rust(String),
}

#[derive(Error, Debug)]
pub enum ProbeError {
    /// Metric is unavailable on this platform
    /// e.g. "Battery percentage"
    MetricsUnavailable,
    /// Metric is unimplemented yet
    Unimplemented,
    /// Metric readout failed possibly because of missing dependencies or some criteria
    Other(String),
    /// Metric readout might be erroneous
    Warning(String),
}

impl Display for ProbeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ProbeError::MetricsUnavailable => write!(f, "Metrics unavailable"),
            ProbeError::Unimplemented => write!(f, "Unimplemented"),
            ProbeError::Other(s) => write!(f, "{}", s),
            ProbeError::Warning(s) => write!(f, "{}", s),
        }
    }
}

impl From<ReadoutError> for ProbeError {
    fn from(err: ReadoutError) -> Self {
        match err {
            ReadoutError::MetricNotAvailable => ProbeError::MetricsUnavailable,
            ReadoutError::NotImplemented => ProbeError::Unimplemented,
            ReadoutError::Other(s) => ProbeError::Other(s),
            ReadoutError::Warning(s) => ProbeError::Warning(s),
        }
    }
}

impl From<ProbeType> for ProbeResultFunction {
    fn from(probe_type: ProbeType) -> Self {
        use libmacchina::traits::BatteryReadout as _;
        use libmacchina::traits::GeneralReadout as _;
        use libmacchina::traits::KernelReadout as _;
        #[cfg(not(target_os = "macos"))]
        use libmacchina::traits::MemoryReadout as _;
        #[cfg(not(target_os = "macos"))]
        use libmacchina::traits::NetworkReadout as _;
        #[cfg(not(target_os = "macos"))]
        use libmacchina::traits::PackageReadout as _;

        match probe_type {
            ProbeType::Host => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::Host(
                    general_readout().username()?,
                    general_readout().hostname()?,
                )))
            }),
            ProbeType::OS => Box::new(|| {
                // macOS: build the OS string the way neofetch does — codename
                // ("macOS" for modern releases) + `sw_vers` product version +
                // build. The architecture is appended later by `DistroOptions`.
                #[cfg(target_os = "macos")]
                {
                    let version = sw_vers("-productVersion")?;
                    let build = sw_vers("-buildVersion")?;
                    Ok(ProbeResultValue::Single(ProbeValue::OS(format!(
                        "macOS {version} {build}"
                    ))))
                }
                // libmacchina's `os_name` is unimplemented on Linux, so fall back to the
                // distribution pretty-name (e.g. "Fedora Linux 44 (Silverblue)"). The
                // architecture and shorthand are applied later by `DistroOptions`.
                #[cfg(not(target_os = "macos"))]
                {
                    let name = general_readout()
                        .os_name()
                        .or_else(|_| general_readout().distribution())?;
                    Ok(ProbeResultValue::Single(ProbeValue::OS(name)))
                }
            }),
            ProbeType::Distro => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::Distro(
                    general_readout().distribution()?,
                )))
            }),
            ProbeType::Model => Box::new(|| {
                // macOS reports just the model identifier (`sysctl hw.model`,
                // e.g. "Mac15,7"), with no vendor, matching neofetch.
                #[cfg(target_os = "macos")]
                {
                    Ok(ProbeResultValue::Single(ProbeValue::Model(
                        String::new(),
                        clean_model(&sysctl("hw.model")?),
                    )))
                }
                // neofetch sources the model from DMI sysfs — preferring the
                // motherboard (`board_vendor`/`board_name`) over the chassis
                // product, then strips dummy OEM placeholders. libmacchina only
                // exposes the product name, so read the sysfs files directly and
                // fall back to libmacchina when they're unavailable.
                #[cfg(not(target_os = "macos"))]
                {
                    #[cfg(target_os = "linux")]
                    let raw = read_dmi_model().or_else(libmacchina_model);
                    #[cfg(not(target_os = "linux"))]
                    let raw = libmacchina_model();

                    match raw.map(|s| clean_model(&s)).filter(|s| !s.is_empty()) {
                        Some(model) => Ok(ProbeResultValue::Single(ProbeValue::Model(
                            String::new(),
                            model,
                        ))),
                        None => Err(ProbeError::MetricsUnavailable),
                    }
                }
            }),
            ProbeType::Kernel => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::Kernel(
                    kernel_readout().os_release()?,
                )))
            }),
            ProbeType::Uptime => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::Uptime(
                    general_readout().uptime()?,
                )))
            }),
            // TODO: Test libmacchina packages() function for package manager hanging issues
            ProbeType::Packages => Box::new(|| {
                let _span = debug_span!("pkg_count").entered();
                // macOS: count MacPorts + Homebrew (Cellar) like neofetch;
                // libmacchina otherwise includes casks/cargo and mislabels brew.
                #[cfg(target_os = "macos")]
                let counts = count_packages_macos();
                #[cfg(not(target_os = "macos"))]
                let counts = package_readout()
                    .count_pkgs()
                    .into_iter()
                    .map(|(name, count)| (name.to_string(), count))
                    .collect::<Vec<_>>();
                Ok(ProbeResultValue::Single(ProbeValue::Packages(counts)))
            }),
            ProbeType::Shell => Box::new(|| {
                // macOS: libmacchina's `shell()` errors, so derive the name from
                // `$SHELL` like neofetch (`${SHELL##*/}`).
                #[cfg(target_os = "macos")]
                let name = {
                    let shell =
                        std::env::var("SHELL").map_err(|_| ProbeError::MetricsUnavailable)?;
                    shell.rsplit('/').next().unwrap_or(&shell).to_string()
                };
                #[cfg(not(target_os = "macos"))]
                let name = general_readout()
                    .shell(ShellFormat::Relative, ShellKind::Current)?
                    .trim()
                    .to_string();
                // Gather the version here (in the parallel probe thread) so the
                // renderer never shells out on its hot path.
                let version = shell_version(&name);
                Ok(ProbeResultValue::Single(ProbeValue::Shell(name, version)))
            }),
            ProbeType::Editor => Box::new(|| {
                let editor = std::env::var("VISUAL")
                    .or_else(|_| std::env::var("EDITOR"))
                    .map_err(|_| ProbeError::MetricsUnavailable)?;
                Ok(ProbeResultValue::Single(ProbeValue::Editor(editor)))
            }),
            ProbeType::Resolution => Box::new(|| {
                let raw = general_readout().resolution()?;
                // macOS: neofetch shows only the logical/scaled resolution
                // (e.g. "1728x1117"), no native-mode or refresh-rate suffix.
                #[cfg(target_os = "macos")]
                let value = scaled_resolution(&raw);
                #[cfg(not(target_os = "macos"))]
                let value = raw;
                Ok(ProbeResultValue::Single(ProbeValue::Resolution(value)))
            }),
            ProbeType::DE => Box::new(|| {
                let name = general_readout().desktop_environment()?;
                let version = de_version(&name);
                Ok(ProbeResultValue::Single(ProbeValue::DE(name, version)))
            }),
            ProbeType::WM => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::WM(normalize_wm(
                    &general_readout().window_manager()?,
                ))))
            }),
            ProbeType::WMTheme => Box::new(|| {
                // macOS: "<accent> (<interface style>)", e.g. "Blue (Dark)", from
                // the global AppleAccentColor / AppleInterfaceStyle preferences.
                #[cfg(target_os = "macos")]
                {
                    let style = defaults_read_global("AppleInterfaceStyle")
                        .unwrap_or_else(|| "Light".to_string());
                    let accent = map_accent(defaults_read_global("AppleAccentColor").as_deref());
                    Ok(ProbeResultValue::Single(ProbeValue::WMTheme(format!(
                        "{accent} ({style})"
                    ))))
                }
                // GNOME (Fedora/Ubuntu default): query WM preferences theme via gsettings
                // Fallback: parse GTK3 settings.ini
                #[cfg(not(target_os = "macos"))]
                {
                    let theme = gsettings_get("org.gnome.desktop.wm.preferences", "theme")
                        .or_else(|_| {
                            let home = std::env::var("HOME")
                                .map_err(|_| ProbeError::MetricsUnavailable)?;
                            parse_ini_key(
                                std::path::Path::new(&format!(
                                    "{}/.config/gtk-3.0/settings.ini",
                                    home
                                )),
                                "gtk-theme-name",
                            )
                        })?;
                    Ok(ProbeResultValue::Single(ProbeValue::WMTheme(theme)))
                }
            }),
            ProbeType::Theme => Box::new(|| {
                // GTK theme via gsettings (GNOME), falling back to the gtk-3.0
                // settings.ini, with neofetch's `[GTK2/3]`/`[GTK3]` tag.
                let theme = gtk_style("gtk-theme", "gtk-theme-name")?;
                Ok(ProbeResultValue::Single(ProbeValue::Theme(theme)))
            }),
            ProbeType::Icons => Box::new(|| {
                let icons = gtk_style("icon-theme", "gtk-icon-theme-name")?;
                Ok(ProbeResultValue::Single(ProbeValue::Icons(icons)))
            }),
            ProbeType::Cursor => Box::new(|| Err(ProbeError::Unimplemented)),
            ProbeType::Terminal => Box::new(|| {
                // macOS: neofetch derives the terminal from `$TERM_PROGRAM`
                // (no version). Fall back to libmacchina, stripping its version
                // suffix, when the variable is unset.
                #[cfg(target_os = "macos")]
                {
                    let term = match std::env::var("TERM_PROGRAM") {
                        Ok(tp) if !tp.is_empty() => map_terminal(&tp),
                        _ => {
                            let t = general_readout().terminal()?.trim().to_string();
                            t.split(" (Version").next().unwrap_or(&t).trim().to_string()
                        }
                    };
                    Ok(ProbeResultValue::Single(ProbeValue::Terminal(term)))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok(ProbeResultValue::Single(ProbeValue::Terminal(
                        general_readout().terminal()?.trim().to_string(),
                    )))
                }
            }),
            ProbeType::TerminalFont => Box::new(|| {
                let font = detect_terminal_font()?;
                Ok(ProbeResultValue::Single(ProbeValue::TerminalFont(font)))
            }),
            ProbeType::CPU => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::CPU(
                    general_readout().cpu_model_name()?,
                )))
            }),
            ProbeType::GPU => gpu_probe_fn(GpuOptions::default()),
            ProbeType::Memory => Box::new(|| {
                // macOS: used = (wired + active + compressed) pages. neofetch uses
                // the same components but hardcodes a 4 KiB page size, which
                // undercounts ~4x on Apple Silicon (16 KiB pages); purr uses the
                // real page size and reports the correct figure (close to Activity
                // Monitor's "Memory Used").
                #[cfg(target_os = "macos")]
                {
                    let total_kib = sysctl("hw.memsize")?
                        .parse::<u64>()
                        .map_err(|_| ProbeError::MetricsUnavailable)?
                        / 1024;
                    let vm = run_command("vm_stat", &[])?;
                    let (wired, active, compressed) =
                        parse_vm_stat_pages(&vm).ok_or(ProbeError::MetricsUnavailable)?;
                    let page_bytes = parse_vm_stat_page_size(&vm)
                        .or_else(|| sysctl("hw.pagesize").ok()?.parse::<u64>().ok())
                        .unwrap_or(4096);
                    let used_kib = (wired + active + compressed) * page_bytes / 1024;
                    Ok(ProbeResultValue::Single(ProbeValue::Memory(
                        used_kib, total_kib,
                    )))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok(ProbeResultValue::Single(ProbeValue::Memory(
                        memory_readout().used()?,
                        memory_readout().total()?,
                    )))
                }
            }),
            ProbeType::Network => Box::new(|| Err(ProbeError::Unimplemented)),
            ProbeType::Bluetooth => Box::new(|| Err(ProbeError::Unimplemented)),
            ProbeType::BIOS => Box::new(|| Err(ProbeError::Unimplemented)),
            ProbeType::GPUDriver => Box::new(|| {
                let driver = gpu_driver().ok_or(ProbeError::MetricsUnavailable)?;
                Ok(ProbeResultValue::Single(ProbeValue::GPUDriver(driver)))
            }),
            ProbeType::CPUUsage => Box::new(|| {
                let _span = debug_span!("cpu_usage_poll").entered();
                Ok(ProbeResultValue::Single(ProbeValue::CPUUsage(
                    general_readout().cpu_usage()?,
                )))
            }),
            ProbeType::Disk => disk_probe_fn(DiskOptions::default()),
            ProbeType::Battery => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::Battery(
                    battery_readout().percentage()?,
                )))
            }),
            // TODO: Check if it's correct and matches neofetch
            ProbeType::PowerAdapter => Box::new(|| {
                Ok(ProbeResultValue::Single(ProbeValue::PowerAdapter(
                    match battery_readout().status()? {
                        BatteryState::Charging => "Charging".to_string(),
                        BatteryState::Discharging => "Discharging".to_string(),
                    },
                )))
            }),
            ProbeType::Font => Box::new(|| {
                // Mirror Theme/Icons: GNOME interface font, then GTK3 settings.ini.
                let font =
                    gsettings_get("org.gnome.desktop.interface", "font-name").or_else(|_| {
                        let home =
                            std::env::var("HOME").map_err(|_| ProbeError::MetricsUnavailable)?;
                        parse_ini_key(
                            std::path::Path::new(&format!("{}/.config/gtk-3.0/settings.ini", home)),
                            "gtk-font-name",
                        )
                    })?;
                Ok(ProbeResultValue::Single(ProbeValue::Font(font)))
            }),
            ProbeType::Song => song_probe_fn(SongOptions::default()),
            ProbeType::LocalIP => Box::new(|| {
                // macOS: neofetch reads `ipconfig getifaddr en0` (then en1).
                #[cfg(target_os = "macos")]
                {
                    let ip = run_command("ipconfig", &["getifaddr", "en0"])
                        .or_else(|_| run_command("ipconfig", &["getifaddr", "en1"]))?;
                    Ok(ProbeResultValue::Single(ProbeValue::LocalIP(ip)))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok(ProbeResultValue::Single(ProbeValue::LocalIP(
                        network_readout().logical_address(None)?,
                    )))
                }
            }),
            ProbeType::PublicIP => Box::new(|| Err(ProbeError::Unimplemented)),
            ProbeType::Users => Box::new(|| {
                let _span = debug_span!("user_scan").entered();
                Ok(ProbeResultValue::Single(ProbeValue::Users(
                    // TODO: Evaluate, whether we should make determining user platform dependent (may be unreliable currently)
                    Users::new_with_refreshed_list()
                        .iter()
                        .filter(|user| {
                            // On Unix, regular login users fall in the
                            // 1000..65535 UID range. Windows identifies users by
                            // SID, which has no such numeric range, so the
                            // heuristic doesn't apply there — include everyone.
                            #[cfg(unix)]
                            {
                                let numeric_id = *user.id().to_owned();
                                (1000..65535).contains(&numeric_id)
                            }
                            #[cfg(not(unix))]
                            {
                                let _ = user;
                                true
                            }
                        })
                        .map(|user| user.name().to_string())
                        .collect::<Vec<_>>(),
                )))
            }),
            ProbeType::Locale => Box::new(|| {
                let locale = std::env::var("LANG")
                    .or_else(|_| std::env::var("LC_ALL"))
                    .map_err(|_| ProbeError::MetricsUnavailable)?;
                Ok(ProbeResultValue::Single(ProbeValue::Locale(locale)))
            }),
            ProbeType::Java => Box::new(|| {
                // java -version writes to stderr
                let _span = debug_span!("subprocess", cmd = "java").entered();
                let output = Command::new("java")
                    .arg("-version")
                    .output()
                    .map_err(|_| ProbeError::MetricsUnavailable)?;
                let version = String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if version.is_empty() {
                    return Err(ProbeError::MetricsUnavailable);
                }
                Ok(ProbeResultValue::Single(ProbeValue::Java(version)))
            }),
            ProbeType::Python => Box::new(|| {
                let version = run_command("python3", &["--version"])
                    .or_else(|_| run_command("python", &["--version"]))?;
                Ok(ProbeResultValue::Single(ProbeValue::Python(version)))
            }),
            ProbeType::Node => Box::new(|| {
                let version = run_command("node", &["--version"])?;
                Ok(ProbeResultValue::Single(ProbeValue::Node(version)))
            }),
            ProbeType::Rust => Box::new(|| {
                let version = run_command("rustc", &["--version"])?;
                Ok(ProbeResultValue::Single(ProbeValue::Rust(version)))
            }),
        }
    }
}

/// Probe type. Refer to `ProbeValue` for what each metric corresponds to.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ProbeType {
    Host,
    OS,
    Model,
    Kernel,
    Distro,
    Uptime,
    Packages,
    Shell,
    Editor,
    Resolution,
    DE,
    WM,
    WMTheme,
    Theme,
    Icons,
    Cursor,
    Terminal,
    TerminalFont,
    CPU,
    GPU,
    Memory,
    Network,
    Bluetooth,
    BIOS,

    GPUDriver,
    CPUUsage,
    Disk,
    Battery,
    // TODO: Figure out what this should be
    PowerAdapter,
    Font,
    Song,
    LocalIP,
    PublicIP,
    Users,
    Locale,

    Java,
    Python,
    Node,
    Rust,
}

impl ProbeType {
    /// Stable machine-readable key for this metric (used by JSON output).
    pub fn id(&self) -> &'static str {
        match self {
            ProbeType::Host => "host",
            ProbeType::OS => "os",
            ProbeType::Model => "model",
            ProbeType::Kernel => "kernel",
            ProbeType::Distro => "distro",
            ProbeType::Uptime => "uptime",
            ProbeType::Packages => "packages",
            ProbeType::Shell => "shell",
            ProbeType::Editor => "editor",
            ProbeType::Resolution => "resolution",
            ProbeType::DE => "de",
            ProbeType::WM => "wm",
            ProbeType::WMTheme => "wm_theme",
            ProbeType::Theme => "theme",
            ProbeType::Icons => "icons",
            ProbeType::Cursor => "cursor",
            ProbeType::Terminal => "terminal",
            ProbeType::TerminalFont => "terminal_font",
            ProbeType::CPU => "cpu",
            ProbeType::GPU => "gpu",
            ProbeType::Memory => "memory",
            ProbeType::Network => "network",
            ProbeType::Bluetooth => "bluetooth",
            ProbeType::BIOS => "bios",
            ProbeType::GPUDriver => "gpu_driver",
            ProbeType::CPUUsage => "cpu_usage",
            ProbeType::Disk => "disk",
            ProbeType::Battery => "battery",
            ProbeType::PowerAdapter => "power_adapter",
            ProbeType::Font => "font",
            ProbeType::Song => "song",
            ProbeType::LocalIP => "local_ip",
            ProbeType::PublicIP => "public_ip",
            ProbeType::Users => "users",
            ProbeType::Locale => "locale",
            ProbeType::Java => "java",
            ProbeType::Python => "python",
            ProbeType::Node => "node",
            ProbeType::Rust => "rust",
        }
    }
}

pub enum ProbeResultValue {
    Single(ProbeValue),
    Multiple(Vec<ProbeValue>),
}

impl From<ProbeValue> for ProbeResultValue {
    fn from(value: ProbeValue) -> Self {
        ProbeResultValue::Single(value)
    }
}

impl From<Vec<ProbeValue>> for ProbeResultValue {
    fn from(values: Vec<ProbeValue>) -> Self {
        ProbeResultValue::Multiple(values)
    }
}

pub type ProbeResult = Result<ProbeResultValue, ProbeError>;
pub type ProbeResultFunction = Box<dyn Fn() -> ProbeResult + Send + Sync>;
pub type ProbeList = Vec<(String, ProbeResultFunction)>;

impl ProbeValue {
    /// Render this probe value to its default display string (option-free).
    ///
    /// This is the baseline rendering shared by every renderer. Option-aware
    /// formatting (memory units, uptime shorthand, …) layers on top of this in
    /// the config layer; this remains the fallback and the machine-readable form.
    pub fn format(&self) -> String {
        match self {
            ProbeValue::Host(username, hostname) => format!("{}@{}", username, hostname),
            ProbeValue::OS(os) => DistroOptions::default().format(os),
            ProbeValue::Distro(distro) => distro.to_string(),
            ProbeValue::Model(vendor, product) => {
                // macOS supplies no vendor (just the model identifier).
                if vendor.is_empty() {
                    product.to_string()
                } else {
                    format!("{} {}", vendor, product)
                }
            }
            ProbeValue::Kernel(kernel) => KernelOptions::default().format(kernel),
            ProbeValue::Uptime(uptime) => UptimeOptions::default().format(*uptime),
            ProbeValue::Packages(counts) => PackagesOptions::default().format(counts),
            ProbeValue::Shell(name, version) => {
                ShellOptions::default().format(name, version.as_deref())
            }
            ProbeValue::Editor(editor) => editor.to_string(),
            ProbeValue::Resolution(resolution) => resolution.to_string(),
            ProbeValue::DE(name, version) => DeOptions::default().format(name, version.as_deref()),
            ProbeValue::WM(wm) => wm.to_string(),
            ProbeValue::WMTheme(wm_theme) => wm_theme.to_string(),
            ProbeValue::Theme(theme) => theme.to_string(),
            ProbeValue::Icons(icons) => icons.to_string(),
            ProbeValue::Cursor(cursor) => cursor.to_string(),
            ProbeValue::Terminal(terminal) => terminal.to_string(),
            ProbeValue::TerminalFont(terminal_font) => terminal_font.to_string(),
            ProbeValue::CPU(cpu) => format_cpu(cpu, &CpuOptions::default()),
            ProbeValue::GPU(gpu) => GpuOptions::default().format(gpu),
            ProbeValue::Memory(used, total) => MemoryOptions::default().format(*used, *total),
            ProbeValue::Network(network) => network.to_string(),
            ProbeValue::Bluetooth(bluetooth) => bluetooth.to_string(),
            ProbeValue::BIOS(bios) => bios.to_string(),
            ProbeValue::GPUDriver(gpu_driver) => gpu_driver.to_string(),
            ProbeValue::CPUUsage(cpu_usage) => format!("{}%", cpu_usage),
            ProbeValue::Disk(mount, name, used, total) => {
                DiskOptions::default().format(mount, name, *used, *total)
            }
            ProbeValue::Battery(battery) => battery.to_string(),
            ProbeValue::PowerAdapter(power_adapter) => power_adapter.to_string(),
            ProbeValue::Font(font) => font.to_string(),
            ProbeValue::Song(song) => song.to_string(),
            ProbeValue::LocalIP(local_ip) => local_ip.to_string(),
            ProbeValue::PublicIP(public_ip) => public_ip.to_string(),
            ProbeValue::Users(users) => users.join(", "),
            ProbeValue::Locale(locale) => locale.to_string(),
            ProbeValue::Java(java) => java.to_string(),
            ProbeValue::Node(node) => node.to_string(),
            ProbeValue::Python(python) => python.to_string(),
            ProbeValue::Rust(rust) => rust.to_string(),
        }
    }
}

/// Format a raw CPU model into neofetch's CPU line, honoring [`CpuOptions`]:
/// cruft stripped, optional brand, core count, and max clock, e.g.
/// "AMD Ryzen 7 7800X3D (16) @ 5.053GHz".
/// Read the host model from DMI sysfs in neofetch's order: motherboard
/// (`board_vendor` + `board_name`) first, then the chassis product
/// (`product_name` + `product_version`), then device-tree / `/tmp/sysinfo`.
/// Returns the raw (un-cleaned) string, or `None` when no source exists.
#[cfg(target_os = "linux")]
fn read_dmi_model() -> Option<String> {
    fn read(path: &str) -> Option<String> {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
    }
    // neofetch keys off file existence of the pair, then joins the two contents
    // with a space (either may be empty; `clean_model` trims the result).
    let pair = |a: &str, b: &str| -> Option<String> {
        let base = "/sys/devices/virtual/dmi/id";
        let va = read(&format!("{base}/{a}"));
        let vb = read(&format!("{base}/{b}"));
        (va.is_some() || vb.is_some())
            .then(|| format!("{} {}", va.unwrap_or_default(), vb.unwrap_or_default()))
    };
    pair("board_vendor", "board_name")
        .or_else(|| pair("product_name", "product_version"))
        .or_else(|| read("/sys/firmware/devicetree/base/model"))
        .or_else(|| read("/tmp/sysinfo/model"))
}

/// libmacchina fallback for the host model (`vendor` + `product`), used when DMI
/// sysfs is unavailable (e.g. on Windows).
#[cfg(not(target_os = "macos"))]
fn libmacchina_model() -> Option<String> {
    use libmacchina::traits::ProductReadout as _;
    let pr = product_readout();
    match (pr.vendor().ok(), pr.product().ok()) {
        (Some(v), Some(p)) => Some(format!("{v} {p}")),
        (Some(s), None) | (None, Some(s)) => Some(s),
        (None, None) => None,
    }
}

/// Strip the dummy OEM placeholder strings neofetch removes from model strings
/// (`To be filled by O.E.M.`, `System Product Name`, `Default string`, the `�`
/// replacement char, …) and collapse whitespace. Wraps known VM identifiers
/// (`Standard PC…` → `KVM/QEMU (…)`, `OpenBSD…` → `vmm (…)`). An empty result
/// means the string was entirely placeholder text.
fn clean_model(model: &str) -> String {
    let mut s = model.to_string();
    s = s.replace("To be filled by O.E.M.", "");
    // Greedy `*` tails: truncate at the marker.
    for marker in ["To Be Filled", "OEM"] {
        if let Some(i) = s.find(marker) {
            s.truncate(i);
        }
    }
    for pat in [
        "Not Applicable",
        "System Product Name",
        "System Version",
        "Undefined",
        "Default string",
        "Not Specified",
        "Type1ProductConfigId",
        "INVALID",
        "All Series",
        "\u{fffd}",
    ] {
        s = s.replace(pat, "");
    }
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.starts_with("Standard PC") {
        return format!("KVM/QEMU ({s})");
    }
    if s.starts_with("OpenBSD") {
        return format!("vmm ({s})");
    }
    s
}

pub fn format_cpu(model: &str, opts: &CpuOptions) -> String {
    use libmacchina::traits::GeneralReadout as _;

    let mut cpu = clean_cpu_model(model);

    // `cpu_brand = off`: drop the leading brand token (e.g. "AMD", "Intel"),
    // matching neofetch.
    if !opts.brand
        && let Some((_brand, rest)) = cpu.split_once(' ')
    {
        cpu = rest.to_string();
    }

    let cores = match opts.cores {
        CoresMode::Logical => general_readout().cpu_cores().ok(),
        CoresMode::Physical => general_readout().cpu_physical_cores().ok(),
        CoresMode::Off => None,
    };
    if let Some(n) = cores {
        if cfg!(target_os = "macos") {
            // neofetch inserts the core count next to the clock speed
            // (`${cpu/@/(${cores}) @}`). Apple Silicon brand strings have no
            // "@ speed", so no core count is shown there.
            if cpu.contains('@') {
                cpu = cpu.replacen('@', &format!("({}) @", n), 1);
            }
        } else {
            cpu.push_str(&format!(" ({})", n));
        }
    }

    if opts.speed
        && let Some(ghz) = cpu_max_ghz(opts.speed_type, opts.speed_shorthand)
    {
        cpu.push_str(&format!(" @ {}GHz", ghz));
    }

    cpu
}

/// Strip the marketing cruft neofetch removes from CPU model strings, mirroring
/// the exact removal sequence in neofetch's `get_cpu` (lines `cpu="${cpu//…}"`).
///
/// So "Intel(R) Core(TM) i7-11800H CPU" -> "Intel i7-11800H" (the "Core" word is
/// dropped), "AMD Ryzen 7 7800X3D 8-Core Processor" -> "AMD Ryzen 7 7800X3D",
/// and "… with Radeon Vega Graphics" is removed while a bare "with Radeon
/// Graphics" is kept. Whitespace is collapsed at the end (neofetch's `trim()`).
fn clean_cpu_model(model: &str) -> String {
    let mut s = model.to_string();

    // Literal trademark / core-word / filler removals.
    for pat in [
        "(TM)",
        "(tm)",
        "(R)",
        "(r)",
        "CPU",
        "Processor",
        "Dual-Core",
        "Quad-Core",
        "Six-Core",
        "Eight-Core",
    ] {
        s = s.replace(pat, "");
    }

    // "16-Core" / "8-Core" (neofetch's `[1-9][0-9]-Core` / `[0-9]-Core`).
    remove_n_core(&mut s);
    // ", <n> Compute Cores" (AMD APUs).
    remove_span(&mut s, ", ", " Compute Cores");
    // "Core " -> " " drops the marketing "Core" word (e.g. "Intel Core i7").
    s = s.replace("Core ", " ");
    // `("AuthenticAMD" …)` parenthetical on old AMD strings.
    remove_span(&mut s, "(\"AuthenticAMD\"", ")");
    // "with Radeon <model> Graphics" — only when a model word is present, so a
    // bare "with Radeon Graphics" (single space) is preserved, like neofetch.
    remove_span(&mut s, "with Radeon ", " Graphics");
    s = s.replace(", altivec supported", "");
    // "FPU…" / "Chip Revision…" extend to end of string (`FPU*` / `Chip Revision*`).
    if let Some(i) = s.find("FPU") {
        s.truncate(i);
    }
    if let Some(i) = s.find("Chip Revision") {
        s.truncate(i);
    }
    s = s.replace("Technologies, Inc", "");
    s = s.replace("Core2", "Core 2");

    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove "<digits>-Core" runs (e.g. "8-Core", "16-Core"), matching neofetch's
/// digit-prefixed `-Core` globs. Scans the whole string so multiple runs go.
fn remove_n_core(s: &mut String) {
    let needle = "-Core";
    let mut from = 0;
    while let Some(rel) = s[from..].find(needle) {
        let dash = from + rel;
        let mut start = dash;
        while start > 0 && s.as_bytes()[start - 1].is_ascii_digit() {
            start -= 1;
        }
        if start < dash {
            s.replace_range(start..dash + needle.len(), "");
            from = start;
        } else {
            from = dash + needle.len();
        }
    }
}

/// Remove the span from the first `start` marker through the last following
/// `end` marker (a greedy `start*end` glob, like neofetch). No-op if either
/// marker is missing or `end` doesn't occur after `start`.
fn remove_span(s: &mut String, start: &str, end: &str) {
    let Some(a) = s.find(start) else { return };
    let after = a + start.len();
    let Some(rel) = s[after..].rfind(end) else {
        return;
    };
    let b = after + rel + end.len();
    s.replace_range(a..b, "");
}

/// Read the CPU's frequency from sysfs and render it as GHz, matching neofetch.
///
/// `speed_type` selects which cpufreq file to read first; the others are tried
/// as fallbacks (some are absent on VMs / certain governors). `shorthand`
/// rounds to a single decimal (e.g. "5.1") instead of the full "5.053".
/// Returns `None` when sysfs is unavailable.
fn cpu_max_ghz(speed_type: SpeedType, shorthand: bool) -> Option<String> {
    let base = "/sys/devices/system/cpu/cpu0/cpufreq";
    let primary = match speed_type {
        SpeedType::BiosLimit => "bios_limit",
        SpeedType::ScalingCurFreq => "scaling_cur_freq",
        SpeedType::ScalingMinFreq => "scaling_min_freq",
        SpeedType::ScalingMaxFreq => "scaling_max_freq",
    };
    for file in [
        primary,
        "bios_limit",
        "scaling_max_freq",
        "cpuinfo_max_freq",
    ] {
        if let Ok(contents) = std::fs::read_to_string(format!("{base}/{file}"))
            && let Ok(khz) = contents.trim().parse::<u64>()
        {
            let mhz = khz / 1000;
            return Some(if shorthand {
                let tenths = (mhz + 50) / 100; // round to nearest 0.1 GHz
                format!("{}.{}", tenths / 10, tenths % 10)
            } else {
                format!("{}.{:03}", mhz / 1000, mhz % 1000)
            });
        }
    }
    None
}

/// Best-effort shell version (e.g. "5.9") by running `<shell> --version` and
/// taking the first version-looking token.
fn shell_version(name: &str) -> Option<String> {
    let bin = name.rsplit('/').next().unwrap_or(name);
    let out = run_command(bin, &["--version"]).ok()?;
    let line = out.lines().next()?;
    line.split_whitespace()
        .find(|t| t.contains('.') && t.chars().any(|c| c.is_ascii_digit()))
        .map(|t| t.split(['(', '-']).next().unwrap_or(t).to_string())
}

/// Best-effort desktop-environment version, mirroring neofetch's per-DE
/// `--version` commands and taking the trailing version token.
fn de_version(de: &str) -> Option<String> {
    let (bin, arg) = if de.starts_with("Plasma") || de.contains("KDE") {
        ("plasmashell", "--version")
    } else if de.starts_with("MATE") {
        ("mate-session", "--version")
    } else if de.starts_with("Xfce") {
        ("xfce4-session", "--version")
    } else if de.starts_with("GNOME") {
        ("gnome-shell", "--version")
    } else if de.starts_with("Cinnamon") {
        ("cinnamon", "--version")
    } else if de.starts_with("Budgie") {
        ("budgie-desktop", "--version")
    } else if de.starts_with("LXQt") {
        ("lxqt-session", "--version")
    } else {
        return None;
    };
    let out = run_command(bin, &[arg]).ok()?;
    let line = out.lines().next()?;
    // Strip a trailing "(...)" then take the last whitespace token, like neofetch.
    let cleaned = line.split('(').next().unwrap_or(line).trim();
    cleaned
        .split_whitespace()
        .next_back()
        .map(|s| s.trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
}

/// Build the disk probe, filtering mount points by `opts.show` (empty = all).
pub fn disk_probe_fn(opts: DiskOptions) -> ProbeResultFunction {
    Box::new(move || {
        let _span = debug_span!("disk_scan").entered();
        let disks = Disks::new_with_refreshed_list();
        let entries: Vec<ProbeValue> = disks
            .iter()
            .filter(|disk| {
                opts.show.is_empty()
                    || opts
                        .show
                        .iter()
                        .any(|s| disk.mount_point() == std::path::Path::new(s))
            })
            .map(|disk| {
                ProbeValue::Disk(
                    disk.mount_point().to_path_buf(),
                    disk.name().to_string_lossy().to_string(),
                    disk.total_space().saturating_sub(disk.available_space()),
                    disk.total_space(),
                )
            })
            .collect();
        Ok(ProbeResultValue::Multiple(entries))
    })
}

/// Build the GPU probe, filtering by `opts.gpu_type` (all/dedicated/integrated).
pub fn gpu_probe_fn(opts: GpuOptions) -> ProbeResultFunction {
    Box::new(move || {
        let _span = debug_span!("gpu_readout").entered();
        let entries: Vec<ProbeValue> = gpus_list()?
            .into_iter()
            .filter(|name| match opts.gpu_type {
                GpuType::All => true,
                GpuType::Integrated => is_integrated_gpu(name),
                GpuType::Dedicated => !is_integrated_gpu(name),
            })
            .map(ProbeValue::GPU)
            .collect();
        Ok(ProbeResultValue::Multiple(entries))
    })
}

/// Heuristic for whether a GPU name denotes an integrated GPU. Best-effort:
/// matches "Graphics" (Intel/AMD iGPU naming) and common AMD APU codenames.
fn is_integrated_gpu(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("graphics")
        || [
            "raphael", "renoir", "cezanne", "phoenix", "lucienne", "barcelo", "picasso",
        ]
        .iter()
        .any(|c| n.contains(c))
}

/// Kernel driver(s) bound to the display controller(s), read from sysfs
/// (the `driver` symlink of each PCI device with class `0x03xx`). Returns e.g.
/// "amdgpu" or "i915, nvidia". Linux-only.
#[cfg(target_os = "linux")]
fn gpu_driver() -> Option<String> {
    use std::fs;
    let mut drivers: Vec<String> = Vec::new();
    for entry in fs::read_dir("/sys/bus/pci/devices").ok()?.flatten() {
        let path = entry.path();
        let Ok(class) = fs::read_to_string(path.join("class")) else {
            continue;
        };
        // 0x03xxxx = display controller (VGA / 3D / other display).
        if !class.trim_start().starts_with("0x03") {
            continue;
        }
        if let Ok(link) = fs::read_link(path.join("driver"))
            && let Some(name) = link.file_name()
        {
            let d = name.to_string_lossy().to_string();
            if !drivers.contains(&d) {
                drivers.push(d);
            }
        }
    }
    (!drivers.is_empty()).then(|| drivers.join(", "))
}

/// macOS has a single, unnamed system graphics driver; neofetch reports a
/// constant string here.
#[cfg(target_os = "macos")]
fn gpu_driver() -> Option<String> {
    Some("macOS Default Graphics Driver".to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn gpu_driver() -> Option<String> {
    None
}

/// Now-playing track lookup over MPRIS (D-Bus), pure-Rust via zbus.
#[cfg(target_os = "linux")]
mod song {
    use std::collections::HashMap;

    use zbus::blocking::Connection;
    use zbus::zvariant::OwnedValue;

    #[zbus::proxy(
        interface = "org.mpris.MediaPlayer2.Player",
        default_path = "/org/mpris/MediaPlayer2"
    )]
    trait Player {
        #[zbus(property)]
        fn metadata(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
        #[zbus(property)]
        fn playback_status(&self) -> zbus::Result<String>;
    }

    pub struct SongInfo {
        pub artist: String,
        pub album: String,
        pub title: String,
    }

    /// Query MPRIS for the current track. `player` is "auto" (prefer a playing
    /// one, else the first with metadata) or a bus-name suffix (e.g. "spotify").
    pub fn current(player: &str) -> Option<SongInfo> {
        let conn = Connection::session().ok()?;
        let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).ok()?;
        let names = dbus.list_names().ok()?;

        let mut fallback: Option<SongInfo> = None;
        for name in names.into_iter().map(|n| n.as_str().to_string()) {
            if !name.starts_with("org.mpris.MediaPlayer2.") {
                continue;
            }
            if player != "auto" {
                let suffix = name.trim_start_matches("org.mpris.MediaPlayer2.");
                if !suffix.eq_ignore_ascii_case(player) {
                    continue;
                }
            }
            let Ok(proxy) = PlayerProxyBlocking::builder(&conn)
                .destination(name)
                .and_then(|b| b.build())
            else {
                continue;
            };
            let Some(info) = proxy.metadata().ok().and_then(|m| extract(&m)) else {
                continue;
            };
            if proxy
                .playback_status()
                .map(|s| s == "Playing")
                .unwrap_or(false)
            {
                return Some(info);
            }
            if fallback.is_none() {
                fallback = Some(info);
            }
        }
        fallback
    }

    fn extract(meta: &HashMap<String, OwnedValue>) -> Option<SongInfo> {
        let string_of = |key: &str| {
            meta.get(key)
                .and_then(|v| String::try_from(v.clone()).ok())
                .unwrap_or_default()
        };
        let artist = meta
            .get("xesam:artist")
            .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
            .map(|a| a.join(", "))
            .unwrap_or_default();
        let info = SongInfo {
            artist,
            album: string_of("xesam:album"),
            title: string_of("xesam:title"),
        };
        if info.artist.is_empty() && info.album.is_empty() && info.title.is_empty() {
            None
        } else {
            Some(info)
        }
    }
}

/// Build the now-playing probe. Selects the player and renders the configured
/// `song_format`; `song_shorthand` drops the album. MPRIS is Linux-only.
#[cfg(target_os = "linux")]
pub fn song_probe_fn(opts: SongOptions) -> ProbeResultFunction {
    Box::new(move || {
        let info = song::current(&opts.player).ok_or(ProbeError::MetricsUnavailable)?;
        let fmt = if opts.shorthand {
            "%artist% - %title%"
        } else {
            opts.format.as_str()
        };
        let text = fmt
            .replace("%artist%", &info.artist)
            .replace("%album%", &info.album)
            .replace("%title%", &info.title);
        // Collapse empty " - " segments (e.g. missing album) for the default format.
        let text = text
            .split(" - ")
            .filter(|p| !p.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" - ");
        if text.trim().is_empty() {
            return Err(ProbeError::MetricsUnavailable);
        }
        Ok(ProbeResultValue::Single(ProbeValue::Song(text)))
    })
}

#[cfg(not(target_os = "linux"))]
pub fn song_probe_fn(_opts: SongOptions) -> ProbeResultFunction {
    Box::new(|| Err(ProbeError::Unimplemented))
}

#[cfg(test)]
mod tests {
    use super::{clean_cpu_model, clean_model, format_cpu, normalize_wm, with_gtk_tag};
    use crate::config::{CoresMode, CpuOptions};

    #[test]
    fn wm_renames_match_neofetch() {
        // libmacchina's lowercase "gnome-shell" -> "Mutter".
        assert_eq!(normalize_wm("gnome-shell"), "Mutter");
        assert_eq!(normalize_wm("GNOME Shell"), "Mutter");
        assert_eq!(normalize_wm("WindowMaker"), "wmaker");
        // Other WMs pass through untouched.
        assert_eq!(normalize_wm("sway"), "sway");
        assert_eq!(normalize_wm("KWin"), "KWin");
    }

    #[test]
    fn gtk_theme_tags() {
        // gsettings source -> [GTK2/3] (GNOME), ini fallback -> [GTK3].
        assert_eq!(with_gtk_tag("Adwaita", true), "Adwaita [GTK2/3]");
        assert_eq!(with_gtk_tag("Arc-Dark", false), "Arc-Dark [GTK3]");
    }

    #[test]
    fn model_dmi_and_oem_cleanup() {
        // Real motherboard string passes through (whitespace collapsed).
        assert_eq!(
            clean_model("Micro-Star International Co., Ltd. MEG X870E GODLIKE (MS-7E48)"),
            "Micro-Star International Co., Ltd. MEG X870E GODLIKE (MS-7E48)"
        );
        // Dummy OEM placeholders are removed.
        assert_eq!(
            clean_model("System manufacturer System Product Name"),
            "System manufacturer"
        );
        assert_eq!(clean_model("To be filled by O.E.M."), "");
        assert_eq!(clean_model("Default string Default string"), "");
        // A bare vendor with an empty product (trailing space from the join).
        assert_eq!(clean_model("Dell Inc. "), "Dell Inc.");
        // QEMU / KVM identifier wrapping.
        assert_eq!(
            clean_model("Standard PC (Q35 + ICH9, 2009)"),
            "KVM/QEMU (Standard PC (Q35 + ICH9, 2009))"
        );
    }

    #[test]
    fn cpu_brand_off_drops_leading_token() {
        let opts = CpuOptions {
            brand: false,
            cores: CoresMode::Off,
            speed: false,
            ..Default::default()
        };
        assert_eq!(format_cpu("AMD Ryzen 7 7800X3D", &opts), "Ryzen 7 7800X3D");
    }

    #[test]
    fn cpu_default_brand_no_cores_no_speed_keeps_model() {
        let opts = CpuOptions {
            cores: CoresMode::Off,
            speed: false,
            ..Default::default()
        };
        assert_eq!(
            format_cpu("AMD Ryzen 7 7800X3D", &opts),
            "AMD Ryzen 7 7800X3D"
        );
    }

    #[test]
    fn strips_amd_core_and_processor() {
        assert_eq!(
            clean_cpu_model("AMD Ryzen 7 7800X3D 8-Core Processor"),
            "AMD Ryzen 7 7800X3D"
        );
    }

    #[test]
    fn strips_intel_trademarks_and_cpu() {
        // neofetch drops the marketing "Core" word too ("Core " -> " ").
        assert_eq!(
            clean_cpu_model("Intel(R) Core(TM) i7-11800H CPU"),
            "Intel i7-11800H"
        );
        assert_eq!(
            clean_cpu_model("Intel(R) Xeon(R) CPU E5-2680 v4"),
            "Intel Xeon E5-2680 v4"
        );
        // "Core2" becomes "Core 2" (and is not stripped by the "Core " rule).
        assert_eq!(
            clean_cpu_model("Intel(R) Core(TM)2 Duo CPU E8400"),
            "Intel Core 2 Duo E8400"
        );
    }

    #[test]
    fn strips_amd_apu_radeon_and_compute_cores() {
        // A model word between Radeon/Graphics -> stripped.
        assert_eq!(
            clean_cpu_model("AMD Ryzen 5 3400G with Radeon Vega Graphics"),
            "AMD Ryzen 5 3400G"
        );
        // Bare "with Radeon Graphics" (single space) is preserved.
        assert_eq!(
            clean_cpu_model("AMD Ryzen 7 5825U with Radeon Graphics"),
            "AMD Ryzen 7 5825U with Radeon Graphics"
        );
        // Two-digit core counts and ", N Compute Cores".
        assert_eq!(
            clean_cpu_model("AMD Ryzen 9 5900X 12-Core Processor"),
            "AMD Ryzen 9 5900X"
        );
        assert_eq!(
            clean_cpu_model("AMD A10-7850K APU with Radeon(TM) R7 Graphics, 4 Compute Cores 2C+2G"),
            "AMD A10-7850K APU 2C+2G"
        );
    }

    #[test]
    fn leaves_clean_names_untouched() {
        assert_eq!(clean_cpu_model("Apple M1"), "Apple M1");
        assert_eq!(
            clean_cpu_model("AMD Ryzen 7 7800X3D"),
            "AMD Ryzen 7 7800X3D"
        );
    }

    use super::{
        map_accent, map_terminal, parse_chipset_models, parse_port_count, parse_vm_stat_page_size,
        parse_vm_stat_pages, scaled_resolution,
    };

    #[test]
    fn resolution_reduces_to_scaled() {
        assert_eq!(
            scaled_resolution("3456x2234@120fps (as 1728x1117)"),
            "1728x1117"
        );
    }

    #[test]
    fn resolution_plain_passthrough_and_at_strip() {
        assert_eq!(scaled_resolution("1920x1080"), "1920x1080");
        assert_eq!(scaled_resolution("1920x1080@60fps"), "1920x1080");
    }

    #[test]
    fn resolution_joins_multiple_displays() {
        assert_eq!(
            scaled_resolution("3456x2234@120fps (as 1728x1117)\n2560x1440@60fps"),
            "1728x1117, 2560x1440"
        );
    }

    #[test]
    fn accent_maps_to_neofetch_names() {
        assert_eq!(map_accent(None), "Blue"); // AppleAccentColor unset
        assert_eq!(map_accent(Some("-1")), "Graphite");
        assert_eq!(map_accent(Some("3")), "Green");
        assert_eq!(map_accent(Some("4")), "Blue"); // 4 (blue) falls through
    }

    #[test]
    fn terminal_maps_known_and_strips_app() {
        assert_eq!(map_terminal("iTerm.app"), "iTerm2");
        assert_eq!(map_terminal("Apple_Terminal"), "Apple Terminal");
        assert_eq!(map_terminal("Hyper"), "HyperTerm");
        assert_eq!(map_terminal("ghostty"), "ghostty");
        assert_eq!(map_terminal("WezTerm.app"), "WezTerm"); // case preserved
    }

    #[test]
    fn parses_chipset_models() {
        let sp = "      Bus: Built-In\n          Resolution: 3456 x 2234 Retina\n          Chipset Model: Apple M3 Pro\n";
        assert_eq!(parse_chipset_models(sp), vec!["Apple M3 Pro".to_string()]);
    }

    #[test]
    fn parses_vm_stat_pages_and_size() {
        let vm = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\n\
                  Pages free:                               12345.\n\
                  Pages active:                            658691.\n\
                  Pages wired down:                        203437.\n\
                  Pages occupied by compressor:            762997.\n";
        assert_eq!(parse_vm_stat_pages(vm), Some((203437, 658691, 762997)));
        assert_eq!(parse_vm_stat_page_size(vm), Some(16384));
    }

    #[test]
    fn port_count_includes_every_line() {
        // neofetch's per-manager count is the array length (header included).
        let out = "The following ports are currently installed:\n  pkg-a @1\n  pkg-b @2\n";
        assert_eq!(parse_port_count(out), 3);
    }
}
