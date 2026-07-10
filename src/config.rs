use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::probe::{ProbeResultFunction, ProbeType, ProbeValue};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Config {
    Neofetch(NeofetchRendererConfig),
    Json(JsonRendererConfig),
}

pub enum RendererOverride {
    Neofetch,
    Json,
}

impl Config {
    /// Default config with all features enabled
    pub fn default_all() -> Self {
        Self::default_neofetch_all()
    }

    /// Default config replicating neofetch
    pub fn default_neofetch() -> Self {
        Self::Neofetch(NeofetchRendererConfig::default())
    }

    /// Default config replicating neofetch with all features enabled
    pub fn default_neofetch_all() -> Self {
        Self::Neofetch(NeofetchRendererConfig::default_all())
    }

    /// Default config emitting JSON.
    pub fn default_json() -> Self {
        Self::Json(JsonRendererConfig::default())
    }

    /// Load config from a file
    pub fn from_file(
        path: &Path,
        _renderer_override: Option<RendererOverride>,
    ) -> Result<Self, ConfigParseError> {
        // TODO: Support "extending" default configs
        // TODO: Implement renderer override
        toml::from_str(&std::fs::read_to_string(path)?).map_err(|e| e.into())
    }

    /// Write config to a file
    pub fn to_file(&self, path: &Path) -> Result<(), ConfigWriteError> {
        // Serialize to toml
        let toml = toml::to_string(self)?;
        // Write to file
        Ok(std::fs::write(path, toml)?)
    }

    /// Generate a default config file
    /// If the file already exists, it will not be overwritten
    pub fn generate_default(path: &Path) -> Result<(), ConfigWriteError> {
        if path.exists() {
            return Err(ConfigWriteError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "File already exists",
            )));
        }
        let config = Self::default();
        config.to_file(path)
        // TODO: Replace line above with custom serialization to include comments
    }

    fn get_project_dirs() -> Option<directories::ProjectDirs> {
        directories::ProjectDirs::from("net", "justin13888", "purr")
    }

    pub fn get_config_dir() -> Option<PathBuf> {
        Self::get_project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Mutable access to the active renderer's probe list (for CLI overrides).
    pub fn probes_mut(&mut self) -> &mut Vec<ProbeConfig> {
        match self {
            Config::Neofetch(c) => &mut c.probes,
            Config::Json(c) => &mut c.probes,
        }
    }

    /// Swap to a different renderer, preserving the probe list. A Neofetch
    /// config kept as Neofetch is returned untouched (no styling loss).
    pub fn with_renderer(self, target: RendererOverride) -> Self {
        match (target, &self) {
            (RendererOverride::Neofetch, Config::Neofetch(_)) => self,
            (RendererOverride::Neofetch, Config::Json(c)) => {
                Config::Neofetch(NeofetchRendererConfig {
                    probes: c.probes.clone(),
                    ..Default::default()
                })
            }
            (RendererOverride::Json, _) => Config::Json(JsonRendererConfig {
                probes: match self {
                    Config::Neofetch(c) => c.probes,
                    Config::Json(c) => c.probes,
                },
            }),
        }
    }

    pub const CONFIG_FILE_NAME: &'static str = "config.toml";
}

impl Default for Config {
    fn default() -> Self {
        Self::default_neofetch()
    }
}

#[derive(Error, Debug)]
pub enum ConfigParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] toml::de::Error),
}

#[derive(Error, Debug)]
pub enum ConfigWriteError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] toml::ser::Error),
}

fn default_separator() -> String {
    ":".to_string()
}
fn default_bold() -> bool {
    true
}
fn default_underline_char() -> String {
    "-".to_string()
}
fn default_block_range() -> [u8; 2] {
    [0, 15]
}
fn default_block_width() -> u16 {
    3
}
fn default_block_height() -> u16 {
    1
}

/// Color-block grid options (neofetch `block_range`/`block_width`/
/// `block_height`/`col_offset`).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorBlocks {
    /// Inclusive `[start, end]` palette range to display.
    #[serde(default = "default_block_range")]
    pub range: [u8; 2],
    /// Width of each block, in spaces.
    #[serde(default = "default_block_width")]
    pub width: u16,
    /// Number of rows each block group spans.
    #[serde(default = "default_block_height")]
    pub height: u16,
    /// Left offset before the blocks; `None` = auto (align with the info column).
    #[serde(default)]
    pub offset: Option<u16>,
}

impl Default for ColorBlocks {
    fn default() -> Self {
        Self {
            range: default_block_range(),
            width: default_block_width(),
            height: default_block_height(),
            offset: None,
        }
    }
}

fn default_ascii_bold() -> bool {
    true
}

/// ASCII logo options (neofetch `ascii_distro`/`ascii_colors`/`ascii_bold`).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AsciiOptions {
    /// Force a specific distro logo; `None` = auto-detect from the running OS.
    #[serde(default)]
    pub distro: Option<String>,
    /// Override the logo's `${c1}`..`${c6}` palette; empty = the logo's own colours.
    #[serde(default)]
    pub colors: Vec<u8>,
    /// Bold the logo (neofetch defaults this on).
    #[serde(default = "default_ascii_bold")]
    pub bold: bool,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self {
            distro: None,
            colors: Vec::new(),
            bold: default_ascii_bold(),
        }
    }
}

/// Logo backend: ASCII art or a Kitty-graphics-protocol image (neofetch `backend`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Ascii,
    Kitty,
    /// No logo (neofetch `--off`).
    Off,
}

fn default_image_cols() -> u16 {
    40
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NeofetchRendererConfig {
    /// Whether to display the title
    /// (e.g. "johndoe@myhostname\n------------------")
    pub title: bool,
    pub underline: bool,
    pub col: bool,

    /// Separator between a label and its value (neofetch `separator`).
    #[serde(default = "default_separator")]
    pub separator: String,
    /// Bold the title and labels (neofetch `bold`).
    #[serde(default = "default_bold")]
    pub bold: bool,
    /// Character used for the title underline (neofetch `underline_char`).
    #[serde(default = "default_underline_char")]
    pub underline_char: String,
    /// Show the fully-qualified hostname in the title (neofetch `title_fqdn`).
    #[serde(default)]
    pub title_fqdn: bool,
    /// 256-colour text slots `[title, @, underline, subtitle, colon, info]`
    /// (neofetch `colors`). Empty means use the distro's logo colour.
    #[serde(default)]
    pub colors: Vec<u8>,
    /// Color-block grid layout (neofetch `block_*` / `col_offset`).
    #[serde(default)]
    pub color_blocks: ColorBlocks,
    /// ASCII logo options (neofetch `ascii_*`).
    #[serde(default)]
    pub ascii: AsciiOptions,
    /// Logo backend (neofetch `backend`): ASCII art or a Kitty image.
    #[serde(default)]
    pub backend: Backend,
    /// PNG image source for the Kitty backend (neofetch `--source`).
    #[serde(default)]
    pub image_source: Option<PathBuf>,
    /// Image width in terminal cells for the Kitty backend.
    #[serde(default = "default_image_cols")]
    pub image_cols: u16,

    pub probes: Vec<ProbeConfig>,
}

impl NeofetchRendererConfig {
    pub fn default_all() -> Self {
        Self {
            probes: ProbeConfig::default_all(),
            ..Self::default()
        }
    }
}

impl Default for NeofetchRendererConfig {
    fn default() -> Self {
        Self {
            title: true,
            underline: true,
            col: true,
            separator: default_separator(),
            bold: default_bold(),
            underline_char: default_underline_char(),
            title_fqdn: false,
            colors: Vec::new(),
            color_blocks: ColorBlocks::default(),
            ascii: AsciiOptions::default(),
            backend: Backend::Ascii,
            image_source: None,
            image_cols: default_image_cols(),
            probes: ProbeConfig::default_neofetch(),
        }
    }
}

/// Configuration for the JSON output renderer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JsonRendererConfig {
    pub probes: Vec<ProbeConfig>,
}

impl Default for JsonRendererConfig {
    fn default() -> Self {
        Self {
            probes: ProbeConfig::default_all(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Per-probe options
//
// Each probe carries a small options struct: a `label` plus the neofetch-style
// tunables that apply to it. The `probe_options!` macro generates the struct,
// its neofetch defaults, and a `Deserialize` impl that accepts either a bare
// label string (`OS = "OS"`) or a full options table (`{ label = "OS", … }`),
// so the terse form stays available while rich options are opt-in.
// ─────────────────────────────────────────────────────────────────────────

macro_rules! probe_options {
    ($(#[$m:meta])* $name:ident { $($field:ident : $ty:ty = $default:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Debug, Serialize)]
        pub struct $name {
            pub label: String,
            $(pub $field: $ty,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { label: String::new(), $($field: $default,)* }
            }
        }

        impl $name {
            /// Construct with the given label and neofetch-default options.
            // `..Default::default()` is redundant for option-less probes (only `label`).
            #[allow(clippy::needless_update)]
            pub fn with_label(label: &str) -> Self {
                Self { label: label.to_string(), ..Default::default() }
            }
        }

        impl From<String> for $name {
            #[allow(clippy::needless_update)]
            fn from(label: String) -> Self {
                Self { label, ..Default::default() }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
                // Mirror struct used for the table form; container `default` so a
                // partial table fills the rest from the neofetch defaults.
                #[derive(Deserialize)]
                #[serde(default)]
                struct Full { label: String, $($field: $ty,)* }
                impl Default for Full {
                    fn default() -> Self { Self { label: String::new(), $($field: $default,)* } }
                }

                struct OptVisitor;
                impl<'de> serde::de::Visitor<'de> for OptVisitor {
                    type Value = $name;
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("a label string or an options table")
                    }
                    fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<$name, E> {
                        Ok($name::with_label(s))
                    }
                    fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<$name, A::Error> {
                        let Full { label, $($field,)* } =
                            Full::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                        Ok($name { label, $($field,)* })
                    }
                }
                de.deserialize_any(OptVisitor)
            }
        }
    };
}

/// Memory size unit (`memory_unit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryUnit {
    Kib,
    #[default]
    Mib,
    Gib,
}

/// Uptime verbosity (`uptime_shorthand`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UptimeFormat {
    #[default]
    On,
    Tiny,
    Off,
}

/// CPU core-count style (`cpu_cores`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoresMode {
    #[default]
    Logical,
    Physical,
    Off,
}

/// Which sysfs frequency to report (`speed_type`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedType {
    #[default]
    BiosLimit,
    ScalingCurFreq,
    ScalingMinFreq,
    ScalingMaxFreq,
}

/// GPU filter (`gpu_type`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuType {
    #[default]
    All,
    Dedicated,
    Integrated,
}

/// Package-list verbosity (`package_managers`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageDisplay {
    #[default]
    On,
    Tiny,
    Off,
}

/// Distro-name verbosity (`distro_shorthand`). neofetch defaults to off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shorthand {
    On,
    Tiny,
    #[default]
    Off,
}

/// Disk line subtitle (`disk_subtitle`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskSubtitle {
    #[default]
    Mount,
    Name,
    Dir,
    None,
}

probe_options!(
    /// A probe with no neofetch options beyond its label.
    LabeledOptions {}
);
probe_options!(
    /// Options for the OS/distro line (`distro_shorthand`, `os_arch`).
    DistroOptions {
        shorthand: Shorthand = Shorthand::Off,
        os_arch: bool = true,
    }
);
probe_options!(
    /// Options for the kernel line (`kernel_shorthand`).
    KernelOptions { shorthand: bool = true }
);
probe_options!(
    /// Options for the uptime line (`uptime_shorthand`).
    UptimeOptions { format: UptimeFormat = UptimeFormat::On }
);
probe_options!(
    /// Options for the packages line (`package_managers`).
    PackagesOptions { display: PackageDisplay = PackageDisplay::On }
);
probe_options!(
    /// Options for the shell line (`shell_path`, `shell_version`).
    ShellOptions {
        path: bool = false,
        version: bool = true,
    }
);
probe_options!(
    /// Options for the resolution line (`refresh_rate`).
    ResolutionOptions { refresh_rate: bool = false }
);
probe_options!(
    /// Options for the desktop-environment line (`de_version`).
    DeOptions { version: bool = true }
);
probe_options!(
    /// Options for the CPU line (`cpu_brand`, `cpu_cores`, `cpu_speed`, …).
    CpuOptions {
        brand: bool = true,
        cores: CoresMode = CoresMode::Logical,
        speed: bool = true,
        speed_type: SpeedType = SpeedType::BiosLimit,
        speed_shorthand: bool = false,
    }
);
probe_options!(
    /// Options for the GPU line (`gpu_brand`, `gpu_type`).
    GpuOptions {
        brand: bool = true,
        gpu_type: GpuType = GpuType::All,
    }
);
probe_options!(
    /// Options for the memory line (`memory_unit`, `memory_percent`).
    MemoryOptions {
        unit: MemoryUnit = MemoryUnit::Mib,
        percent: bool = false,
    }
);
probe_options!(
    /// Options for the disk line (`disk_show`, `disk_subtitle`, `disk_percent`).
    /// Empty `show` means every mount point (neofetch defaults to `/`).
    DiskOptions {
        show: Vec<String> = Vec::new(),
        subtitle: DiskSubtitle = DiskSubtitle::Mount,
        percent: bool = true,
    }
);
probe_options!(
    /// Options for the now-playing line (`music_player`, `song_format`, `song_shorthand`).
    SongOptions {
        player: String = "auto".to_string(),
        format: String = "%artist% - %album% - %title%".to_string(),
        shorthand: bool = false,
    }
);

// ── Per-probe value formatting ───────────────────────────────────────────

impl MemoryOptions {
    /// Render `used`/`total` (both KiB) in the configured unit, optionally with
    /// a usage percentage.
    pub fn format(&self, used_kib: u64, total_kib: u64) -> String {
        let (used, total, unit) = match self.unit {
            MemoryUnit::Kib => (used_kib as f64, total_kib as f64, "KiB"),
            MemoryUnit::Mib => (used_kib as f64 / 1024.0, total_kib as f64 / 1024.0, "MiB"),
            MemoryUnit::Gib => (
                used_kib as f64 / 1_048_576.0,
                total_kib as f64 / 1_048_576.0,
                "GiB",
            ),
        };
        let mut s = match self.unit {
            MemoryUnit::Gib => format!("{used:.2}{unit} / {total:.2}{unit}"),
            _ => format!(
                "{}{unit} / {}{unit}",
                used.round() as u64,
                total.round() as u64
            ),
        };
        if self.percent && total_kib > 0 {
            let pct = (used_kib as f64 / total_kib as f64 * 100.0).round() as u64;
            s.push_str(&format!(" ({pct}%)"));
        }
        s
    }
}

impl UptimeOptions {
    /// Render `secs` of uptime in the configured verbosity.
    pub fn format(&self, secs: usize) -> String {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;
        let s = secs % 60;
        match self.format {
            UptimeFormat::Tiny => {
                let mut parts = Vec::new();
                if days > 0 {
                    parts.push(format!("{days}d"));
                }
                if hours > 0 {
                    parts.push(format!("{hours}h"));
                }
                if mins > 0 {
                    parts.push(format!("{mins}m"));
                }
                if parts.is_empty() {
                    parts.push(format!("{s}s"));
                }
                parts.join(" ")
            }
            UptimeFormat::On => uptime_words(days, hours, mins, s, "min", "sec"),
            UptimeFormat::Off => uptime_words(days, hours, mins, s, "minute", "second"),
        }
    }
}

/// Build neofetch's worded uptime ("1 day, 9 hours, 22 mins"), with the
/// minute/second words parameterized for the `on`/`off` shorthand.
///
/// Like neofetch, each of day/hour/min is hidden individually when zero (so an
/// exact-hour uptime reads "9 hours", not "9 hours, 0 mins"), falling back to
/// seconds only when day, hour and minute are all zero.
fn uptime_words(
    days: usize,
    hours: usize,
    mins: usize,
    secs: usize,
    min_word: &str,
    sec_word: &str,
) -> String {
    let unit = |n: usize, word: &str| {
        if n == 1 {
            format!("{n} {word}")
        } else {
            format!("{n} {word}s")
        }
    };
    let mut parts = Vec::with_capacity(3);
    if days > 0 {
        parts.push(unit(days, "day"));
    }
    if hours > 0 {
        parts.push(unit(hours, "hour"));
    }
    if mins > 0 {
        parts.push(unit(mins, min_word));
    }
    if parts.is_empty() {
        return unit(secs, sec_word);
    }
    parts.join(", ")
}

impl KernelOptions {
    /// Render the kernel `version`, prepending the kernel name when
    /// `kernel_shorthand` is off.
    pub fn format(&self, version: &str) -> String {
        if self.shorthand {
            version.to_string()
        } else {
            let name = if cfg!(target_os = "macos") {
                "Darwin"
            } else if cfg!(target_os = "windows") {
                "Windows NT"
            } else {
                "Linux"
            };
            format!("{name} {version}")
        }
    }
}

impl DistroOptions {
    /// Render the OS/distro `name`, applying `distro_shorthand` and appending
    /// the machine architecture when `os_arch` is on.
    pub fn format(&self, name: &str) -> String {
        let mut s = match self.shorthand {
            Shorthand::Off => name.to_string(),
            // Drop a trailing parenthetical (codename), e.g.
            // "Fedora Linux 44 (Silverblue)" -> "Fedora Linux 44".
            Shorthand::On => name.split('(').next().unwrap_or(name).trim().to_string(),
            // Just the distro name (first token), e.g. "Fedora".
            Shorthand::Tiny => name.split_whitespace().next().unwrap_or(name).to_string(),
        };
        if self.os_arch {
            s.push(' ');
            s.push_str(os_arch_str());
        }
        s
    }
}

/// Architecture string with neofetch's (`uname -m`) naming, which differs from
/// Rust's target arch on macOS (`arm64`, not `aarch64`).
fn os_arch_str() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "arm64"
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        std::env::consts::ARCH
    }
}

impl ShellOptions {
    /// Render the shell `name` (or the `$SHELL` path), optionally with `version`.
    pub fn format(&self, name: &str, version: Option<&str>) -> String {
        let mut s = if self.path {
            std::env::var("SHELL").unwrap_or_else(|_| name.to_string())
        } else {
            name.to_string()
        };
        if self.version
            && let Some(v) = version
        {
            s.push(' ');
            s.push_str(v);
        }
        s
    }
}

impl DeOptions {
    /// Render the desktop-environment `name`, optionally with `version`, plus a
    /// trailing " (Wayland)" on Wayland sessions (matching neofetch:
    /// `[[ $de && $WAYLAND_DISPLAY ]] && de+=" (Wayland)"`).
    pub fn format(&self, name: &str, version: Option<&str>) -> String {
        self.format_inner(name, version, std::env::var_os("WAYLAND_DISPLAY").is_some())
    }

    fn format_inner(&self, name: &str, version: Option<&str>, wayland: bool) -> String {
        if name.is_empty() {
            return String::new();
        }
        let mut s = name.to_string();
        if self.version
            && let Some(v) = version
        {
            s.push(' ');
            s.push_str(v);
        }
        // The " (Wayland)" suffix comes after the version, like neofetch. Guard
        // against double-appending if the upstream name already carries it.
        if wayland && !s.contains("(Wayland)") {
            s.push_str(" (Wayland)");
        }
        s
    }
}

impl PackagesOptions {
    /// Render package `counts` per the `package_managers` verbosity: per-manager
    /// breakdown (on), total with manager names (tiny), or bare total (off).
    pub fn format(&self, counts: &[(String, usize)]) -> String {
        let total: usize = counts.iter().map(|(_, c)| c).sum();
        match self.display {
            PackageDisplay::On => counts
                .iter()
                .map(|(m, c)| format!("{c} ({m})"))
                .collect::<Vec<_>>()
                .join(", "),
            PackageDisplay::Tiny => {
                let names = counts
                    .iter()
                    .map(|(m, _)| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{total} ({names})")
            }
            PackageDisplay::Off => total.to_string(),
        }
    }
}

impl GpuOptions {
    /// Render the GPU `name`, optionally dropping the leading vendor token.
    pub fn format(&self, name: &str) -> String {
        if self.brand {
            return name.to_string();
        }
        for brand in ["NVIDIA ", "AMD ", "Intel ", "Advanced Micro Devices, Inc. "] {
            if let Some(rest) = name.strip_prefix(brand) {
                return rest.to_string();
            }
        }
        name.to_string()
    }
}

impl DiskOptions {
    /// Render a disk line: an optional `(subtitle)` prefix (mount/name/dir), the
    /// used/total size in GiB, and an optional usage percentage.
    pub fn format(&self, mount: &Path, name: &str, used: u64, total: u64) -> String {
        let gib = |b: u64| (b as f64 / (1024.0 * 1024.0 * 1024.0)).round() as u64;
        let subtitle = match self.subtitle {
            DiskSubtitle::Mount => Some(mount.display().to_string()),
            DiskSubtitle::Name => Some(name.to_string()),
            DiskSubtitle::Dir => Some(
                mount
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| mount.display().to_string()),
            ),
            DiskSubtitle::None => None,
        };
        let mut s = String::new();
        if let Some(sub) = subtitle.filter(|s| !s.is_empty()) {
            s.push_str(&format!("({sub}) "));
        }
        s.push_str(&format!("{} G / {} G", gib(used), gib(total)));
        if self.percent && total > 0 {
            let pct = (used as f64 / total as f64 * 100.0).round() as u64;
            s.push_str(&format!(" ({pct}%)"));
        }
        s
    }
}

/// Probe config. Each variant selects a metric and carries its display label
/// plus any neofetch-style options. Refer to [`ProbeValue`] for what each
/// metric corresponds to.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ProbeConfig {
    Host(LabeledOptions),
    OS(DistroOptions),
    Model(LabeledOptions),
    Kernel(KernelOptions),
    Distro(LabeledOptions),
    Uptime(UptimeOptions),
    Packages(PackagesOptions),
    Shell(ShellOptions),
    Editor(LabeledOptions),
    Resolution(ResolutionOptions),
    DE(DeOptions),
    WM(LabeledOptions),
    WMTheme(LabeledOptions),
    Theme(LabeledOptions),
    Icons(LabeledOptions),
    Cursor(LabeledOptions),
    Terminal(LabeledOptions),
    TerminalFont(LabeledOptions),
    CPU(CpuOptions),
    GPU(GpuOptions),
    Memory(MemoryOptions),
    Network(LabeledOptions),
    Bluetooth(LabeledOptions),
    BIOS(LabeledOptions),

    GPUDriver(LabeledOptions),
    CPUUsage(LabeledOptions),
    Disk(DiskOptions),
    Battery(LabeledOptions),
    PowerAdapter(LabeledOptions),
    Font(LabeledOptions),
    Song(SongOptions),
    LocalIP(LabeledOptions),
    PublicIP(LabeledOptions),
    Users(LabeledOptions),
    Locale(LabeledOptions),

    Java(LabeledOptions),
    Python(LabeledOptions),
    Node(LabeledOptions),
    Rust(LabeledOptions),
}

impl ProbeConfig {
    /// Default config enabling all probes (the `--all` preset).
    pub fn default_all() -> Vec<Self> {
        vec![
            Self::OS(DistroOptions::with_label("OS")),
            Self::Model(LabeledOptions::with_label("Host")),
            Self::Kernel(KernelOptions::with_label("Kernel")),
            Self::Uptime(UptimeOptions::with_label("Uptime")),
            Self::Packages(PackagesOptions::with_label("Packages")),
            Self::Shell(ShellOptions::with_label("Shell")),
            Self::Editor(LabeledOptions::with_label("Editor")),
            Self::Resolution(ResolutionOptions::with_label("Resolution")),
            Self::DE(DeOptions::with_label("DE")),
            Self::WM(LabeledOptions::with_label("WM")),
            Self::WMTheme(LabeledOptions::with_label("WM Theme")),
            Self::Theme(LabeledOptions::with_label("Theme")),
            Self::Icons(LabeledOptions::with_label("Icons")),
            Self::Cursor(LabeledOptions::with_label("Cursor")),
            Self::Terminal(LabeledOptions::with_label("Terminal")),
            Self::TerminalFont(LabeledOptions::with_label("Terminal Font")),
            Self::CPU(CpuOptions::with_label("CPU")),
            Self::GPU(GpuOptions::with_label("GPU")),
            Self::Memory(MemoryOptions::with_label("Memory")),
            Self::Network(LabeledOptions::with_label("Network")),
            Self::Bluetooth(LabeledOptions::with_label("Bluetooth")),
            Self::BIOS(LabeledOptions::with_label("BIOS")),
            Self::GPUDriver(LabeledOptions::with_label("GPU Driver")),
            Self::CPUUsage(LabeledOptions::with_label("CPU Usage")),
            Self::Disk(DiskOptions::with_label("Disk")),
            Self::Battery(LabeledOptions::with_label("Battery")),
            Self::PowerAdapter(LabeledOptions::with_label("Power Adapter")),
            Self::Font(LabeledOptions::with_label("Font")),
            Self::Song(SongOptions::with_label("Song")),
            Self::LocalIP(LabeledOptions::with_label("Local IP")),
            Self::PublicIP(LabeledOptions::with_label("Public IP")),
            Self::Users(LabeledOptions::with_label("Users")),
            Self::Locale(LabeledOptions::with_label("Locale")),
            Self::Java(LabeledOptions::with_label("Java")),
            Self::Python(LabeledOptions::with_label("Python")),
            Self::Node(LabeledOptions::with_label("Node")),
            Self::Rust(LabeledOptions::with_label("Rust")),
        ]
    }

    /// Default config replicating neofetch's default info block.
    pub fn default_neofetch() -> Vec<Self> {
        vec![
            Self::OS(DistroOptions::with_label("OS")),
            Self::Model(LabeledOptions::with_label("Host")),
            Self::Kernel(KernelOptions::with_label("Kernel")),
            Self::Uptime(UptimeOptions::with_label("Uptime")),
            Self::Packages(PackagesOptions::with_label("Packages")),
            Self::Shell(ShellOptions::with_label("Shell")),
            Self::Resolution(ResolutionOptions::with_label("Resolution")),
            Self::DE(DeOptions::with_label("DE")),
            Self::WM(LabeledOptions::with_label("WM")),
            Self::WMTheme(LabeledOptions::with_label("WM Theme")),
            Self::Theme(LabeledOptions::with_label("Theme")),
            Self::Icons(LabeledOptions::with_label("Icons")),
            Self::Terminal(LabeledOptions::with_label("Terminal")),
            Self::TerminalFont(LabeledOptions::with_label("Terminal Font")),
            Self::CPU(CpuOptions::with_label("CPU")),
            Self::GPU(GpuOptions::with_label("GPU")),
            Self::Memory(MemoryOptions::with_label("Memory")),
        ]
    }

    /// Display label for this probe.
    pub fn label(&self) -> &str {
        match self {
            Self::Host(o) => &o.label,
            Self::OS(o) => &o.label,
            Self::Model(o) => &o.label,
            Self::Kernel(o) => &o.label,
            Self::Distro(o) => &o.label,
            Self::Uptime(o) => &o.label,
            Self::Packages(o) => &o.label,
            Self::Shell(o) => &o.label,
            Self::Editor(o) => &o.label,
            Self::Resolution(o) => &o.label,
            Self::DE(o) => &o.label,
            Self::WM(o) => &o.label,
            Self::WMTheme(o) => &o.label,
            Self::Theme(o) => &o.label,
            Self::Icons(o) => &o.label,
            Self::Cursor(o) => &o.label,
            Self::Terminal(o) => &o.label,
            Self::TerminalFont(o) => &o.label,
            Self::CPU(o) => &o.label,
            Self::GPU(o) => &o.label,
            Self::Memory(o) => &o.label,
            Self::Network(o) => &o.label,
            Self::Bluetooth(o) => &o.label,
            Self::BIOS(o) => &o.label,
            Self::GPUDriver(o) => &o.label,
            Self::CPUUsage(o) => &o.label,
            Self::Disk(o) => &o.label,
            Self::Battery(o) => &o.label,
            Self::PowerAdapter(o) => &o.label,
            Self::Font(o) => &o.label,
            Self::Song(o) => &o.label,
            Self::LocalIP(o) => &o.label,
            Self::PublicIP(o) => &o.label,
            Self::Users(o) => &o.label,
            Self::Locale(o) => &o.label,
            Self::Java(o) => &o.label,
            Self::Python(o) => &o.label,
            Self::Node(o) => &o.label,
            Self::Rust(o) => &o.label,
        }
    }

    /// Stable machine-readable key for this probe (used by JSON output).
    pub fn id(&self) -> &'static str {
        self.probe_type().id()
    }

    /// The underlying metric this probe gathers.
    pub(crate) fn probe_type(&self) -> ProbeType {
        match self {
            Self::Host(_) => ProbeType::Host,
            Self::OS(_) => ProbeType::OS,
            Self::Model(_) => ProbeType::Model,
            Self::Kernel(_) => ProbeType::Kernel,
            Self::Distro(_) => ProbeType::Distro,
            Self::Uptime(_) => ProbeType::Uptime,
            Self::Packages(_) => ProbeType::Packages,
            Self::Shell(_) => ProbeType::Shell,
            Self::Editor(_) => ProbeType::Editor,
            Self::Resolution(_) => ProbeType::Resolution,
            Self::DE(_) => ProbeType::DE,
            Self::WM(_) => ProbeType::WM,
            Self::WMTheme(_) => ProbeType::WMTheme,
            Self::Theme(_) => ProbeType::Theme,
            Self::Icons(_) => ProbeType::Icons,
            Self::Cursor(_) => ProbeType::Cursor,
            Self::Terminal(_) => ProbeType::Terminal,
            Self::TerminalFont(_) => ProbeType::TerminalFont,
            Self::CPU(_) => ProbeType::CPU,
            Self::GPU(_) => ProbeType::GPU,
            Self::Memory(_) => ProbeType::Memory,
            Self::Network(_) => ProbeType::Network,
            Self::Bluetooth(_) => ProbeType::Bluetooth,
            Self::BIOS(_) => ProbeType::BIOS,
            Self::GPUDriver(_) => ProbeType::GPUDriver,
            Self::CPUUsage(_) => ProbeType::CPUUsage,
            Self::Disk(_) => ProbeType::Disk,
            Self::Battery(_) => ProbeType::Battery,
            Self::PowerAdapter(_) => ProbeType::PowerAdapter,
            Self::Font(_) => ProbeType::Font,
            Self::Song(_) => ProbeType::Song,
            Self::LocalIP(_) => ProbeType::LocalIP,
            Self::PublicIP(_) => ProbeType::PublicIP,
            Self::Users(_) => ProbeType::Users,
            Self::Locale(_) => ProbeType::Locale,
            Self::Java(_) => ProbeType::Java,
            Self::Python(_) => ProbeType::Python,
            Self::Node(_) => ProbeType::Node,
            Self::Rust(_) => ProbeType::Rust,
        }
    }

    /// Build the `(label, probe function)` pair the renderer executes.
    pub fn get_funcs(&self) -> (String, ProbeResultFunction) {
        // Disk/GPU capture their options into the closure (mount filtering and
        // GPU-type filtering happen at gather time); the rest are option-
        // independent when gathering and format via `format_value`.
        let func: ProbeResultFunction = match self {
            Self::Disk(o) => crate::probe::disk_probe_fn(o.clone()),
            Self::GPU(o) => crate::probe::gpu_probe_fn(o.clone()),
            Self::Song(o) => crate::probe::song_probe_fn(o.clone()),
            _ => self.probe_type().into(),
        };
        (self.label().to_string(), func)
    }

    /// Render a probed value for this probe, honoring its options. Falls back
    /// to the option-free [`ProbeValue::format`] for probes without options.
    pub fn format_value(&self, value: &ProbeValue) -> String {
        match (self, value) {
            (Self::CPU(o), ProbeValue::CPU(model)) => crate::probe::format_cpu(model, o),
            (Self::OS(o), ProbeValue::OS(name)) => o.format(name),
            (Self::Kernel(o), ProbeValue::Kernel(v)) => o.format(v),
            (Self::Uptime(o), ProbeValue::Uptime(s)) => o.format(*s),
            (Self::Memory(o), ProbeValue::Memory(u, t)) => o.format(*u, *t),
            (Self::Shell(o), ProbeValue::Shell(name, ver)) => o.format(name, ver.as_deref()),
            (Self::DE(o), ProbeValue::DE(name, ver)) => o.format(name, ver.as_deref()),
            (Self::Packages(o), ProbeValue::Packages(counts)) => o.format(counts),
            (Self::Disk(o), ProbeValue::Disk(mount, name, used, total)) => {
                o.format(mount, name, *used, *total)
            }
            (Self::GPU(o), ProbeValue::GPU(name)) => o.format(name),
            _ => value.format(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrips_through_toml() {
        let cfg = Config::default();
        let serialized = toml::to_string(&cfg).expect("serialize");
        let back: Config = toml::from_str(&serialized).expect("deserialize");
        match back {
            Config::Neofetch(c) => assert!(!c.probes.is_empty()),
            Config::Json(_) => panic!("expected Neofetch"),
        }
    }

    #[test]
    fn probe_accepts_bare_label_and_table_forms() {
        let src = r#"
[Neofetch]
title = true
underline = true
col = true
probes = [
    { OS = "Operating System" },
    { CPU = { label = "Processor", cores = "physical" } },
]
"#;
        let cfg: Config = toml::from_str(src).expect("deserialize");
        match cfg {
            Config::Neofetch(c) => {
                assert_eq!(c.probes.len(), 2);
                assert_eq!(c.probes[0].label(), "Operating System");
                assert_eq!(c.probes[1].label(), "Processor");
                match &c.probes[1] {
                    ProbeConfig::CPU(o) => assert_eq!(o.cores, CoresMode::Physical),
                    other => panic!("expected CPU, got {other:?}"),
                }
            }
            Config::Json(_) => panic!("expected Neofetch"),
        }
    }

    #[test]
    fn memory_units_and_percent() {
        let mut o = MemoryOptions::with_label("Memory");
        assert_eq!(o.format(1024 * 1024, 2 * 1024 * 1024), "1024MiB / 2048MiB");
        o.percent = true;
        assert_eq!(
            o.format(1024 * 1024, 2 * 1024 * 1024),
            "1024MiB / 2048MiB (50%)"
        );
        o.unit = MemoryUnit::Gib;
        o.percent = false;
        assert_eq!(o.format(1024 * 1024, 2 * 1024 * 1024), "1.00GiB / 2.00GiB");
    }

    #[test]
    fn uptime_formats() {
        let secs = 86400 + 9 * 3600 + 22 * 60;
        let on = UptimeOptions {
            format: UptimeFormat::On,
            ..Default::default()
        };
        let off = UptimeOptions {
            format: UptimeFormat::Off,
            ..Default::default()
        };
        let tiny = UptimeOptions {
            format: UptimeFormat::Tiny,
            ..Default::default()
        };
        assert_eq!(on.format(secs), "1 day, 9 hours, 22 mins");
        assert_eq!(off.format(secs), "1 day, 9 hours, 22 minutes");
        assert_eq!(tiny.format(secs), "1d 9h 22m");

        // Zero-valued fields are hidden individually (neofetch parity), rather
        // than rendered as "0 hours" / "0 mins".
        assert_eq!(on.format(2 * 86400 + 5 * 60), "2 days, 5 mins");
        assert_eq!(on.format(9 * 3600), "9 hours");
        assert_eq!(on.format(86400 + 3 * 3600), "1 day, 3 hours");
        assert_eq!(tiny.format(2 * 86400 + 5 * 60), "2d 5m");
        // All of day/hour/min zero -> seconds fallback.
        assert_eq!(on.format(45), "45 secs");
        assert_eq!(off.format(45), "45 seconds");
    }

    #[test]
    fn de_wayland_suffix() {
        let de = DeOptions {
            version: true,
            ..Default::default()
        };
        // Wayland appends after the version; X11 / non-Wayland does not.
        assert_eq!(
            de.format_inner("GNOME", Some("50.2"), true),
            "GNOME 50.2 (Wayland)"
        );
        assert_eq!(de.format_inner("GNOME", Some("50.2"), false), "GNOME 50.2");
        // Appended even when the version is hidden, and never doubled.
        let no_ver = DeOptions {
            version: false,
            ..Default::default()
        };
        assert_eq!(
            no_ver.format_inner("KDE", Some("6.1"), true),
            "KDE (Wayland)"
        );
        assert_eq!(
            de.format_inner("GNOME (Wayland)", None, true),
            "GNOME (Wayland)"
        );
        // Empty DE stays empty (renderer omits the line).
        assert_eq!(de.format_inner("", None, true), "");
    }

    #[test]
    fn distro_arch_and_shorthand() {
        let full = DistroOptions {
            shorthand: Shorthand::Off,
            os_arch: false,
            ..Default::default()
        };
        assert_eq!(
            full.format("Fedora Linux 44 (Silverblue)"),
            "Fedora Linux 44 (Silverblue)"
        );
        let on = DistroOptions {
            shorthand: Shorthand::On,
            os_arch: false,
            ..Default::default()
        };
        assert_eq!(on.format("Fedora Linux 44 (Silverblue)"), "Fedora Linux 44");
        let tiny = DistroOptions {
            shorthand: Shorthand::Tiny,
            os_arch: false,
            ..Default::default()
        };
        assert_eq!(tiny.format("Fedora Linux 44 (Silverblue)"), "Fedora");
    }

    #[test]
    fn packages_display_modes() {
        let counts = vec![
            ("rpm".to_string(), 1998usize),
            ("cargo".to_string(), 55),
            ("flatpak".to_string(), 69),
        ];
        let on = PackagesOptions {
            display: PackageDisplay::On,
            ..Default::default()
        };
        assert_eq!(on.format(&counts), "1998 (rpm), 55 (cargo), 69 (flatpak)");
        let tiny = PackagesOptions {
            display: PackageDisplay::Tiny,
            ..Default::default()
        };
        assert_eq!(tiny.format(&counts), "2122 (rpm, cargo, flatpak)");
        let off = PackagesOptions {
            display: PackageDisplay::Off,
            ..Default::default()
        };
        assert_eq!(off.format(&counts), "2122");
    }

    #[test]
    fn gpu_brand_toggle() {
        let on = GpuOptions::with_label("GPU");
        assert_eq!(
            on.format("NVIDIA GeForce RTX 4090"),
            "NVIDIA GeForce RTX 4090"
        );
        let off = GpuOptions {
            brand: false,
            ..Default::default()
        };
        assert_eq!(off.format("NVIDIA GeForce RTX 4090"), "GeForce RTX 4090");
    }

    #[test]
    fn disk_subtitle_and_percent() {
        let gib = 1024u64 * 1024 * 1024;
        let default = DiskOptions::with_label("Disk"); // subtitle = mount, percent = true
        assert_eq!(
            default.format(Path::new("/"), "/dev/sda1", 42 * gib, 256 * gib),
            "(/) 42 G / 256 G (16%)"
        );
        let bare = DiskOptions {
            subtitle: DiskSubtitle::None,
            percent: false,
            ..Default::default()
        };
        assert_eq!(
            bare.format(Path::new("/"), "/dev/sda1", 42 * gib, 256 * gib),
            "42 G / 256 G"
        );
    }
}
