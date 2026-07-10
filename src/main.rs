// #![feature(type_alias_impl_trait)]
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("This crate is only supported on Linux, macOS, and Windows.");

use clap::Parser;
use tracing::{Level, debug, info, info_span};

use purr_lib::{
    cli::{Cli, Command},
    config::{Config, RendererOverride},
    renderer::{json::JsonRenderer, neofetch::NeofetchRenderer},
};

// TODO: Include 'libmacchina' version in version command

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let mut args = Cli::parse();
    let verbose = args.verbose;

    // Initialize logger
    #[cfg(feature = "profile")]
    let _chrome_guard = {
        use tracing_chrome::ChromeLayerBuilder;
        use tracing_subscriber::prelude::*;
        let (chrome_layer, guard) = ChromeLayerBuilder::new()
            .file("purr-trace.json")
            .include_args(true)
            .build();
        tracing_subscriber::registry().with(chrome_layer).init();
        guard
    };

    #[cfg(not(feature = "profile"))]
    match verbose {
        true => {
            tracing_subscriber::fmt()
                .with_max_level(Level::TRACE)
                .init();
        }
        false => {
            tracing_subscriber::fmt::init();
        }
    }
    debug!("Args: {:?}", args);

    if let Some(command) = args.command.take() {
        match command {
            Command::Generate(args) => {
                // Generate a new config file
                debug!("Generating new config file");
                let config_dir =
                    Config::get_config_dir().expect("Could not determine config directory");
                debug!("Default config dir: {:?}", config_dir);

                // Create config directory if it doesn't exist
                if !config_dir.exists() {
                    std::fs::create_dir_all(&config_dir)?;
                    println!("Config directory created successfully");
                }

                let config_path = config_dir.join(Config::CONFIG_FILE_NAME);
                debug!("Default config path: {:?}", config_path);

                if config_path.try_exists()? {
                    println!("Config file already exists, skipping generation");
                    return Ok(());
                }

                // Determine which preset to generate
                let default_config = if args.neofetch {
                    if args.all {
                        Config::default_neofetch_all()
                    } else {
                        Config::default_neofetch()
                    }
                } else if args.all {
                    Config::default_all()
                } else {
                    Config::default()
                };

                default_config.to_file(&config_path)?;
                println!("Config file generated successfully");
                return Ok(());
            }
            Command::ConfigPath => {
                // Return default config file path
                debug!("Returning default config file path");
                let config_path = Config::get_config_dir()
                    .expect("Could not determine config directory")
                    .join(Config::CONFIG_FILE_NAME);
                println!("{}", config_path.display());
                return Ok(());
            }
        }
    }

    // Handle main command (no subcommand): resolve config, then layer flags.

    // `--stdout` disables colour for the whole run via the NO_COLOR convention.
    if args.stdout {
        // SAFETY: still single-threaded here — set before any probe threads spawn.
        unsafe { std::env::set_var("NO_COLOR", "1") };
    }

    // Renderer override from flags.
    let renderer_override = if args.json {
        Some(RendererOverride::Json)
    } else if args.neofetch {
        Some(RendererOverride::Neofetch)
    } else {
        None
    };

    // Base config: --all preset, --no-config defaults, else load the config file.
    let mut config = {
        let _span = info_span!("config_load").entered();
        if args.all {
            Config::default_all()
        } else if args.no_config {
            Config::default()
        } else {
            let config_path = args.config.clone().unwrap_or_else(|| {
                Config::get_config_dir()
                    .expect("Could not determine config directory")
                    .join(Config::CONFIG_FILE_NAME)
            });
            match config_path.try_exists() {
                Ok(true) => Config::from_file(&config_path, None)?,
                Ok(false) => Config::default(),
                Err(e) => {
                    info!("Using default config. Error checking config file: {:?}", e);
                    Config::default()
                }
            }
        }
    };

    // Precedence: defaults < config file < CLI flags.
    if let Some(target) = renderer_override {
        config = config.with_renderer(target);
    }
    apply_overrides(&mut config, &args);

    debug!("Config: {:?}", config);

    // `--example` swaps live probes for a curated preset's canned data.
    let demo = args.example.map(|id| id.preset());

    match (config, demo) {
        (Config::Neofetch(neofetch_config), demo) => {
            let renderer = {
                let _span = info_span!("renderer_init").entered();
                match demo {
                    Some(preset) => NeofetchRenderer::new_demo(neofetch_config, preset),
                    None => NeofetchRenderer::new(neofetch_config),
                }
            };
            let _span = info_span!("render").entered();
            renderer.draw()?;
        }
        (Config::Json(json_config), demo) => {
            let _span = info_span!("render").entered();
            let renderer = match demo {
                Some(preset) => JsonRenderer::new_demo(json_config, preset),
                None => JsonRenderer::new(json_config),
            };
            renderer.draw()?;
        }
    };

    Ok(())
}

/// Parse a space/comma-separated list of 0-255 values (for colour overrides).
fn parse_u8_list(s: &str) -> Vec<u8> {
    s.split([',', ' '])
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect()
}

/// Layer CLI flag overrides onto a loaded config (defaults < config < flags).
fn apply_overrides(config: &mut Config, args: &Cli) {
    use purr_lib::config::{Backend, CoresMode, MemoryUnit, ProbeConfig, UptimeFormat};

    if let Config::Neofetch(c) = config {
        if let Some(s) = &args.separator {
            c.separator = s.clone();
        }
        if args.no_bold || args.stdout {
            c.bold = false;
        }
        if let Some(u) = &args.underline_char {
            c.underline_char = u.clone();
        }
        if args.title_fqdn {
            c.title_fqdn = true;
        }
        if let Some(cl) = &args.colors {
            c.colors = parse_u8_list(cl);
        }
        if let Some(d) = &args.ascii_distro {
            c.ascii.distro = Some(d.clone());
        }
        if let Some(ac) = &args.ascii_colors {
            c.ascii.colors = parse_u8_list(ac);
        }
        if args.no_ascii_bold {
            c.ascii.bold = false;
        }
        if let Some(src) = &args.source {
            c.image_source = Some(src.clone());
            c.backend = Backend::Kitty;
        }
        if let Some(b) = &args.backend {
            c.backend = match b.as_str() {
                "kitty" => Backend::Kitty,
                "off" => Backend::Off,
                _ => Backend::Ascii,
            };
        }
        if args.off {
            c.backend = Backend::Off;
        }
        if args.logo {
            c.title = false;
            c.underline = false;
            c.col = false;
            c.probes.clear();
        }
    }

    // Per-field probe overrides apply to whichever renderer's probe list.
    let probes = config.probes_mut();
    if let Some(u) = &args.memory_unit {
        let unit = match u.as_str() {
            "kib" => MemoryUnit::Kib,
            "gib" => MemoryUnit::Gib,
            _ => MemoryUnit::Mib,
        };
        for p in probes.iter_mut() {
            if let ProbeConfig::Memory(o) = p {
                o.unit = unit;
            }
        }
    }
    if let Some(f) = &args.uptime_shorthand {
        let fmt = match f.as_str() {
            "tiny" => UptimeFormat::Tiny,
            "off" => UptimeFormat::Off,
            _ => UptimeFormat::On,
        };
        for p in probes.iter_mut() {
            if let ProbeConfig::Uptime(o) = p {
                o.format = fmt;
            }
        }
    }
    if let Some(m) = &args.cpu_cores {
        let mode = match m.as_str() {
            "physical" => CoresMode::Physical,
            "off" => CoresMode::Off,
            _ => CoresMode::Logical,
        };
        for p in probes.iter_mut() {
            if let ProbeConfig::CPU(o) = p {
                o.cores = mode;
            }
        }
    }
}
