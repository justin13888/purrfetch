//! ASCII art module for distro logos.
//!
//! This module provides access to ASCII art logos for various distributions.
//! The art is generated at build time from text files in `ascii/distros/`.

// Include the generated ASCII art module
include!(concat!(env!("OUT_DIR"), "/ascii_art.rs"));

pub mod colors;

/// Primary colour for a distro logo.
///
/// Used to tint the title (username@hostname) and underline. The logo art
/// itself is coloured via the `${c1}`..`${c6}` palette expanded in `build.rs`.
#[allow(clippy::if_same_then_else)]
pub fn get_distro_color(distro: &str) -> crossterm::style::Color {
    use crossterm::style::Color;
    let d = distro.to_lowercase();
    if d.contains("ubuntu") {
        Color::AnsiValue(208) // Orange
    } else if d.contains("arch") {
        Color::DarkCyan
    } else if d.contains("debian") {
        Color::DarkRed
    } else if d.contains("fedora") {
        Color::AnsiValue(12) // Bright blue, matches neofetch's Fedora logo
    } else if d.contains("rhel") || d.contains("rocky") {
        Color::DarkBlue
    } else if d.contains("nixos") || d.contains("pop") {
        Color::DarkCyan
    } else if d.contains("macos") || d.contains("darwin") {
        Color::DarkGreen // neofetch c1 = 2
    } else if d.contains("windows") {
        Color::DarkCyan
    } else if d.contains("manjaro")
        || d.contains("opensuse")
        || d.contains("suse")
        || d.contains("mint")
        || d.contains("void")
    {
        Color::DarkGreen
    } else if d.contains("vylen") {
        Color::Green
    } else if d.contains("gentoo") {
        Color::DarkMagenta
    } else if d.contains("alpine") {
        Color::DarkBlue
    } else if d.contains("freebsd") {
        Color::DarkRed // neofetch c1 = 1
    } else if d.contains("netbsd") {
        Color::DarkMagenta // neofetch c1 = 5
    } else if d.contains("openbsd") {
        Color::DarkYellow // neofetch c1 = 3
    } else if d.contains("endeavour") || d.contains("kali") {
        Color::DarkMagenta
    } else if d.contains("kde") || d.contains("plasma") {
        Color::Blue
    } else {
        Color::DarkBlue // Default
    }
}

/// Generate a filler line with the same width as the ASCII art.
pub fn get_filler(width: usize) -> String {
    " ".repeat(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ascii_art_ubuntu() {
        let (art, width, _palette) = get_ascii_art("Ubuntu");
        assert!(!art.is_empty());
        assert!(width > 0);
    }

    #[test]
    fn test_get_ascii_art_fallback() {
        let (art, width, _palette) = get_ascii_art("unknown_distro_xyz");
        assert!(!art.is_empty());
        assert!(width > 0);
    }

    #[test]
    fn test_get_distro_color() {
        use crossterm::style::Color;
        assert!(matches!(get_distro_color("Ubuntu"), Color::AnsiValue(208)));
        assert!(matches!(get_distro_color("Arch Linux"), Color::DarkCyan));
    }
}
