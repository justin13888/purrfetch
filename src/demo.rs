//! Curated example presets for `--example`: canned probe values rendered
//! through the real formatting pipeline, so any distro's output can be shown
//! on any machine (README gallery, docs, screenshots) with fully fictional
//! identities and deterministic values.
//!
//! This module must never touch live readouts: no libmacchina, no env vars,
//! no clocks. Every preset field is a compile-time constant, which is what
//! keeps the generated gallery SVGs byte-stable across runs and hosts.

use std::path::PathBuf;

use crate::config::{CoresMode, ProbeConfig};
use crate::probe::{
    ProbeError, ProbeResult, ProbeResultFunction, ProbeResultValue, ProbeType, ProbeValue,
};

/// The presets selectable via `purr --example <PRESET>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum DemoPresetId {
    /// Fedora Workstation desktop (the README hero).
    FedoraDesktop,
    Arch,
    Nixos,
    Gentoo,
    DebianServer,
    Ubuntu,
    Macos,
    Windows,
    Void,
}

impl DemoPresetId {
    pub const ALL: [DemoPresetId; 9] = [
        Self::FedoraDesktop,
        Self::Arch,
        Self::Nixos,
        Self::Gentoo,
        Self::DebianServer,
        Self::Ubuntu,
        Self::Macos,
        Self::Windows,
        Self::Void,
    ];

    pub fn preset(self) -> &'static DemoPreset {
        match self {
            Self::FedoraDesktop => &FEDORA_DESKTOP,
            Self::Arch => &ARCH,
            Self::Nixos => &NIXOS,
            Self::Gentoo => &GENTOO,
            Self::DebianServer => &DEBIAN_SERVER,
            Self::Ubuntu => &UBUNTU,
            Self::Macos => &MACOS,
            Self::Windows => &WINDOWS,
            Self::Void => &VOID,
        }
    }
}

/// One curated fake system. `None`/empty fields render as a live probe
/// failure would (the line is skipped), e.g. no DE on the server preset.
///
/// Strings are stored post-cleanup (what neofetch would display): the CPU
/// string carries its core count and clock baked in, the OS string its
/// architecture, and the DE version its `(Wayland)` suffix — see
/// [`normalize_probes`] for why.
pub struct DemoPreset {
    pub username: &'static str,
    /// Dot-free by convention, so `title_fqdn` on/off renders identically.
    pub hostname: &'static str,
    /// Pretty distro name; drives logo lookup, tint, and JSON `distro`.
    pub distro: &'static str,
    pub os: &'static str,
    pub model: Option<(&'static str, &'static str)>,
    pub kernel: &'static str,
    pub uptime_secs: usize,
    pub packages: &'static [(&'static str, usize)],
    pub shell: Option<(&'static str, Option<&'static str>)>,
    pub resolution: Option<&'static str>,
    pub de: Option<(&'static str, Option<&'static str>)>,
    pub wm: Option<&'static str>,
    pub wm_theme: Option<&'static str>,
    pub theme: Option<&'static str>,
    pub icons: Option<&'static str>,
    pub cursor: Option<&'static str>,
    pub terminal: Option<&'static str>,
    pub terminal_font: Option<&'static str>,
    pub cpu: &'static str,
    pub gpus: &'static [&'static str],
    /// (used, total) in KiB, like the live memory probe.
    pub memory_kib: (u64, u64),
    /// (mount, filesystem, used, total) in bytes.
    pub disk: Option<(&'static str, &'static str, u64, u64)>,
    pub battery: Option<u8>,
    pub song: Option<&'static str>,
    pub local_ip: Option<&'static str>,
    pub locale: Option<&'static str>,
}

const fn gib(n: u64) -> u64 {
    n * 1024 * 1024 * 1024
}

const fn mib_kib(n: u64) -> u64 {
    n * 1024
}

impl DemoPreset {
    /// Canned result for one metric. Total over [`ProbeType`]: every arm
    /// returns `Ok` or `Err(MetricsUnavailable)` (mirroring a live probe
    /// failing, which the renderers skip), never panics.
    pub fn value(&self, t: ProbeType) -> ProbeResult {
        fn one(v: ProbeValue) -> ProbeResult {
            Ok(ProbeResultValue::Single(v))
        }
        fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> ProbeValue) -> ProbeResult {
            v.map(f)
                .map(ProbeResultValue::Single)
                .ok_or(ProbeError::MetricsUnavailable)
        }
        match t {
            ProbeType::Host => one(ProbeValue::Host(self.username.into(), self.hostname.into())),
            ProbeType::OS => one(ProbeValue::OS(self.os.into())),
            ProbeType::Distro => one(ProbeValue::Distro(self.distro.into())),
            ProbeType::Model => opt(self.model, |(v, p)| ProbeValue::Model(v.into(), p.into())),
            ProbeType::Kernel => one(ProbeValue::Kernel(self.kernel.into())),
            ProbeType::Uptime => one(ProbeValue::Uptime(self.uptime_secs)),
            ProbeType::Packages => one(ProbeValue::Packages(
                self.packages
                    .iter()
                    .map(|&(m, c)| (m.to_string(), c))
                    .collect(),
            )),
            ProbeType::Shell => opt(self.shell, |(n, v)| {
                ProbeValue::Shell(n.into(), v.map(Into::into))
            }),
            ProbeType::Resolution => opt(self.resolution, |r| ProbeValue::Resolution(r.into())),
            ProbeType::DE => opt(self.de, |(n, v)| {
                ProbeValue::DE(n.into(), v.map(Into::into))
            }),
            ProbeType::WM => opt(self.wm, |w| ProbeValue::WM(w.into())),
            ProbeType::WMTheme => opt(self.wm_theme, |w| ProbeValue::WMTheme(w.into())),
            ProbeType::Theme => opt(self.theme, |v| ProbeValue::Theme(v.into())),
            ProbeType::Icons => opt(self.icons, |v| ProbeValue::Icons(v.into())),
            ProbeType::Cursor => opt(self.cursor, |v| ProbeValue::Cursor(v.into())),
            ProbeType::Terminal => opt(self.terminal, |v| ProbeValue::Terminal(v.into())),
            ProbeType::TerminalFont => {
                opt(self.terminal_font, |v| ProbeValue::TerminalFont(v.into()))
            }
            ProbeType::CPU => one(ProbeValue::CPU(self.cpu.into())),
            ProbeType::GPU => Ok(ProbeResultValue::Multiple(
                self.gpus
                    .iter()
                    .map(|&g| ProbeValue::GPU(g.into()))
                    .collect(),
            )),
            ProbeType::Memory => one(ProbeValue::Memory(self.memory_kib.0, self.memory_kib.1)),
            ProbeType::Disk => opt(self.disk, |(m, n, used, total)| {
                ProbeValue::Disk(PathBuf::from(m), n.into(), used, total)
            }),
            ProbeType::Battery => opt(self.battery, ProbeValue::Battery),
            ProbeType::Song => opt(self.song, |s| ProbeValue::Song(s.into())),
            ProbeType::LocalIP => opt(self.local_ip, |v| ProbeValue::LocalIP(v.into())),
            ProbeType::Users => one(ProbeValue::Users(vec![self.username.to_string()])),
            ProbeType::Locale => opt(self.locale, |v| ProbeValue::Locale(v.into())),
            // Not part of the curated data — render as a live probe failure.
            ProbeType::Editor
            | ProbeType::Network
            | ProbeType::Bluetooth
            | ProbeType::BIOS
            | ProbeType::GPUDriver
            | ProbeType::CPUUsage
            | ProbeType::PowerAdapter
            | ProbeType::Font
            | ProbeType::PublicIP
            | ProbeType::Java
            | ProbeType::Python
            | ProbeType::Node
            | ProbeType::Rust => Err(ProbeError::MetricsUnavailable),
        }
    }

    /// Canned probe closure — same shape as a live probe, never blocks.
    pub fn probe_fn(&'static self, t: ProbeType) -> ProbeResultFunction {
        Box::new(move || self.value(t))
    }
}

/// Neutralize the format-time options that would leak live data into demo
/// output: `format_cpu` reads live core counts/clocks (baked into the preset
/// CPU string instead), `DistroOptions` appends the compile-time arch (baked
/// into the preset OS string), and `shell_path` reads `$SHELL`.
pub fn normalize_probes(probes: &mut [ProbeConfig]) {
    for p in probes {
        match p {
            ProbeConfig::OS(o) => o.os_arch = false,
            ProbeConfig::CPU(o) => {
                o.cores = CoresMode::Off;
                o.speed = false;
            }
            ProbeConfig::Shell(o) => o.path = false,
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Presets. Identities are fictional; hardware/software combos are plausible
// but deliberately match no real machine.
// ─────────────────────────────────────────────────────────────────────────

/// The README hero: a Fedora Workstation desktop.
pub static FEDORA_DESKTOP: DemoPreset = DemoPreset {
    username: "rice",
    hostname: "macchiato",
    distro: "Fedora Linux",
    os: "Fedora Linux 44 (Workstation Edition) x86_64",
    model: Some(("ASUS", "ROG STRIX B650E-F GAMING WIFI")),
    kernel: "6.15.4-200.fc44.x86_64",
    uptime_secs: 4 * 3600 + 23 * 60,
    packages: &[("rpm", 2143), ("flatpak", 41)],
    shell: Some(("zsh", Some("5.9"))),
    resolution: Some("3440x1440"),
    de: Some(("GNOME", Some("48 (Wayland)"))),
    wm: Some("Mutter"),
    wm_theme: Some("Adwaita"),
    theme: Some("Adwaita-dark"),
    icons: Some("Papirus-Dark"),
    cursor: Some("Adwaita"),
    terminal: Some("Ptyxis"),
    terminal_font: Some("JetBrains Mono 11"),
    cpu: "AMD Ryzen 7 9800X3D (16) @ 5.27GHz",
    gpus: &["AMD Radeon RX 7800 XT"],
    memory_kib: (mib_kib(6742), mib_kib(32 * 1024)),
    disk: Some(("/", "btrfs", gib(412), gib(953))),
    battery: None,
    song: Some("Tame Impala - Currents - Let It Happen"),
    local_ip: Some("192.168.1.23"),
    locale: Some("en_US.UTF-8"),
};

pub static ARCH: DemoPreset = DemoPreset {
    username: "kt",
    hostname: "tokyo",
    distro: "Arch Linux",
    os: "Arch Linux x86_64",
    model: Some(("MSI", "MAG B650 TOMAHAWK WIFI")),
    kernel: "6.15.8-arch1-1",
    uptime_secs: 2 * 3600 + 4 * 60,
    packages: &[("pacman", 1187)],
    shell: Some(("fish", Some("4.0.2"))),
    resolution: Some("2560x1440"),
    de: None,
    wm: Some("Hyprland"),
    wm_theme: None,
    theme: Some("Catppuccin-Mocha"),
    icons: Some("Papirus-Dark"),
    cursor: Some("Bibata-Modern-Ice"),
    terminal: Some("kitty"),
    terminal_font: Some("JetBrainsMono Nerd Font 12"),
    cpu: "AMD Ryzen 9 7950X (32) @ 5.75GHz",
    gpus: &["NVIDIA GeForce RTX 4080 SUPER"],
    memory_kib: (mib_kib(4913), mib_kib(64 * 1024)),
    disk: Some(("/", "ext4", gib(217), gib(931))),
    battery: None,
    song: Some("Lemaitre - Relativity 3 - Continuum"),
    local_ip: Some("192.168.1.42"),
    locale: Some("en_US.UTF-8"),
};

pub static NIXOS: DemoPreset = DemoPreset {
    username: "nix",
    hostname: "delta",
    distro: "NixOS",
    os: "NixOS 26.05 (Xantusia) x86_64",
    model: Some(("ASRock", "Z890 Steel Legend")),
    kernel: "6.14.9",
    uptime_secs: 86400 + 3 * 3600 + 12 * 60,
    packages: &[("nix-system", 1342), ("nix-user", 267)],
    shell: Some(("fish", Some("4.0.2"))),
    resolution: Some("2560x1440"),
    de: Some(("GNOME", Some("48 (Wayland)"))),
    wm: Some("Mutter"),
    wm_theme: Some("Adwaita"),
    theme: Some("Adwaita"),
    icons: Some("Adwaita"),
    cursor: Some("Adwaita"),
    terminal: Some("Ghostty"),
    terminal_font: Some("JetBrains Mono 12"),
    cpu: "Intel Ultra 7 265K (20) @ 5.50GHz",
    gpus: &["Intel Arc B580"],
    memory_kib: (mib_kib(5321), mib_kib(32 * 1024)),
    disk: Some(("/", "btrfs", gib(156), gib(476))),
    battery: None,
    song: None,
    local_ip: Some("10.0.0.7"),
    locale: Some("en_US.UTF-8"),
};

pub static GENTOO: DemoPreset = DemoPreset {
    username: "larry",
    hostname: "anvil",
    distro: "Gentoo",
    os: "Gentoo Linux x86_64",
    model: Some(("Gigabyte", "X670E AORUS MASTER")),
    kernel: "6.15.9-gentoo",
    uptime_secs: 6 * 86400 + 14 * 3600,
    packages: &[("emerge", 1456)],
    shell: Some(("zsh", Some("5.9"))),
    resolution: Some("3840x2160"),
    de: None,
    wm: Some("bspwm"),
    wm_theme: None,
    theme: Some("Gruvbox-Dark"),
    icons: Some("Gruvbox-Plus"),
    cursor: None,
    terminal: Some("Alacritty"),
    terminal_font: Some("Iosevka 11"),
    cpu: "AMD Ryzen 9 9950X (32) @ 5.67GHz",
    gpus: &["AMD Radeon RX 7900 XTX"],
    memory_kib: (mib_kib(3874), mib_kib(64 * 1024)),
    disk: Some(("/", "ext4", gib(389), gib(1863))),
    battery: None,
    song: None,
    local_ip: Some("192.168.0.11"),
    locale: Some("en_US.UTF-8"),
};

/// Headless server: no DE/WM, no resolution, no theming, no terminal.
pub static DEBIAN_SERVER: DemoPreset = DemoPreset {
    username: "ops",
    hostname: "atlas",
    distro: "Debian GNU/Linux",
    os: "Debian GNU/Linux 13 (trixie) x86_64",
    model: Some(("Supermicro", "AS-2015CS-TNR")),
    kernel: "6.12.30-amd64",
    uptime_secs: 142 * 86400 + 7 * 3600 + 51 * 60,
    packages: &[("dpkg", 812)],
    shell: Some(("bash", Some("5.2.37"))),
    resolution: None,
    de: None,
    wm: None,
    wm_theme: None,
    theme: None,
    icons: None,
    cursor: None,
    terminal: None,
    terminal_font: None,
    cpu: "AMD EPYC 9354 (64) @ 3.80GHz",
    gpus: &["ASPEED Graphics Family"],
    memory_kib: (mib_kib(2011), mib_kib(128 * 1024)),
    disk: Some(("/", "ext4", gib(102), gib(447))),
    battery: None,
    song: None,
    local_ip: Some("10.20.0.4"),
    locale: Some("C.UTF-8"),
};

pub static UBUNTU: DemoPreset = DemoPreset {
    username: "mei",
    hostname: "lily",
    distro: "Ubuntu",
    os: "Ubuntu 26.04 LTS x86_64",
    model: Some(("Dell Inc.", "OptiPlex 7020")),
    kernel: "6.14.0-27-generic",
    uptime_secs: 3 * 3600 + 40 * 60,
    packages: &[("dpkg", 2314), ("snap", 12)],
    shell: Some(("bash", Some("5.2.37"))),
    resolution: Some("1920x1080"),
    de: Some(("GNOME", Some("48 (Wayland)"))),
    wm: Some("Mutter"),
    wm_theme: Some("Adwaita"),
    theme: Some("Yaru-dark"),
    icons: Some("Yaru"),
    cursor: Some("Yaru"),
    terminal: Some("GNOME Terminal"),
    terminal_font: Some("Ubuntu Mono 13"),
    cpu: "Intel i7-14700K (28) @ 5.60GHz",
    gpus: &["NVIDIA GeForce RTX 4070"],
    memory_kib: (mib_kib(5872), mib_kib(32 * 1024)),
    disk: Some(("/", "ext4", gib(268), gib(931))),
    battery: None,
    song: None,
    local_ip: Some("192.168.10.5"),
    locale: Some("en_US.UTF-8"),
};

/// A MacBook Pro; vendor-less model matches how macOS reports it.
pub static MACOS: DemoPreset = DemoPreset {
    username: "aki",
    hostname: "kumo",
    distro: "macOS",
    os: "macOS 26.1 25B77 arm64",
    model: Some(("", "Mac16,8")),
    kernel: "25.1.0",
    uptime_secs: 8 * 86400 + 2 * 3600 + 17 * 60,
    packages: &[("brew", 156)],
    shell: Some(("zsh", Some("5.9"))),
    resolution: Some("1728x1117"),
    de: Some(("Aqua", None)),
    wm: Some("Quartz Compositor"),
    wm_theme: Some("Multicolor (Dark)"),
    theme: None,
    icons: None,
    cursor: None,
    terminal: Some("Ghostty"),
    terminal_font: Some("JetBrains Mono 12"),
    cpu: "Apple M4 Pro",
    gpus: &["Apple M4 Pro"],
    memory_kib: (mib_kib(9214), mib_kib(24 * 1024)),
    disk: Some(("/", "APFS", gib(302), gib(494))),
    battery: Some(86),
    song: None,
    local_ip: Some("192.168.1.87"),
    locale: Some("en_US.UTF-8"),
};

/// A Windows 11 gaming laptop.
pub static WINDOWS: DemoPreset = DemoPreset {
    username: "alex",
    hostname: "aurora",
    distro: "Windows",
    os: "Windows 11 Pro x86_64",
    model: Some(("ASUS", "ROG Zephyrus G16 GU605")),
    kernel: "10.0.26100",
    uptime_secs: 5 * 3600 + 31 * 60,
    packages: &[("winget", 83)],
    shell: Some(("PowerShell", Some("7.5.2"))),
    resolution: Some("2560x1600"),
    de: Some(("Fluent", None)),
    wm: Some("Desktop Window Manager"),
    wm_theme: Some("Dark"),
    theme: Some("Windows 11 Dark"),
    icons: Some("Windows 11"),
    cursor: Some("Windows Default"),
    terminal: Some("Windows Terminal"),
    terminal_font: Some("Cascadia Mono 12"),
    cpu: "Intel Ultra 9 185H (22) @ 5.10GHz",
    gpus: &["NVIDIA GeForce RTX 4070 Laptop GPU"],
    memory_kib: (mib_kib(10192), mib_kib(32 * 1024)),
    disk: Some((r"C:\", "NTFS", gib(384), gib(952))),
    battery: Some(72),
    song: None,
    local_ip: Some("192.168.1.54"),
    locale: Some("en-CA"),
};

pub static VOID: DemoPreset = DemoPreset {
    username: "vd",
    hostname: "mute",
    distro: "Void Linux",
    os: "Void Linux x86_64",
    model: Some(("Lenovo", "ThinkCentre M70q Gen 4")),
    kernel: "6.12.31_1",
    uptime_secs: 11 * 3600 + 2 * 60,
    packages: &[("xbps", 846)],
    shell: Some(("bash", Some("5.2.37"))),
    resolution: Some("1920x1080"),
    de: None,
    wm: Some("dwm"),
    wm_theme: None,
    theme: None,
    icons: None,
    cursor: None,
    terminal: Some("st"),
    terminal_font: Some("Terminus 12"),
    cpu: "Intel i5-12400 (12) @ 4.40GHz",
    gpus: &["Intel UHD Graphics 730"],
    memory_kib: (mib_kib(1893), mib_kib(16 * 1024)),
    disk: Some(("/", "ext4", gib(58), gib(238))),
    battery: None,
    song: None,
    local_ip: Some("192.168.1.66"),
    locale: Some("en_US.UTF-8"),
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `ProbeType`, kept in sync with the enum by the compiler wherever
    /// possible (the `value()` match is exhaustive; this list feeds tests).
    const ALL_PROBE_TYPES: [ProbeType; 39] = [
        ProbeType::Host,
        ProbeType::OS,
        ProbeType::Model,
        ProbeType::Kernel,
        ProbeType::Distro,
        ProbeType::Uptime,
        ProbeType::Packages,
        ProbeType::Shell,
        ProbeType::Editor,
        ProbeType::Resolution,
        ProbeType::DE,
        ProbeType::WM,
        ProbeType::WMTheme,
        ProbeType::Theme,
        ProbeType::Icons,
        ProbeType::Cursor,
        ProbeType::Terminal,
        ProbeType::TerminalFont,
        ProbeType::CPU,
        ProbeType::GPU,
        ProbeType::Memory,
        ProbeType::Network,
        ProbeType::Bluetooth,
        ProbeType::BIOS,
        ProbeType::GPUDriver,
        ProbeType::CPUUsage,
        ProbeType::Disk,
        ProbeType::Battery,
        ProbeType::PowerAdapter,
        ProbeType::Font,
        ProbeType::Song,
        ProbeType::LocalIP,
        ProbeType::PublicIP,
        ProbeType::Users,
        ProbeType::Locale,
        ProbeType::Java,
        ProbeType::Python,
        ProbeType::Node,
        ProbeType::Rust,
    ];

    /// Format `preset`'s values through a normalized neofetch-default config.
    fn render_default(preset: &DemoPreset) -> Vec<String> {
        let mut probes = ProbeConfig::default_neofetch();
        normalize_probes(&mut probes);
        probes
            .iter()
            .filter_map(|p| match preset.value(p.probe_type()) {
                Ok(ProbeResultValue::Single(v)) => Some(p.format_value(&v)),
                Ok(ProbeResultValue::Multiple(vs)) => Some(
                    vs.iter()
                        .map(|v| p.format_value(v))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(_) => None,
            })
            .collect()
    }

    #[test]
    fn every_preset_is_total_over_probe_types() {
        for id in DemoPresetId::ALL {
            let preset = id.preset();
            for t in ALL_PROBE_TYPES {
                match preset.value(t) {
                    Ok(_) | Err(ProbeError::MetricsUnavailable) => {}
                    Err(e) => panic!("{id:?}/{t:?}: unexpected error {e}"),
                }
            }
        }
    }

    #[test]
    fn every_preset_covers_the_core_probes() {
        const CORE: [ProbeType; 9] = [
            ProbeType::Host,
            ProbeType::OS,
            ProbeType::Kernel,
            ProbeType::Uptime,
            ProbeType::Packages,
            ProbeType::Shell,
            ProbeType::CPU,
            ProbeType::GPU,
            ProbeType::Memory,
        ];
        for id in DemoPresetId::ALL {
            let preset = id.preset();
            for t in CORE {
                assert!(
                    preset.value(t).is_ok(),
                    "{id:?} is missing core probe {t:?}"
                );
            }
        }
    }

    #[test]
    fn hero_preset_covers_the_full_neofetch_default() {
        for p in ProbeConfig::default_neofetch() {
            assert!(
                FEDORA_DESKTOP.value(p.probe_type()).is_ok(),
                "hero preset is missing {} (would leave a hole in the README render)",
                p.label()
            );
        }
    }

    #[test]
    fn demo_formatting_is_deterministic_and_env_free() {
        for id in DemoPresetId::ALL {
            let preset = id.preset();
            assert_eq!(render_default(preset), render_default(preset), "{id:?}");

            // The normalized CPU line must be exactly the baked string: no
            // live core count or clock appended at format time.
            let mut cpu = ProbeConfig::default_neofetch()
                .into_iter()
                .find(|p| matches!(p, ProbeConfig::CPU(_)))
                .unwrap();
            normalize_probes(std::slice::from_mut(&mut cpu));
            let Ok(ProbeResultValue::Single(v)) = preset.value(ProbeType::CPU) else {
                panic!("{id:?}: CPU must be a single value");
            };
            assert_eq!(cpu.format_value(&v), preset.cpu, "{id:?}");
        }
    }
}
