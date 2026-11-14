use anyhow::{Context, Result};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SizeThreshold {
    pub min_size: i64,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Theme {
    pub header: Style,
    pub footer: Style,
    pub selected: Style,
    pub dir: Style,
    pub file: Style,
    pub graph: Style,
    pub percent: Style,
    pub shortcut: Style,
    pub border: Style,
    pub dialog_bg: Color,
    pub flag: Style,
    pub size_thresholds: Vec<SizeThreshold>,
}

impl Theme {
    /// Returns the style for a file/directory size based on configured
    /// thresholds (sorted descending by `min_size`), or `fallback` if none match.
    pub fn size_style(&self, size: i64, fallback: Style) -> Style {
        for threshold in &self.size_thresholds {
            if size >= threshold.min_size {
                return threshold.style;
            }
        }
        fallback
    }
}

pub fn get_theme(name: &str) -> Theme {
    match name {
        "dark" => Theme {
            header: Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            footer: Style::default().bg(Color::Blue).fg(Color::White),
            selected: Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            dir: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            file: Style::default().fg(Color::White),
            graph: Style::default().fg(Color::Green),
            percent: Style::default(),
            shortcut: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::Gray),
            dialog_bg: Color::Black,
            flag: Style::default(),
            size_thresholds: Vec::new(),
        },
        "dark-bg" => Theme {
            header: Style::default()
                .bg(Color::Black)
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            footer: Style::default().bg(Color::Black).fg(Color::Cyan),
            selected: Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            dir: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            file: Style::default().fg(Color::Gray),
            graph: Style::default().fg(Color::Cyan),
            percent: Style::default(),
            shortcut: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::DarkGray),
            dialog_bg: Color::Black,
            flag: Style::default(),
            size_thresholds: Vec::new(),
        },
        _ => {
            // off (default monochrome style)
            Theme {
                header: Style::default().add_modifier(Modifier::REVERSED),
                footer: Style::default().add_modifier(Modifier::REVERSED),
                selected: Style::default().add_modifier(Modifier::REVERSED),
                dir: Style::default().add_modifier(Modifier::BOLD),
                file: Style::default(),
                graph: Style::default(),
                percent: Style::default(),
                shortcut: Style::default().add_modifier(Modifier::UNDERLINED),
                border: Style::default(),
                dialog_bg: Color::Black,
                flag: Style::default(),
                size_thresholds: Vec::new(),
            }
        }
    }
}

/// Resolves the effective `Theme` by combining the CLI `--color` scheme with
/// optional `[color]` overrides from `config.toml`.
pub fn resolve_theme(cli_color: &str, config: Option<&ThemeConfig>) -> Theme {
    let base_name = if cli_color != "off" {
        cli_color
    } else {
        config
            .and_then(|c| c.preset.as_deref())
            .unwrap_or(cli_color)
    };
    let mut theme = get_theme(base_name);
    if let Some(cfg) = config {
        cfg.apply_to(&mut theme);
    }
    theme
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct TomlConfig {
    pub color: Option<ThemeConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub preset: Option<String>,
    pub header: Option<StyleSpec>,
    pub footer: Option<StyleSpec>,
    pub selected: Option<StyleSpec>,
    pub dir: Option<StyleSpec>,
    pub file: Option<StyleSpec>,
    pub graph: Option<StyleSpec>,
    pub percent: Option<StyleSpec>,
    pub shortcut: Option<StyleSpec>,
    pub border: Option<StyleSpec>,
    pub dialog_bg: Option<ColorSpec>,
    pub flag: Option<StyleSpec>,
    pub size_thresholds: Vec<SizeThresholdSpec>,
}

impl ThemeConfig {
    pub fn from_toml_str(input: &str) -> Result<Option<Self>> {
        let parsed: TomlConfig =
            toml::from_str(input).context("Failed to parse TOML configuration")?;
        Ok(parsed.color)
    }

    pub fn load_from_path(path: &Path) -> Result<Option<Self>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file {:?}", path))?;
        Self::from_toml_str(&content)
            .with_context(|| format!("Invalid color configuration in {:?}", path))
    }

    pub fn apply_to(&self, theme: &mut Theme) {
        if let Some(ref s) = self.header {
            theme.header = s.apply_to(theme.header);
        }
        if let Some(ref s) = self.footer {
            theme.footer = s.apply_to(theme.footer);
        }
        if let Some(ref s) = self.selected {
            theme.selected = s.apply_to(theme.selected);
        }
        if let Some(ref s) = self.dir {
            theme.dir = s.apply_to(theme.dir);
        }
        if let Some(ref s) = self.file {
            theme.file = s.apply_to(theme.file);
        }
        if let Some(ref s) = self.graph {
            theme.graph = s.apply_to(theme.graph);
        }
        if let Some(ref s) = self.percent {
            theme.percent = s.apply_to(theme.percent);
        }
        if let Some(ref s) = self.shortcut {
            theme.shortcut = s.apply_to(theme.shortcut);
        }
        if let Some(ref s) = self.border {
            theme.border = s.apply_to(theme.border);
        }
        if let Some(c) = self.dialog_bg {
            theme.dialog_bg = c.0;
        }
        if let Some(ref s) = self.flag {
            theme.flag = s.apply_to(theme.flag);
        }
        if !self.size_thresholds.is_empty() {
            let mut thresholds: Vec<SizeThreshold> = self
                .size_thresholds
                .iter()
                .map(|t| t.to_threshold())
                .collect();
            thresholds.sort_by_key(|a| std::cmp::Reverse(a.min_size));
            theme.size_thresholds = thresholds;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum StyleSpec {
    ColorOnly(ColorSpec),
    Table {
        fg: Option<ColorSpec>,
        bg: Option<ColorSpec>,
        modifiers: Option<Vec<ModifierSpec>>,
    },
}

impl StyleSpec {
    pub fn apply_to(&self, base: Style) -> Style {
        match self {
            StyleSpec::ColorOnly(fg) => base.fg(fg.0),
            StyleSpec::Table { fg, bg, modifiers } => {
                let mut style = base;
                if let Some(fg) = fg {
                    style = style.fg(fg.0);
                }
                if let Some(bg) = bg {
                    style = style.bg(bg.0);
                }
                if let Some(mods) = modifiers {
                    style.add_modifier = Modifier::empty();
                    for m in mods {
                        style = style.add_modifier(m.0);
                    }
                }
                style
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct SizeThresholdSpec {
    pub min_size: SizeSpec,
    pub style: Option<StyleSpec>,
    pub fg: Option<ColorSpec>,
    pub bg: Option<ColorSpec>,
    pub modifiers: Option<Vec<ModifierSpec>>,
}

impl SizeThresholdSpec {
    pub fn to_threshold(&self) -> SizeThreshold {
        let mut style = self
            .style
            .as_ref()
            .map(|s| s.apply_to(Style::default()))
            .unwrap_or_default();
        if let Some(fg) = self.fg {
            style = style.fg(fg.0);
        }
        if let Some(bg) = self.bg {
            style = style.bg(bg.0);
        }
        if let Some(ref mods) = self.modifiers {
            style.add_modifier = Modifier::empty();
            for m in mods {
                style = style.add_modifier(m.0);
            }
        }
        SizeThreshold {
            min_size: self.min_size.0,
            style,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorSpec(pub Color);

impl<'de> Deserialize<'de> for ColorSpec {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawColor {
            Str(String),
            Index(u8),
            Rgb([u8; 3]),
        }

        match RawColor::deserialize(deserializer)? {
            RawColor::Str(s) => parse_color_str(&s)
                .map(ColorSpec)
                .map_err(serde::de::Error::custom),
            RawColor::Index(idx) => Ok(ColorSpec(Color::Indexed(idx))),
            RawColor::Rgb([r, g, b]) => Ok(ColorSpec(Color::Rgb(r, g, b))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModifierSpec(pub Modifier);

impl<'de> Deserialize<'de> for ModifierSpec {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_modifier_str(&s)
            .map(ModifierSpec)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeSpec(pub i64);

impl<'de> Deserialize<'de> for SizeSpec {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawSize {
            Int(i64),
            Str(String),
        }

        match RawSize::deserialize(deserializer)? {
            RawSize::Int(n) if n >= 0 => Ok(SizeSpec(n)),
            RawSize::Int(n) => Err(serde::de::Error::custom(format!(
                "size threshold must be non-negative, got {}",
                n
            ))),
            RawSize::Str(s) => parse_size_str(&s)
                .map(SizeSpec)
                .map_err(serde::de::Error::custom),
        }
    }
}

fn parse_color_str(raw: &str) -> std::result::Result<Color, String> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("invalid hex color '{}'", raw))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("invalid hex color '{}'", raw))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("invalid hex color '{}'", raw))?;
                Ok(Color::Rgb(r, g, b))
            }
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16)
                    .map_err(|_| format!("invalid hex color '{}'", raw))?;
                let g = u8::from_str_radix(&hex[1..2], 16)
                    .map_err(|_| format!("invalid hex color '{}'", raw))?;
                let b = u8::from_str_radix(&hex[2..3], 16)
                    .map_err(|_| format!("invalid hex color '{}'", raw))?;
                Ok(Color::Rgb(r * 17, g * 17, b * 17))
            }
            _ => Err(format!(
                "invalid hex color '{}': expected #RGB or #RRGGBB",
                raw
            )),
        };
    }

    let lower = s.to_ascii_lowercase();
    if let Some(inner) = lower.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() != 3 {
            return Err(format!("invalid rgb() color '{}'", raw));
        }
        let r: u8 = parts[0]
            .parse()
            .map_err(|_| format!("invalid red component in '{}'", raw))?;
        let g: u8 = parts[1]
            .parse()
            .map_err(|_| format!("invalid green component in '{}'", raw))?;
        let b: u8 = parts[2]
            .parse()
            .map_err(|_| format!("invalid blue component in '{}'", raw))?;
        return Ok(Color::Rgb(r, g, b));
    }

    if let Ok(idx) = lower.parse::<u8>() {
        return Ok(Color::Indexed(idx));
    }

    let norm = lower.replace(['-', ' '], "_");
    match norm.as_str() {
        "reset" | "default" => Ok(Color::Reset),
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "dark_gray" | "dark_grey" | "darkgray" | "darkgrey" => Ok(Color::DarkGray),
        "light_red" | "bright_red" => Ok(Color::LightRed),
        "light_green" | "bright_green" => Ok(Color::LightGreen),
        "light_yellow" | "bright_yellow" => Ok(Color::LightYellow),
        "light_blue" | "bright_blue" => Ok(Color::LightBlue),
        "light_magenta" | "bright_magenta" => Ok(Color::LightMagenta),
        "light_cyan" | "bright_cyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        _ => Err(format!("unknown color '{}'", raw)),
    }
}

fn parse_modifier_str(raw: &str) -> std::result::Result<Modifier, String> {
    let norm = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match norm.as_str() {
        "bold" => Ok(Modifier::BOLD),
        "dim" => Ok(Modifier::DIM),
        "italic" => Ok(Modifier::ITALIC),
        "underlined" | "underline" => Ok(Modifier::UNDERLINED),
        "slow_blink" => Ok(Modifier::SLOW_BLINK),
        "rapid_blink" => Ok(Modifier::RAPID_BLINK),
        "reversed" | "reverse" => Ok(Modifier::REVERSED),
        "hidden" => Ok(Modifier::HIDDEN),
        "crossed_out" | "strikethrough" => Ok(Modifier::CROSSED_OUT),
        _ => Err(format!("unknown style modifier '{}'", raw)),
    }
}

fn parse_size_str(raw: &str) -> std::result::Result<i64, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("empty size threshold".to_string());
    }
    let split_idx = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let (num_part, unit_part) = s.split_at(split_idx);
    let num: f64 = num_part
        .parse()
        .map_err(|_| format!("invalid size number in '{}'", raw))?;
    if !num.is_finite() || num < 0.0 {
        return Err(format!("size threshold must be non-negative: '{}'", raw));
    }
    let mult: i64 = match unit_part.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kib" => 1024,
        "kb" => 1000,
        "m" | "mib" => 1024 * 1024,
        "mb" => 1_000_000,
        "g" | "gib" => 1024 * 1024 * 1024,
        "gb" => 1_000_000_000,
        "t" | "tib" => 1024_i64.pow(4),
        "tb" => 1000_i64.pow(4),
        other => return Err(format!("unknown size unit '{}' in '{}'", other, raw)),
    };
    let bytes = (num * mult as f64).round();
    if bytes > i64::MAX as f64 {
        return Err(format!("size threshold overflow in '{}'", raw));
    }
    Ok(bytes as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_variants() {
        assert_eq!(parse_color_str("cyan").unwrap(), Color::Cyan);
        assert_eq!(parse_color_str("bright-blue").unwrap(), Color::LightBlue);
        assert_eq!(parse_color_str("dark_grey").unwrap(), Color::DarkGray);
        assert_eq!(parse_color_str("#ff8800").unwrap(), Color::Rgb(255, 136, 0));
        assert_eq!(parse_color_str("#f80").unwrap(), Color::Rgb(255, 136, 0));
        assert_eq!(
            parse_color_str("rgb(10, 20, 30)").unwrap(),
            Color::Rgb(10, 20, 30)
        );
        assert_eq!(parse_color_str("240").unwrap(), Color::Indexed(240));
    }

    #[test]
    fn test_parse_size_units() {
        assert_eq!(parse_size_str("0").unwrap(), 0);
        assert_eq!(parse_size_str("512B").unwrap(), 512);
        assert_eq!(parse_size_str("1KiB").unwrap(), 1024);
        assert_eq!(parse_size_str("1KB").unwrap(), 1000);
        assert_eq!(parse_size_str("1.5MiB").unwrap(), 1_572_864);
        assert_eq!(parse_size_str("2G").unwrap(), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_toml_color_config_and_thresholds() {
        let toml_str = r##"
            [color]
            preset = "dark"
            header = { fg = "#ffffff", bg = [30, 58, 138], modifiers = ["bold"] }
            file = "light_gray"
            graph = 42
            dialog_bg = "#111111"

            [[color.size_thresholds]]
            min_size = "100MiB"
            style = "yellow"

            [[color.size_thresholds]]
            min_size = "1GiB"
            fg = "red"
            modifiers = ["bold"]
        "##;

        // "light_gray" is invalid; verify error, then test valid config
        assert!(ThemeConfig::from_toml_str(toml_str).is_err());

        let valid_toml = toml_str.replace("light_gray", "gray");
        let cfg = ThemeConfig::from_toml_str(&valid_toml)
            .expect("valid TOML")
            .expect("color section present");

        let theme = resolve_theme("off", Some(&cfg));
        assert_eq!(theme.header.fg, Some(Color::Rgb(255, 255, 255)));
        assert_eq!(theme.header.bg, Some(Color::Rgb(30, 58, 138)));
        assert!(theme.header.add_modifier.contains(Modifier::BOLD));
        assert_eq!(theme.file.fg, Some(Color::Gray));
        assert_eq!(theme.graph.fg, Some(Color::Indexed(42)));
        assert_eq!(theme.dialog_bg, Color::Rgb(17, 17, 17));
        // Base preset "dark" should be preserved for un-overridden fields
        assert_eq!(theme.footer.bg, Some(Color::Blue));

        // Thresholds should be sorted descending (1GiB before 100MiB)
        let gib = 1024 * 1024 * 1024;
        let mib100 = 100 * 1024 * 1024;
        assert_eq!(
            theme.size_style(2 * gib, Style::default()).fg,
            Some(Color::Red)
        );
        assert_eq!(
            theme.size_style(200 * 1024 * 1024, Style::default()).fg,
            Some(Color::Yellow)
        );
        assert_eq!(
            theme.size_style(mib100 - 1, Style::default()),
            Style::default()
        );
    }
}
