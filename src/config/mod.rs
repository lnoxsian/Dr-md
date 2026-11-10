use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ThemeAccent {
    #[default]
    Blue,
    Purple,
    Orange,
    Red,
    Green,
    Pink,
    Teal,
    Yellow,
}


impl ThemeAccent {
    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Blue => egui::Color32::from_rgb(29, 78, 216),
            Self::Purple => egui::Color32::from_rgb(162, 122, 228),
            Self::Orange => egui::Color32::from_rgb(194, 65, 12),
            Self::Red => egui::Color32::from_rgb(185, 28, 28),
            Self::Green => egui::Color32::from_rgb(21, 128, 61),
            Self::Pink => egui::Color32::from_rgb(190, 24, 93),
            Self::Teal => egui::Color32::from_rgb(15, 118, 110),
            Self::Yellow => egui::Color32::from_rgb(161, 98, 7),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Orange => "Orange",
            Self::Red => "Red",
            Self::Green => "Green",
            Self::Pink => "Pink",
            Self::Teal => "Teal",
            Self::Yellow => "Yellow",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Blue,
            Self::Purple,
            Self::Orange,
            Self::Red,
            Self::Green,
            Self::Pink,
            Self::Teal,
            Self::Yellow,
        ]
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CursorStyle {
    #[default]
    IBeam,
    Block,
    Underline,
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ViewMode {
    Preview,
    #[default]
    #[serde(alias = "editor")]
    Split,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TabState {
    pub path: String,
    pub view_mode: ViewMode,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[derive(Default)]
pub struct FileState {
    pub last_open_tabs: Vec<TabState>,
    pub last_active_tab: Option<usize>,
}


impl FileState {
    fn get_state_path() -> Option<std::path::PathBuf> {
        #[cfg(target_os = "windows")]
        {
            directories::ProjectDirs::from("com", "drmd", "dr-md")
                .map(|proj| proj.config_dir().join("state.toml"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            directories::BaseDirs::new().map(|base| {
                base.home_dir()
                    .join(".config")
                    .join("dr-md")
                    .join("state.toml")
            })
        }
    }

    pub fn load() -> Self {
        if let Some(state_path) = Self::get_state_path() {
            if state_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&state_path)
                    && let Ok(state) = toml::from_str(&content) {
                        return state;
                    }
            } else {
                let default_state = Self::default();
                let _ = default_state.save();
                return default_state;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        if let Some(state_path) = Self::get_state_path() {
            if let Some(parent) = state_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(self)?;
            std::fs::write(state_path, content)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub theme: String,
    pub theme_accent: ThemeAccent,
    pub font_size: f32,
    pub line_numbers: bool,
    pub autosave: bool,
    pub tab_width: usize,
    pub last_opened_folder: Option<String>,
    pub cursor_style: CursorStyle,
    pub reopen_last_files: bool,
    pub mirror_mode: bool,
    pub gpu_acceleration: bool,
    pub preview_max_width: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            theme_accent: ThemeAccent::default(),
            font_size: 16.0,
            line_numbers: true,
            autosave: true,
            tab_width: 4,
            last_opened_folder: None,
            cursor_style: CursorStyle::default(),
            reopen_last_files: true,
            mirror_mode: false,
            gpu_acceleration: true,
            preview_max_width: 80.0,
        }
    }
}

fn solarized_dark_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    let bg_main = egui::Color32::from_rgb(0, 43, 54); // Base03
    let bg_alt = egui::Color32::from_rgb(7, 54, 66); // Base02
    let fg_main = egui::Color32::from_rgb(131, 148, 150); // Base0
    let fg_muted = egui::Color32::from_rgb(88, 110, 117); // Base01
    let fg_bright = egui::Color32::from_rgb(147, 161, 161); // Base1

    visuals.dark_mode = true;
    visuals.panel_fill = bg_main;
    visuals.window_fill = bg_alt;
    visuals.extreme_bg_color = bg_main;
    visuals.faint_bg_color = bg_alt;
    visuals.code_bg_color = bg_alt;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, fg_muted);

    visuals.widgets.noninteractive.bg_fill = bg_alt;
    visuals.widgets.noninteractive.weak_bg_fill = bg_main;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(20, 81, 98);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(20, 81, 98);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(10, 64, 79);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(10, 64, 79);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, fg_bright);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, fg_bright);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(12, 74, 91);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(12, 74, 91);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, fg_bright);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, fg_bright);

    visuals.widgets.open.bg_fill = bg_alt;
    visuals.widgets.open.weak_bg_fill = bg_alt;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals
}

fn solarized_light_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::light();
    let bg_main = egui::Color32::from_rgb(253, 246, 227); // Base3
    let bg_alt = egui::Color32::from_rgb(238, 232, 213); // Base2
    let fg_main = egui::Color32::from_rgb(101, 123, 131); // Base00
    let fg_muted = egui::Color32::from_rgb(147, 161, 161); // Base1
    let fg_dark = egui::Color32::from_rgb(88, 110, 117); // Base01

    visuals.dark_mode = false;
    visuals.panel_fill = bg_main;
    visuals.window_fill = bg_alt;
    visuals.extreme_bg_color = bg_main;
    visuals.faint_bg_color = bg_alt;
    visuals.code_bg_color = bg_alt;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, fg_muted);

    visuals.widgets.noninteractive.bg_fill = bg_alt;
    visuals.widgets.noninteractive.weak_bg_fill = bg_main;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(220, 214, 190);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(220, 214, 190);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(224, 217, 197);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(224, 217, 197);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, fg_dark);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, fg_dark);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(213, 206, 185);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(213, 206, 185);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, fg_dark);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, fg_dark);

    visuals.widgets.open.bg_fill = bg_alt;
    visuals.widgets.open.weak_bg_fill = bg_alt;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals
}

fn gruvbox_dark_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    let bg_main = egui::Color32::from_rgb(40, 40, 40); // bg0
    let bg_alt = egui::Color32::from_rgb(60, 56, 54); // bg1
    let bg_dark = egui::Color32::from_rgb(29, 32, 33); // bg0_hard
    let fg_main = egui::Color32::from_rgb(235, 219, 178); // fg1
    let fg_bright = egui::Color32::from_rgb(251, 241, 199); // fg0
    let fg_muted = egui::Color32::from_rgb(168, 153, 132); // fg4

    visuals.dark_mode = true;
    visuals.panel_fill = bg_main;
    visuals.window_fill = bg_alt;
    visuals.extreme_bg_color = bg_dark;
    visuals.faint_bg_color = bg_alt;
    visuals.code_bg_color = bg_alt;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, fg_muted);

    visuals.widgets.noninteractive.bg_fill = bg_alt;
    visuals.widgets.noninteractive.weak_bg_fill = bg_main;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(80, 73, 69);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(80, 73, 69);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(80, 73, 69);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(80, 73, 69);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, fg_bright);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, fg_bright);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(102, 92, 84);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(102, 92, 84);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, fg_bright);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, fg_bright);

    visuals.widgets.open.bg_fill = bg_alt;
    visuals.widgets.open.weak_bg_fill = bg_alt;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals
}

fn gruvbox_light_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::light();
    let bg_main = egui::Color32::from_rgb(251, 241, 199); // bg0 (light)
    let bg_alt = egui::Color32::from_rgb(235, 219, 178); // bg1 (light)
    let bg_light = egui::Color32::from_rgb(249, 245, 215); // bg0_hard (light)
    let fg_main = egui::Color32::from_rgb(60, 56, 54); // fg1 (light)
    let fg_dark = egui::Color32::from_rgb(40, 40, 40); // fg0 (light)
    let fg_muted = egui::Color32::from_rgb(124, 111, 100); // fg4 (light)

    visuals.dark_mode = false;
    visuals.panel_fill = bg_main;
    visuals.window_fill = bg_alt;
    visuals.extreme_bg_color = bg_light;
    visuals.faint_bg_color = bg_alt;
    visuals.code_bg_color = bg_alt;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, fg_muted);

    visuals.widgets.noninteractive.bg_fill = bg_alt;
    visuals.widgets.noninteractive.weak_bg_fill = bg_main;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(213, 196, 161);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(213, 196, 161);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(213, 196, 161);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(213, 196, 161);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, fg_dark);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, fg_dark);

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(189, 174, 147);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(189, 174, 147);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, fg_dark);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, fg_dark);

    visuals.widgets.open.bg_fill = bg_alt;
    visuals.widgets.open.weak_bg_fill = bg_alt;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, fg_muted);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, fg_main);

    visuals
}


pub fn apply_theme(ctx: &egui::Context, config: &AppConfig) {
    let mut style = (*ctx.style()).clone();

    // Set theme base mode
    style.visuals = match config.theme.as_str() {
        "solarized_dark" => solarized_dark_visuals(),
        "solarized_light" => solarized_light_visuals(),
        "gruvbox_dark" => gruvbox_dark_visuals(),
        "gruvbox_light" => gruvbox_light_visuals(),
        "light" => egui::Visuals::light(),
        _ => egui::Visuals::dark(),
    };

    // Apply styling options
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::ZERO;
    style.visuals.widgets.inactive.rounding = egui::Rounding::ZERO;
    style.visuals.widgets.hovered.rounding = egui::Rounding::ZERO;
    style.visuals.widgets.active.rounding = egui::Rounding::ZERO;
    style.visuals.widgets.open.rounding = egui::Rounding::ZERO;
    style.visuals.window_rounding = egui::Rounding::ZERO;
    style.visuals.menu_rounding = egui::Rounding::ZERO;
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;

    let accent_color = config.theme_accent.color();

    // Apply theme accent color to selection (using a semi-transparent variant in light mode for readability and contrast)
    if !style.visuals.dark_mode {
        style.visuals.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(
            accent_color.r(),
            accent_color.g(),
            accent_color.b(),
            65,
        );
        style.visuals.selection.stroke.color = accent_color;
    } else {
        style.visuals.selection.bg_fill = accent_color;
        style.visuals.selection.stroke.color = egui::Color32::WHITE;
    }

    // Apply to hyperlinks
    style.visuals.hyperlink_color = accent_color;

    // Apply to cursor stroke
    if config.cursor_style != CursorStyle::IBeam {
        style.visuals.text_cursor.color = egui::Color32::TRANSPARENT;
    } else {
        style.visuals.text_cursor.color = accent_color;
    }

    // Set active/hovered widget border accents to make the UI pop
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, accent_color);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, accent_color);

    // Apply uniform font sizes to all text styles (body, button, heading, monospace, small)
    let font_size = config.font_size;
    let body_font = egui::FontId::new(font_size, egui::FontFamily::Proportional);
    let button_font = egui::FontId::new(font_size, egui::FontFamily::Proportional);
    let heading_font = egui::FontId::new(font_size * 1.3, egui::FontFamily::Proportional);
    let monospace_font = egui::FontId::new(font_size, egui::FontFamily::Monospace);
    let small_font = egui::FontId::new(font_size * 0.85, egui::FontFamily::Proportional);

    style.text_styles.insert(egui::TextStyle::Body, body_font);
    style
        .text_styles
        .insert(egui::TextStyle::Button, button_font);
    style
        .text_styles
        .insert(egui::TextStyle::Heading, heading_font);
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, monospace_font);
    style.text_styles.insert(egui::TextStyle::Small, small_font);

    ctx.set_style(style);
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Helper closure to find and load existing fonts from a list of paths
    let mut load_fallbacks = |base_name: &str, paths: &[&str], max_count: usize| -> usize {
        let mut loaded = 0;
        for path in paths {
            if loaded >= max_count {
                break;
            }
            if std::path::Path::new(path).exists()
                && let Ok(bytes) = std::fs::read(path) {
                    let mut font_data = egui::FontData::from_owned(bytes);
                    // ab_glyph/egui supports loading first face from ttc collections automatically if we set index to 0
                    font_data.index = 0;
                    let name = format!("{}_{}", base_name, loaded);
                    fonts.font_data.insert(name.clone(), font_data);
                    
                    // Add as fallback to Proportional and Monospace families
                    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                        family.push(name.clone());
                    }
                    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                        family.push(name.clone());
                    }
                    loaded += 1;
                }
        }
        loaded
    };

    // Candidates for general sans/serif fallback
    let sans_paths = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    // Candidates for Chinese, Japanese, and Korean (CJK) - Prioritize Noto Sans CJK and system fonts with full CJK coverage
    let cjk_paths = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
        "/usr/share/fonts/opentype/noto/NotoSerifCJK-Bold.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/truetype/droid/DroidSansFallback.ttf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        "C:\\Windows\\Fonts\\malgun.ttf",
        "C:\\Windows\\Fonts\\meiryo.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
    ];

    // Candidates for Arabic
    let arabic_paths = &[
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Bold.ttf",
        "C:\\Windows\\Fonts\\tahoma.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "/System/Library/Fonts/GeezaPro.ttc",
        "/System/Library/Fonts/Supplemental/DecoTypeNaskh.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    // Candidates for Emoji
    let emoji_paths = &[
        "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/truetype/noto-emoji/NotoColorEmoji.ttf",
        "/usr/share/fonts/truetype/emoji/NotoColorEmoji.ttf",
        "C:\\Windows\\Fonts\\seguiemj.ttf",
        "/System/Library/Fonts/Apple Color Emoji.ttc",
        "/System/Library/Fonts/Core/Apple Color Emoji.ttc",
    ];

    load_fallbacks("fallback_sans", sans_paths, 2);
    load_fallbacks("fallback_cjk", cjk_paths, 3);
    load_fallbacks("fallback_arabic", arabic_paths, 2);
    load_fallbacks("fallback_emoji", emoji_paths, 2);

    ctx.set_fonts(fonts);
}

impl AppConfig {
    fn get_config_path() -> Option<std::path::PathBuf> {
        #[cfg(target_os = "windows")]
        {
            directories::ProjectDirs::from("com", "drmd", "dr-md")
                .map(|proj| proj.config_dir().join("config.toml"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            directories::BaseDirs::new().map(|base| {
                base.home_dir()
                    .join(".config")
                    .join("dr-md")
                    .join("config.toml")
            })
        }
    }

    pub fn load() -> Self {
        let mut config = if let Some(config_path) = Self::get_config_path() {
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path)
                    && let Ok(config) = toml::from_str(&content) {
                        config
                    } else {
                        Self::default()
                    }
            } else {
                let default_config = Self::default();
                let _ = default_config.save();
                default_config
            }
        } else {
            Self::default()
        };

        if config.preview_max_width > 100.0 {
            config.preview_max_width = 80.0;
        }
        config
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        if let Some(config_path) = Self::get_config_path() {
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(self)?;
            std::fs::write(config_path, content)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path() {
        let path = AppConfig::get_config_path().unwrap();
        assert!(
            path.to_string_lossy()
                .ends_with(".config/dr-md/config.toml")
                || path
                    .to_string_lossy()
                    .ends_with(".config\\dr-md\\config.toml")
                || cfg!(target_os = "windows")
        );
    }

    #[test]
    fn test_state_path() {
        let path = FileState::get_state_path().unwrap();
        assert!(
            path.to_string_lossy().ends_with(".config/dr-md/state.toml")
                || path
                    .to_string_lossy()
                    .ends_with(".config\\dr-md\\state.toml")
                || cfg!(target_os = "windows")
        );
    }

    #[test]
    fn test_setup_fonts() {
        let ctx = egui::Context::default();
        setup_fonts(&ctx);
    }

    #[test]
    fn test_view_mode_serde() {
        // Deserializing "preview" -> ViewMode::Preview
        let preview_tab: TabState =
            toml::from_str("path = \"test.md\"\nview_mode = \"preview\"\n").unwrap();
        assert_eq!(preview_tab.view_mode, ViewMode::Preview);

        // Deserializing "split" -> ViewMode::Split
        let split_tab: TabState =
            toml::from_str("path = \"test.md\"\nview_mode = \"split\"\n").unwrap();
        assert_eq!(split_tab.view_mode, ViewMode::Split);

        // Deserializing legacy "editor" -> ViewMode::Split
        let legacy_tab: TabState =
            toml::from_str("path = \"test.md\"\nview_mode = \"editor\"\n").unwrap();
        assert_eq!(legacy_tab.view_mode, ViewMode::Split);

        // Serializing TabState with ViewMode::Split -> contains "split"
        let split_str = toml::to_string(&split_tab).unwrap();
        assert!(split_str.contains("split"));
    }
}
