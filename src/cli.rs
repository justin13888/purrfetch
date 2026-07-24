//! Command-line interface definition.
//!
//! Lives in the library (rather than `main.rs`) so it has a single home that
//! both the binary and the `gen-man` example can share: the example renders
//! this exact `clap` command into `man/purr.1`, which keeps the man page from
//! ever drifting from `purr --help`.

use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "purr", version = crate::version::LONG_VERSION, about, long_about = None)]
#[clap(group = ArgGroup::new("renderer").multiple(false).required(false))]
pub struct Cli {
    /// Include verbose output or not.
    #[clap(long, global = true, default_value = "false")]
    pub verbose: bool,

    /// Path to a custom config file.
    #[clap(short, long)]
    pub config: Option<PathBuf>,
    /// Ignore any config file and start from the built-in defaults.
    #[clap(long)]
    pub no_config: bool,
    /// Use the all-probes preset.
    #[clap(long)]
    pub all: bool,
    #[cfg(feature = "example")]
    /// Render curated example data for a preset instead of live system info.
    #[clap(
        long,
        value_name = "PRESET",
        value_enum,
        num_args = 0..=1,
        default_missing_value = "fedora-desktop"
    )]
    pub example: Option<crate::demo::DemoPresetId>,

    /// Use the neofetch text renderer.
    #[clap(short, long, group = "renderer")]
    pub neofetch: bool,
    /// Emit JSON instead of text.
    #[clap(long, group = "renderer")]
    pub json: bool,

    // ── Logo ──
    /// Force a specific distro logo (e.g. "arch").
    #[clap(long = "ascii_distro", value_name = "DISTRO")]
    pub ascii_distro: Option<String>,
    /// Override logo colours (space/comma list, e.g. "4 6 1").
    #[clap(long = "ascii_colors", value_name = "LIST")]
    pub ascii_colors: Option<String>,
    /// Don't bold the logo.
    #[clap(long = "no_ascii_bold")]
    pub no_ascii_bold: bool,
    /// Show only the logo (no info).
    #[clap(short = 'L', long)]
    pub logo: bool,
    /// Hide the logo.
    #[clap(long)]
    pub off: bool,
    /// Logo backend: ascii or kitty.
    #[clap(long, value_name = "BACKEND")]
    pub backend: Option<String>,
    /// Image source (PNG) for the kitty backend.
    #[clap(long, value_name = "PATH")]
    pub source: Option<PathBuf>,

    // ── Text ──
    /// Separator between labels and values.
    #[clap(long, value_name = "STR")]
    pub separator: Option<String>,
    /// Don't bold the title and labels.
    #[clap(long = "no_bold")]
    pub no_bold: bool,
    /// Character used for the title underline.
    #[clap(long = "underline_char", value_name = "CHAR")]
    pub underline_char: Option<String>,
    /// Show the fully-qualified hostname.
    #[clap(long = "title_fqdn")]
    pub title_fqdn: bool,
    /// Override text colours (space/comma list).
    #[clap(long, value_name = "LIST")]
    pub colors: Option<String>,
    /// Pipe-friendly output: disable colour.
    #[clap(long)]
    pub stdout: bool,

    // ── Per-field ──
    /// Memory unit: kib, mib, or gib.
    #[clap(long = "memory_unit", value_name = "UNIT")]
    pub memory_unit: Option<String>,
    /// Uptime format: on, tiny, or off.
    #[clap(long = "uptime_shorthand", value_name = "MODE")]
    pub uptime_shorthand: Option<String>,
    /// CPU cores: logical, physical, or off.
    #[clap(long = "cpu_cores", value_name = "MODE")]
    pub cpu_cores: Option<String>,

    // Command subcommands
    #[clap(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
#[clap(group = ArgGroup::new("preset").multiple(false).required(false))]
pub enum Command {
    /// Generate a new config file
    Generate(GenerateCommandArgs),
    /// Return default config file path
    ConfigPath,
}

#[derive(Parser, Debug)]
pub struct GenerateCommandArgs {
    /// Generate neofetch preset.
    #[clap(short, long, group = "preset")]
    pub neofetch: bool,

    /// Use all default presets.
    #[clap(long)]
    pub all: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "example")]
    use crate::demo::DemoPresetId;

    #[cfg(feature = "example")]
    #[test]
    fn example_flag_parses() {
        let cli = Cli::try_parse_from(["purr", "--example", "arch"]).unwrap();
        assert_eq!(cli.example, Some(DemoPresetId::Arch));

        // Bare `--example` falls back to the hero preset.
        let cli = Cli::try_parse_from(["purr", "--example"]).unwrap();
        assert_eq!(cli.example, Some(DemoPresetId::FedoraDesktop));

        let cli = Cli::try_parse_from(["purr"]).unwrap();
        assert_eq!(cli.example, None);
    }

    #[cfg(not(feature = "example"))]
    #[test]
    fn example_flag_is_not_available_by_default() {
        assert!(Cli::try_parse_from(["purr", "--example"]).is_err());
    }
}
