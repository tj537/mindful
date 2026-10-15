use eframe::egui::{self, Color32, CornerRadius, Margin, Stroke, Vec2};

pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    pub background: Color32,
    pub grid: Color32,
    /// Füllung der Knoten ab der 2. Ebene und der Leisten
    pub card: Color32,
    /// Text auf `card`
    pub text: Color32,
    /// Text auf farbigen Knoten (Mitte und Hauptzweige)
    pub on_accent: Color32,
    pub root: Color32,
    pub selection: Color32,
    /// Jeder Hauptzweig bekommt eine dieser Farben
    pub branches: [Color32; 6],
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const fn hex(v: u32) -> Color32 {
    rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const THEMES: [Theme; 8] = [
    Theme {
        name: "Hell",
        dark: false,
        background: rgb(246, 247, 251),
        grid: rgb(214, 218, 230),
        card: rgb(255, 255, 255),
        text: rgb(40, 44, 52),
        on_accent: Color32::WHITE,
        root: rgb(45, 55, 72),
        selection: rgb(255, 190, 40),
        branches: [
            rgb(66, 133, 244),
            rgb(234, 67, 53),
            rgb(52, 168, 83),
            rgb(251, 140, 0),
            rgb(142, 68, 173),
            rgb(0, 150, 136),
        ],
    },
    Theme {
        name: "Dunkel",
        dark: true,
        background: rgb(24, 26, 33),
        grid: rgb(48, 52, 64),
        card: rgb(38, 41, 52),
        text: rgb(230, 232, 238),
        on_accent: Color32::WHITE,
        root: rgb(92, 106, 156),
        selection: rgb(255, 200, 60),
        branches: [
            rgb(97, 175, 239),
            rgb(224, 108, 117),
            rgb(152, 195, 121),
            rgb(229, 192, 123),
            rgb(198, 120, 221),
            rgb(86, 182, 194),
        ],
    },
    Theme {
        name: "Ozean",
        dark: true,
        background: rgb(12, 32, 48),
        grid: rgb(28, 56, 78),
        card: rgb(20, 46, 66),
        text: rgb(220, 236, 245),
        on_accent: Color32::WHITE,
        root: rgb(0, 121, 145),
        selection: rgb(255, 209, 102),
        branches: [
            rgb(0, 180, 216),
            rgb(46, 196, 182),
            rgb(255, 159, 28),
            rgb(203, 153, 255),
            rgb(239, 71, 111),
            rgb(144, 224, 239),
        ],
    },
    Theme {
        name: "Sonnenuntergang",
        dark: false,
        background: rgb(255, 246, 238),
        grid: rgb(240, 218, 202),
        card: rgb(255, 252, 248),
        text: rgb(70, 40, 40),
        on_accent: Color32::WHITE,
        root: rgb(110, 40, 70),
        selection: rgb(60, 130, 200),
        branches: [
            rgb(255, 111, 89),
            rgb(255, 166, 43),
            rgb(236, 64, 122),
            rgb(171, 71, 188),
            rgb(230, 180, 30),
            rgb(240, 98, 146),
        ],
    },
    Theme {
        name: "Wald",
        dark: false,
        background: rgb(240, 244, 236),
        grid: rgb(208, 220, 200),
        card: rgb(250, 252, 247),
        text: rgb(40, 54, 40),
        on_accent: Color32::WHITE,
        root: rgb(46, 80, 52),
        selection: rgb(255, 183, 3),
        branches: [
            rgb(76, 140, 74),
            rgb(143, 170, 60),
            rgb(170, 120, 70),
            rgb(60, 130, 120),
            rgb(200, 150, 50),
            rgb(110, 90, 60),
        ],
    },
    // Catppuccin (https://catppuccin.com) – Latte, Frappé, Mocha
    Theme {
        name: "Catppuccin Latte",
        dark: false,
        background: hex(0xeff1f5),
        grid: hex(0xccd0da),
        card: hex(0xe6e9ef),
        text: hex(0x4c4f69),
        on_accent: hex(0xeff1f5),
        root: hex(0x8839ef),
        selection: hex(0xdf8e1d),
        branches: [
            hex(0x1e66f5),
            hex(0xd20f39),
            hex(0x40a02b),
            hex(0xfe640b),
            hex(0xea76cb),
            hex(0x179299),
        ],
    },
    Theme {
        name: "Catppuccin Frappé",
        dark: true,
        background: hex(0x303446),
        grid: hex(0x414559),
        card: hex(0x292c3c),
        text: hex(0xc6d0f5),
        on_accent: hex(0x232634),
        root: hex(0xca9ee6),
        selection: hex(0xe5c890),
        branches: [
            hex(0x8caaee),
            hex(0xe78284),
            hex(0xa6d189),
            hex(0xef9f76),
            hex(0xf4b8e4),
            hex(0x81c8be),
        ],
    },
    Theme {
        name: "Catppuccin Mocha",
        dark: true,
        background: hex(0x1e1e2e),
        grid: hex(0x313244),
        card: hex(0x181825),
        text: hex(0xcdd6f4),
        on_accent: hex(0x11111b),
        root: hex(0xcba6f7),
        selection: hex(0xf9e2af),
        branches: [
            hex(0x89b4fa),
            hex(0xf38ba8),
            hex(0xa6e3a1),
            hex(0xfab387),
            hex(0xf5c2e7),
            hex(0x94e2d5),
        ],
    },
];

/// Mischt zwei Farben (t = 0 → a, t = 1 → b)
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// egui-Optik passend zum Farbschema: flache Buttons, runde Ecken, Akzentfarbe
pub fn visuals_for(theme: &Theme) -> egui::Visuals {
    let mut v = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let accent = theme.branches[0];
    let raised = mix(theme.card, theme.text, 0.07);
    let border = mix(theme.card, theme.text, 0.15);

    v.override_text_color = Some(theme.text);
    v.panel_fill = theme.card;
    v.window_fill = theme.card;
    v.faint_bg_color = raised;
    v.extreme_bg_color = theme.background;
    v.code_bg_color = raised;
    v.window_stroke = stroke(1.0, border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.selection.bg_fill = accent.gamma_multiply(0.45);
    v.selection.stroke = stroke(1.0, accent);
    v.hyperlink_color = accent;
    v.text_cursor.stroke = stroke(2.0, accent);

    let radius = CornerRadius::same(8);
    let w = &mut v.widgets;
    for state in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        state.corner_radius = radius;
        state.fg_stroke.color = theme.text;
        state.expansion = 0.0;
    }
    w.noninteractive.bg_stroke = stroke(1.0, border);
    w.noninteractive.bg_fill = theme.card;
    // Buttons sind flach und bekommen erst beim Drüberfahren eine Fläche
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_fill = raised;
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.weak_bg_fill = mix(theme.card, accent, 0.16);
    w.hovered.bg_fill = mix(theme.card, accent, 0.16);
    w.hovered.bg_stroke = stroke(1.0, mix(theme.card, accent, 0.45));
    w.active.weak_bg_fill = mix(theme.card, accent, 0.30);
    w.active.bg_fill = mix(theme.card, accent, 0.30);
    w.active.bg_stroke = stroke(1.0, accent);
    w.open.weak_bg_fill = mix(theme.card, accent, 0.20);
    w.open.bg_fill = mix(theme.card, accent, 0.20);
    w.open.bg_stroke = stroke(1.0, mix(theme.card, accent, 0.45));
    v
}

/// Abstände und Schriftgrößen, unabhängig vom Farbschema
pub fn apply_spacing(style: &mut egui::Style) {
    use egui::{FontFamily, FontId, TextStyle};
    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(8.0, 6.0);
    s.button_padding = Vec2::new(10.0, 5.0);
    s.menu_margin = Margin::same(8);
    s.window_margin = Margin::same(14);
    s.interact_size.y = 26.0;
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(19.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
}

/// macOS-Systemschrift (falls vorhanden) und Phosphor-Icons laden
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let system_font = ["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Supplemental/Arial.ttf"]
        .iter()
        .find_map(|p| std::fs::read(p).ok());
    if let Some(bytes) = system_font {
        fonts
            .font_data
            .insert("system".into(), egui::FontData::from_owned(bytes).into());
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.insert(0, "system".into());
        }
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

fn stroke(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}
