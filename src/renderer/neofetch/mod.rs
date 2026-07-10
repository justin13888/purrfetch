use std::io::{IsTerminal, Write};

use crossterm::{
    cursor, execute, queue,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal,
};
use tracing::debug;

use crate::{
    ascii::{get_ascii_art, get_distro_color, get_filler},
    config::{Backend, NeofetchRendererConfig},
    demo::DemoPreset,
    probe::{ProbeList, ProbeResultValue, general_readout},
};

use super::{RendererError, execute_probes_streaming};

pub struct NeofetchRenderer {
    config: NeofetchRendererConfig,
    probe_list: ProbeList,
    /// `Some` when rendering curated example data (`--example`) — the probe
    /// list is canned and the title/logo identity comes from the preset.
    demo: Option<&'static DemoPreset>,
}

impl Default for NeofetchRenderer {
    fn default() -> Self {
        Self::new(NeofetchRendererConfig::default())
    }
}

/// Resolved text colours for the six neofetch slots.
struct ResolvedColors {
    title: Color,
    at: Color,
    underline: Color,
    subtitle: Color,
    colon: Color,
    info: Color,
}

/// Resolve the configured `colors` slots against the distro logo `palette` and
/// its `primary` tint. Empty `colors` reproduces neofetch's `set_text_colors`
/// distro defaults: the title in the logo colour (c1), the subtitle labels in
/// the logo's second colour (c2), and the `@`, underline, colon and values all
/// in the terminal's default foreground.
fn resolve_colors(colors: &[u8], palette: &[u8; 6], primary: Color) -> ResolvedColors {
    if colors.is_empty() {
        // neofetch sets subtitle=color(c2), but with c2==7 -> c1 (primary) and
        // c2==8 -> reset; everything besides title and subtitle is terminal fg.
        let subtitle = match palette[1] {
            7 => primary,
            8 => Color::Reset,
            c => Color::AnsiValue(c),
        };
        return ResolvedColors {
            title: primary,
            at: Color::Reset,
            underline: Color::Reset,
            subtitle,
            colon: Color::Reset,
            info: Color::Reset,
        };
    }
    let slot = |i: usize| {
        colors
            .get(i)
            .map(|&c| Color::AnsiValue(c))
            .unwrap_or(primary)
    };
    ResolvedColors {
        title: slot(0),
        at: slot(1),
        underline: slot(2),
        subtitle: slot(3),
        colon: slot(4),
        info: slot(5),
    }
}

/// Apply neofetch's `title_fqdn` semantics: off (default) strips the domain
/// (`host.example.com` → `host`, matching neofetch's `${hostname/.*}` — on
/// macOS this drops the `.local` suffix), on keeps the hostname as reported.
fn title_hostname(hostname: String, fqdn: bool) -> String {
    if !fqdn && let Some((short, _)) = hostname.split_once('.') {
        return short.to_string();
    }
    hostname
}

impl NeofetchRenderer {
    pub fn new(config: NeofetchRendererConfig) -> Self {
        let probe_list = config
            .probes
            .iter()
            .map(|p| p.get_funcs())
            .collect::<Vec<_>>();
        Self {
            config,
            probe_list,
            demo: None,
        }
    }

    /// Render `preset`'s curated data instead of live probes. The preset picks
    /// the logo unless the config (e.g. `--ascii_distro`) already forces one.
    pub fn new_demo(mut config: NeofetchRendererConfig, preset: &'static DemoPreset) -> Self {
        crate::demo::normalize_probes(&mut config.probes);
        if config.ascii.distro.is_none() {
            config.ascii.distro = Some(preset.distro.to_string());
        }
        let probe_list = config
            .probes
            .iter()
            .map(|p| (p.label().to_string(), preset.probe_fn(p.probe_type())))
            .collect();
        Self {
            config,
            probe_list,
            demo: Some(preset),
        }
    }

    /// Resolve the `username@hostname` shown in the title, honouring
    /// `title_fqdn`. Single source of truth for the ASCII and image paths.
    fn title_identity(&self) -> Result<(String, String), RendererError> {
        if let Some(preset) = self.demo {
            return Ok((preset.username.to_string(), preset.hostname.to_string()));
        }
        use libmacchina::traits::GeneralReadout as _;
        let username = general_readout().username()?;
        let hostname = title_hostname(general_readout().hostname()?, self.config.title_fqdn);
        Ok((username, hostname))
    }

    /// The distro whose logo (and tint) is shown: an explicit `ascii_distro`
    /// wins, then the demo preset, then live detection.
    fn logo_distro(&self) -> String {
        if let Some(distro) = &self.config.ascii.distro {
            return distro.clone();
        }
        if let Some(preset) = self.demo {
            return preset.distro.to_string();
        }
        use libmacchina::traits::GeneralReadout as _;
        general_readout()
            .distribution()
            .or_else(|_| general_readout().os_name())
            .unwrap_or_else(|_| "Linux".to_string())
    }

    /// Write `text` in `color`, optionally bold, then reset. ANSI is zero-width
    /// so this never affects the renderer's column math.
    fn put<W: Write>(w: &mut W, color: Color, bold: bool, text: &str) -> std::io::Result<()> {
        // Honour the NO_COLOR convention (set by `--stdout`).
        if std::env::var_os("NO_COLOR").is_some() {
            return queue!(w, Print(text));
        }
        if bold {
            queue!(
                w,
                SetAttribute(Attribute::Bold),
                SetForegroundColor(color),
                Print(text),
                SetAttribute(Attribute::Reset),
            )
        } else {
            queue!(w, SetForegroundColor(color), Print(text), ResetColor)
        }
    }

    pub fn draw(&self) -> Result<(), RendererError> {
        // The Kitty image backend short-circuits the ASCII renderer when it
        // applies; otherwise it returns false and we fall through to ASCII.
        if self.draw_image()? {
            return Ok(());
        }

        let distro = self.logo_distro();
        debug!("Logo distro: {}", distro);

        let (full_art, full_width, base_palette) = get_ascii_art(&distro);
        // `backend = off` (neofetch --off) drops the logo entirely.
        let (ascii_art, ascii_width): (&[&str], usize) = if self.config.backend == Backend::Off {
            (&[], 0)
        } else {
            (full_art, full_width)
        };
        let primary_color = get_distro_color(&distro);
        // `ascii_colors` overrides the logo palette (padded with the logo's own).
        let palette: [u8; 6] = {
            let mut p = base_palette;
            for (i, &c) in self.config.ascii.colors.iter().take(6).enumerate() {
                p[i] = c;
            }
            p
        };
        let ascii_bold = self.config.ascii.bold;
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let filler = get_filler(ascii_width);
        // Expand `${cN}` markers at render time (or strip them under NO_COLOR).
        let get_art = |idx: usize| -> String {
            let raw = ascii_art.get(idx).copied().unwrap_or(filler.as_str());
            if no_color {
                crate::ascii::colors::strip(raw)
            } else {
                crate::ascii::colors::expand(raw, &palette, ascii_bold)
            }
        };

        let colors = resolve_colors(&self.config.colors, &palette, primary_color);
        let bold = self.config.bold;
        let sep = self.config.separator.as_str();
        let sep_width = sep.chars().count();

        let stdout = std::io::stdout();
        let is_tty = stdout.is_terminal();
        let mut w = std::io::BufWriter::new(stdout.lock());

        let mut art_idx = 0usize;
        let mut title_len = 0;

        // Print title (username@hostname)
        if self.config.title {
            let (username, hostname) = self.title_identity()?;
            title_len = username.chars().count() + hostname.chars().count() + 1;
            Self::put(&mut w, primary_color, false, &get_art(art_idx))?;
            queue!(w, Print("   "))?;
            Self::put(&mut w, colors.title, bold, &username)?;
            Self::put(&mut w, colors.at, bold, "@")?;
            Self::put(&mut w, colors.title, bold, &hostname)?;
            queue!(w, Print("\n"))?;
            art_idx += 1;
        }

        // Print underline
        if self.config.underline {
            Self::put(&mut w, primary_color, false, &get_art(art_idx))?;
            queue!(w, Print("   "))?;
            Self::put(
                &mut w,
                colors.underline,
                false,
                &self.config.underline_char.repeat(title_len),
            )?;
            queue!(w, Print("\n"))?;
            art_idx += 1;
        }

        let probe_art_start = art_idx;
        let n_probes = self.probe_list.len();
        // 0-based column where a value starts: ascii + "   " + label + sep + " ".
        // Labels are not padded (neofetch puts the colon right after the label),
        // so the column depends on each probe's own label length.
        let value_col = |title_len: usize| (ascii_width + 3 + title_len + sep_width + 1) as u16;
        // Per-probe config, aligned with `probe_list` by index, for option-aware formatting.
        let probes = &self.config.probes;

        // Emit one full probe line: art, label, separator, value.
        let put_line = |w: &mut std::io::BufWriter<_>, art: &str, label: &str, value: &str| {
            Self::put(w, primary_color, false, art)?;
            queue!(w, Print("   "))?;
            Self::put(w, colors.subtitle, bold, label)?;
            Self::put(w, colors.colon, bold, sep)?;
            queue!(w, Print(" "))?;
            Self::put(w, colors.info, false, value)?;
            queue!(w, Print("\n"))
        };

        if !is_tty {
            // Non-TTY: run probes in parallel, print results sequentially
            let mut all_results: Vec<Option<Vec<String>>> = vec![None; n_probes];
            execute_probes_streaming(&self.probe_list, |index, _, result| {
                all_results[index] = Some(match result {
                    Some(ProbeResultValue::Single(v)) => vec![probes[index].format_value(&v)],
                    Some(ProbeResultValue::Multiple(vs)) => {
                        vs.iter().map(|v| probes[index].format_value(v)).collect()
                    }
                    None => vec![],
                });
            });

            for (i, (title, _)) in self.probe_list.iter().enumerate() {
                let strings = match all_results[i].as_deref() {
                    Some([]) | None => {
                        debug!("Error while probing {}", title);
                        continue;
                    }
                    Some(ss) => ss.to_vec(),
                };
                for s in strings.iter() {
                    // Repeat the label on every line (e.g. one "GPU:" per GPU),
                    // matching neofetch rather than leaving orphaned values.
                    put_line(&mut w, &get_art(art_idx), title, s)?;
                    art_idx += 1;
                }
            }
        } else {
            // TTY: progressive rendering with cursor movement

            // Phase 1: print all placeholder lines immediately (label + separator).
            for (i, (title, _)) in self.probe_list.iter().enumerate() {
                Self::put(&mut w, primary_color, false, &get_art(probe_art_start + i))?;
                queue!(w, Print("   "))?;
                Self::put(&mut w, colors.subtitle, bold, title)?;
                Self::put(&mut w, colors.colon, bold, sep)?;
                queue!(w, Print(" \n"))?;
            }
            w.flush()?;
            // Save cursor position at the bottom of the probe section
            execute!(w, cursor::SavePosition)?;

            // Phase 2: fill in values as each probe completes
            let mut results: Vec<Option<Vec<String>>> = vec![None; n_probes];
            let mut needs_rerender = false;

            execute_probes_streaming(&self.probe_list, |index, _label, result| {
                let strings: Vec<String> = match result {
                    Some(ProbeResultValue::Single(v)) => vec![probes[index].format_value(&v)],
                    Some(ProbeResultValue::Multiple(vs)) => {
                        vs.iter().map(|v| probes[index].format_value(v)).collect()
                    }
                    None => vec![],
                };

                if strings.len() == 1 {
                    // Single value: move cursor to the right line and fill in,
                    // at the column just after this probe's own label.
                    let lines_up = (n_probes - index) as u16;
                    let col = value_col(self.probe_list[index].0.chars().count());
                    let _ = execute!(
                        w,
                        cursor::RestorePosition,
                        cursor::MoveUp(lines_up),
                        cursor::MoveToColumn(col),
                    );
                    let _ = Self::put(&mut w, colors.info, false, &strings[0]);
                    let _ = execute!(w, cursor::RestorePosition);
                } else {
                    // Zero (failure) or multiple values: needs a re-render pass
                    needs_rerender = true;
                }

                results[index] = Some(strings);
            });

            // Phase 3: if any probe had 0 or multiple lines, re-render the probe section
            if needs_rerender {
                execute!(
                    w,
                    cursor::RestorePosition,
                    cursor::MoveUp(n_probes as u16),
                    cursor::MoveToColumn(0),
                    terminal::Clear(terminal::ClearType::FromCursorDown),
                )?;
                let mut ra_idx = probe_art_start;
                for (i, (title, _)) in self.probe_list.iter().enumerate() {
                    let strings = match results[i].as_deref() {
                        Some([]) | None => {
                            debug!("Error while probing {}", title);
                            continue;
                        }
                        Some(ss) => ss.to_vec(),
                    };
                    for s in strings.iter() {
                        put_line(&mut w, &get_art(ra_idx), title, s)?;
                        ra_idx += 1;
                    }
                }
                art_idx = ra_idx;
                w.flush()?;
            } else {
                // Ensure cursor is at the bottom of the probe section
                execute!(w, cursor::RestorePosition)?;
                art_idx = probe_art_start + n_probes;
            }
        }

        // Print color blocks (skipped under NO_COLOR — they're meaningless without colour).
        if self.config.col && std::env::var_os("NO_COLOR").is_none() {
            let cb = &self.config.color_blocks;
            let offset = " ".repeat(cb.offset.unwrap_or(3) as usize);
            let block = " ".repeat(cb.width.max(1) as usize);
            let height = cb.height.max(1);
            let (start, end) = (cb.range[0], cb.range[1]);

            // Spacer line between probes and color blocks
            Self::put(&mut w, primary_color, false, &get_art(art_idx))?;
            queue!(w, Print("\n"))?;
            art_idx += 1;

            // Standard colours (0-7) then bright/extended colours (8+), each
            // group spanning `height` rows. The [0,15] default reproduces the
            // classic two rows of eight.
            let dark: Vec<u8> = (start..=end.min(7)).collect();
            let bright: Vec<u8> = (start.max(8)..=end).collect();
            for group in [dark, bright] {
                if group.is_empty() {
                    continue;
                }
                for _ in 0..height {
                    Self::put(&mut w, primary_color, false, &get_art(art_idx))?;
                    queue!(w, Print(&offset))?;
                    for c in &group {
                        queue!(
                            w,
                            SetBackgroundColor(Color::AnsiValue(*c)),
                            Print(&block),
                            ResetColor
                        )?;
                    }
                    queue!(w, Print("\n"))?;
                    art_idx += 1;
                }
            }
        }

        // Print remaining ASCII art lines
        for line in ascii_art.iter().skip(art_idx) {
            let art = if no_color {
                crate::ascii::colors::strip(line)
            } else {
                crate::ascii::colors::expand(line, &palette, ascii_bold)
            };
            Self::put(&mut w, primary_color, false, &art)?;
            queue!(w, Print("\n"))?;
        }

        w.flush()?;
        Ok(())
    }

    /// If the Kitty image backend applies, render the image with the info block
    /// beside it and return `true`; otherwise return `false` to fall back to
    /// ASCII. Falls back whenever the terminal isn't Kitty or the source isn't
    /// a readable PNG, so non-Kitty terminals are unaffected.
    fn draw_image(&self) -> Result<bool, RendererError> {
        use crate::renderer::image;

        if self.config.backend != Backend::Kitty {
            return Ok(false);
        }
        let Some(src) = self.config.image_source.clone() else {
            return Ok(false);
        };
        if !image::kitty_supported() {
            return Ok(false);
        }
        let Ok(png) = std::fs::read(&src) else {
            return Ok(false);
        };
        let Some((iw, ih)) = image::png_dimensions(&png) else {
            return Ok(false);
        };

        let cols = self.config.image_cols.max(1) as u32;
        // Terminal cells are ~2x taller than wide, so halve the row estimate.
        let rows = ((cols as f64 * ih as f64 / iw as f64) / 2.0)
            .round()
            .max(1.0) as u32;
        let pad = " ".repeat(cols as usize + 2);

        let lines = self.build_info_lines()?;

        let mut w = std::io::BufWriter::new(std::io::stdout().lock());
        // Print the info block left-padded to clear the image area, then draw the
        // image over that margin from the saved top-left position.
        execute!(w, cursor::SavePosition)?;
        for line in &lines {
            write!(w, "{pad}{line}\r\n")?;
        }
        execute!(w, cursor::RestorePosition)?;
        image::display_png(&mut w, &png, cols, rows)?;
        let total = lines.len().max(rows as usize) as u16;
        execute!(w, cursor::RestorePosition, cursor::MoveToNextLine(total))?;
        w.flush()?;
        Ok(true)
    }

    /// Build the plain info lines (title, underline, probe values) for the image
    /// backend, where the image rather than ASCII art occupies the left column.
    fn build_info_lines(&self) -> Result<Vec<String>, RendererError> {
        let probes = &self.config.probes;

        let mut results: Vec<Option<Vec<String>>> = vec![None; self.probe_list.len()];
        execute_probes_streaming(&self.probe_list, |index, _, result| {
            results[index] = Some(match result {
                Some(ProbeResultValue::Single(v)) => vec![probes[index].format_value(&v)],
                Some(ProbeResultValue::Multiple(vs)) => {
                    vs.iter().map(|v| probes[index].format_value(v)).collect()
                }
                None => vec![],
            });
        });

        let sep = &self.config.separator;
        let mut lines = Vec::new();
        if self.config.title {
            let (username, hostname) = self.title_identity()?;
            let title = format!("{username}@{hostname}");
            let len = title.chars().count();
            lines.push(title);
            if self.config.underline {
                lines.push(self.config.underline_char.repeat(len));
            }
        }
        for (i, p) in probes.iter().enumerate() {
            if let Some(strings) = &results[i] {
                for s in strings {
                    lines.push(format!("{}{sep} {s}", p.label()));
                }
            }
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::{NeofetchRenderer, resolve_colors, title_hostname};
    use crate::config::NeofetchRendererConfig;
    use crate::demo;
    use crossterm::style::Color;

    #[test]
    fn demo_renderer_uses_preset_identity_and_logo() {
        let r = NeofetchRenderer::new_demo(NeofetchRendererConfig::default(), &demo::ARCH);
        assert_eq!(r.logo_distro(), "Arch Linux");
        let (user, host) = r.title_identity().unwrap();
        assert_eq!((user.as_str(), host.as_str()), ("kt", "tokyo"));
    }

    #[test]
    fn ascii_distro_overrides_demo_logo() {
        let mut config = NeofetchRendererConfig::default();
        config.ascii.distro = Some("gentoo".to_string());
        let r = NeofetchRenderer::new_demo(config, &demo::ARCH);
        assert_eq!(r.logo_distro(), "gentoo");
    }

    #[test]
    fn title_fqdn_off_strips_domain() {
        assert_eq!(title_hostname("host.example.com".into(), false), "host");
        assert_eq!(title_hostname("mac.local".into(), false), "mac");
        assert_eq!(title_hostname("plain".into(), false), "plain");
    }

    #[test]
    fn title_fqdn_on_keeps_hostname_as_reported() {
        assert_eq!(
            title_hostname("host.example.com".into(), true),
            "host.example.com"
        );
        assert_eq!(title_hostname("plain".into(), true), "plain");
    }

    #[test]
    fn distro_default_colors_match_neofetch() {
        let primary = Color::AnsiValue(12);
        // Fedora-like [12, 7, …]: title=c1 tint, subtitle (c2==7) -> primary,
        // and @/underline/colon/info in the terminal's default foreground.
        let c = resolve_colors(&[], &[12, 7, 12, 12, 12, 12], primary);
        assert_eq!(c.title, primary);
        assert_eq!(c.subtitle, primary);
        assert_eq!(c.at, Color::Reset);
        assert_eq!(c.underline, Color::Reset);
        assert_eq!(c.colon, Color::Reset);
        assert_eq!(c.info, Color::Reset);

        // Distinct c2 (macOS-like [2, 3, …]) -> subtitle is that colour.
        let c = resolve_colors(&[], &[2, 3, 1, 1, 5, 4], Color::AnsiValue(2));
        assert_eq!(c.subtitle, Color::AnsiValue(3));

        // c2 == 8 -> subtitle resets to the terminal foreground.
        let c = resolve_colors(&[], &[7, 8, 3, 7, 7, 7], primary);
        assert_eq!(c.subtitle, Color::Reset);
    }

    #[test]
    fn explicit_colors_fill_all_six_slots() {
        let c = resolve_colors(&[1, 2, 3, 4, 5, 6], &[7; 6], Color::AnsiValue(12));
        assert_eq!(c.title, Color::AnsiValue(1));
        assert_eq!(c.at, Color::AnsiValue(2));
        assert_eq!(c.underline, Color::AnsiValue(3));
        assert_eq!(c.subtitle, Color::AnsiValue(4));
        assert_eq!(c.colon, Color::AnsiValue(5));
        assert_eq!(c.info, Color::AnsiValue(6));
    }
}
