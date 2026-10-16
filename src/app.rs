use crate::document::ViewAnim;
use crate::document::{Document, Mode};
use crate::export::{self, ExportNode};
use crate::images;
use crate::model::{parse_outline, Clip, MindMap, SlideLayout, StoredImage, DEFAULT_TEXT};
use crate::nodes::{
    node_size, node_style, paint_label, paint_node, paint_task, paint_thumb, thumb_size,
    with_thumb, NodeStyle, MIN_NODE, TASK_SPACE,
};
use crate::present::ease_in_out_cubic;
use crate::theme::{apply_spacing, install_fonts, visuals_for, Theme, THEMES};
use crate::{ai_prompt, markdown_view, present, tour};
use eframe::egui::{
    self, epaint::CubicBezierShape, Align, Align2, Color32, FontId, Key, KeyboardShortcut, Layout,
    Modifiers, Pos2, Rect, Sense, Stroke, Vec2,
};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 3.0;
/// Farbschema beim ersten Start: "Dark"
const DEFAULT_THEME: usize = 1;

/// Mitgelieferte Beispiele (Name, Markdown)
/// Vorlagen für neue Mindmaps (Name, Symbol, Markdown)
const TEMPLATES: [(&str, &str, &str); 6] = [
    (
        "Brainstorming",
        icon::LIGHTBULB,
        include_str!("../assets/templates/brainstorming.md"),
    ),
    (
        "Project plan",
        icon::LIST_CHECKS,
        include_str!("../assets/templates/project-plan.md"),
    ),
    (
        "SWOT analysis",
        icon::SQUARES_FOUR,
        include_str!("../assets/templates/swot.md"),
    ),
    (
        "Weekly plan",
        icon::CHECK_SQUARE,
        include_str!("../assets/templates/weekly-plan.md"),
    ),
    (
        "Meeting notes",
        icon::NOTE_PENCIL,
        include_str!("../assets/templates/meeting-notes.md"),
    ),
    (
        "Book summary",
        icon::BOOK_OPEN,
        include_str!("../assets/templates/book-summary.md"),
    ),
];

const EXAMPLES: [(&str, &str); 2] = [
    (
        "The Universe",
        include_str!("../assets/examples/universe.md"),
    ),
    (
        "Das Weltall (Deutsch)",
        include_str!("../assets/examples/weltall.md"),
    ),
];

// ---------------------------------------------------------------------------
// Tastenkürzel (Cmd auf dem Mac, Strg sonst)
// ---------------------------------------------------------------------------

const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};

const fn shortcut(modifiers: Modifiers, key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(modifiers, key)
}

const SC_NEW_TAB: KeyboardShortcut = shortcut(CMD, Key::T);
const SC_NEW: KeyboardShortcut = shortcut(CMD, Key::N);
const SC_OPEN: KeyboardShortcut = shortcut(CMD, Key::O);
const SC_SAVE: KeyboardShortcut = shortcut(CMD, Key::S);
const SC_SAVE_AS: KeyboardShortcut = shortcut(CMD_SHIFT, Key::S);
const SC_CLOSE: KeyboardShortcut = shortcut(CMD, Key::W);
const SC_UNDO: KeyboardShortcut = shortcut(CMD, Key::Z);
const SC_REDO: KeyboardShortcut = shortcut(CMD_SHIFT, Key::Z);
const SC_COPY: KeyboardShortcut = shortcut(CMD, Key::C);
const SC_CUT: KeyboardShortcut = shortcut(CMD, Key::X);
const SC_PASTE: KeyboardShortcut = shortcut(CMD, Key::V);
const SC_DUPLICATE: KeyboardShortcut = shortcut(CMD, Key::D);
const SC_FIND: KeyboardShortcut = shortcut(CMD, Key::F);
const SC_LAYOUT: KeyboardShortcut = shortcut(CMD, Key::L);
const SC_FIT: KeyboardShortcut = shortcut(CMD, Key::Num0);
const SC_ZOOM_IN: KeyboardShortcut = shortcut(CMD, Key::Plus);
const SC_ZOOM_OUT: KeyboardShortcut = shortcut(CMD, Key::Minus);
const SC_HELP: KeyboardShortcut = shortcut(Modifiers::NONE, Key::F1);
const SC_MARKDOWN: KeyboardShortcut = shortcut(CMD, Key::E);
const SC_PRESENT: KeyboardShortcut = shortcut(Modifiers::NONE, Key::F5);
const SC_TOUR: KeyboardShortcut = shortcut(Modifiers::NONE, Key::F6);
const SC_TASK: KeyboardShortcut = shortcut(CMD, Key::Enter);

// ---------------------------------------------------------------------------
// Befehle
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum ExportKind {
    Png,
    Svg,
    Markdown,
}

/// Befehle aus Menü, Tableiste und Tastenkürzeln
#[derive(Clone, Copy)]
enum Command {
    NewTab,
    Open,
    Save,
    SaveAs,
    Export(ExportKind),
    CloseTab(usize),
    SelectTab(usize),
    NextTab(i32),
    Undo,
    Redo,
    Copy,
    Cut,
    Duplicate,
    Delete,
    AutoLayout,
    CollapseAll,
    ExpandAll,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Fit,
    Find,
    FindNext,
    AddChild,
    InsertImage,
    PasteImage,
    Help,
    Theme(usize),
    SetMode(Mode),
    ToggleMarkdown,
    TogglePreview,
    CopyPrompt,
    NewMarkdownTab,
    OpenExample(usize),
    OpenTemplate(usize),
}

/// Änderungen an einzelnen Knoten der aktiven Mindmap
enum Action {
    AddChild(u64),
    AddSibling(u64),
    Rename(u64),
    Delete(u64),
    Toggle(u64),
    /// Checkbox anklicken: offen ↔ erledigt
    ToggleDone(u64),
    /// ⌘Enter: keine → offen → erledigt → offen
    CycleTask(u64),
    SetTask(u64, Option<bool>),
    /// Folien-Layout eines Hauptzweigs
    SetLayout(u64, Option<SlideLayout>),
    SetColor(u64, Option<usize>),
    /// Bild(er) per Dateidialog einfügen
    InsertImage(u64),
    /// Bild aus der Zwischenablage einfügen
    PasteImage(u64),
    RemoveImage(u64),
    Copy(u64),
    Cut(u64),
    Duplicate(u64),
    Paste(String),
    AutoLayout,
    Fit,
    SetTheme(usize),
    EditNote(u64),
    OpenExample(usize),
    OpenTemplate(usize),
    HideWelcome,
}

/// Beschriftung aus einem Dateinamen: „mars_rover-2“ → „Mars rover 2“
fn caption(name: &str) -> String {
    let text = name.replace(['_', '-'], " ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Image".into(),
    }
}

/// Zoomt so, dass der Punkt `p` auf dem Bildschirm an seiner Stelle bleibt.
fn zoom_at(doc: &mut Document, area: Rect, p: Pos2, zoom: f32) {
    let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let world = (p - area.center() - doc.pan) / doc.zoom;
    doc.pan = p - area.center() - world * zoom;
    doc.zoom = zoom;
}

/// Verschiebung und Zoom, bei denen alle sichtbaren Knoten ins Bild passen
fn fit_target(doc: &Document, area: Rect, sizes: &HashMap<u64, Vec2>) -> Option<(Vec2, f32)> {
    let mut bounds = Rect::NOTHING;
    for n in doc.map.nodes.iter().filter(|n| !doc.map.is_hidden(n.id)) {
        let size = sizes.get(&n.id).copied().unwrap_or(MIN_NODE);
        bounds = bounds.union(Rect::from_center_size(Pos2::new(n.pos[0], n.pos[1]), size));
    }
    if !bounds.is_positive() {
        return None;
    }
    let zoom = ((area.width() - 80.0) / bounds.width())
        .min((area.height() - 80.0) / bounds.height())
        .clamp(MIN_ZOOM, 1.25);
    Some((-bounds.center().to_vec2() * zoom, zoom))
}

/// Dauer von Gleiten und weicher Kamerafahrt in der Mindmap
const GLIDE: f64 = 0.7;

fn cmd_item(
    ui: &mut egui::Ui,
    enabled: bool,
    label: &str,
    sc: Option<&KeyboardShortcut>,
    cmd: Command,
    out: &mut Option<Command>,
) {
    let mut button = egui::Button::new(label);
    if let Some(sc) = sc {
        button = button.shortcut_text(ui.ctx().format_shortcut(sc));
    }
    if ui.add_enabled(enabled, button).clicked() {
        *out = Some(cmd);
        ui.close_menu();
    }
}

/// Flacher Icon-Button mit Tooltip (samt Tastenkürzel)
fn icon_button(
    ui: &mut egui::Ui,
    ic: &str,
    tip: &str,
    sc: Option<&KeyboardShortcut>,
    enabled: bool,
) -> bool {
    let tip = match sc {
        Some(sc) => format!("{tip}   {}", ui.ctx().format_shortcut(sc)),
        None => tip.to_string(),
    };
    let button = egui::Button::new(egui::RichText::new(ic).size(17.0)).min_size(Vec2::splat(30.0));
    ui.add_enabled(enabled, button)
        .on_hover_text(&tip)
        .on_disabled_hover_text(&tip)
        .clicked()
}

fn export_menu(ui: &mut egui::Ui, out: &mut Option<Command>) {
    ui.set_min_width(220.0);
    let l = |ic: &str, text: &str| format!("{ic}  {text}");
    cmd_item(
        ui,
        true,
        &l(icon::IMAGE, "Image (PNG) …"),
        None,
        Command::Export(ExportKind::Png),
        out,
    );
    cmd_item(
        ui,
        true,
        &l(icon::FILE_SVG, "Vector graphic (SVG) …"),
        None,
        Command::Export(ExportKind::Svg),
        out,
    );
    cmd_item(
        ui,
        true,
        &l(icon::MARKDOWN_LOGO, "Outline (Markdown) …"),
        None,
        Command::Export(ExportKind::Markdown),
        out,
    );
}

/// Farbschemas mit kleiner Farbvorschau
fn theme_menu(ui: &mut egui::Ui, current: usize, out: &mut Option<Command>) {
    ui.set_min_width(230.0);
    for (i, t) in THEMES.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for c in [
                t.background,
                t.root,
                t.branches[0],
                t.branches[1],
                t.branches[2],
            ] {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.5, c);
            }
            ui.add_space(6.0);
            if ui.selectable_label(current == i, t.name).clicked() {
                *out = Some(Command::Theme(i));
                ui.close_menu();
            }
        });
    }
}

fn action_item(
    ui: &mut egui::Ui,
    enabled: bool,
    label: &str,
    shortcut: &str,
    action: Action,
    out: &mut Option<Action>,
) {
    if ui
        .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
        .clicked()
    {
        *out = Some(action);
        ui.close_menu();
    }
}

fn node_menu(ui: &mut egui::Ui, theme: &Theme, map: &MindMap, id: u64, out: &mut Option<Action>) {
    ui.set_min_width(220.0);
    let fmt = |sc: &KeyboardShortcut| ui.ctx().format_shortcut(sc);
    let (copy, cut, dup) = (fmt(&SC_COPY), fmt(&SC_CUT), fmt(&SC_DUPLICATE));
    let is_root = map.parent_of(id).is_none();
    let has_children = map.has_children(id);
    let collapsed = map.get(id).is_some_and(|n| n.collapsed);

    let l = |ic: &str, text: &str| format!("{ic}  {text}");
    action_item(
        ui,
        true,
        &l(icon::PLUS_CIRCLE, "New child"),
        "Tab",
        Action::AddChild(id),
        out,
    );
    action_item(
        ui,
        true,
        &l(icon::ARROW_BEND_DOWN_RIGHT, "New sibling"),
        "Enter",
        Action::AddSibling(id),
        out,
    );
    action_item(
        ui,
        true,
        &l(icon::PENCIL_SIMPLE, "Rename"),
        "F2",
        Action::Rename(id),
        out,
    );
    let toggle = if collapsed {
        l(icon::ARROWS_OUT_SIMPLE, "Expand")
    } else {
        l(icon::ARROWS_IN_SIMPLE, "Collapse")
    };
    action_item(ui, has_children, &toggle, "Space", Action::Toggle(id), out);
    let task = map.get(id).and_then(|n| n.task);
    let cmd_enter = ui.ctx().format_shortcut(&SC_TASK);
    match task {
        None => action_item(
            ui,
            true,
            &l(icon::CHECK_SQUARE, "Add checkbox"),
            &cmd_enter,
            Action::SetTask(id, Some(false)),
            out,
        ),
        Some(done) => {
            let (ic, label) = if done {
                (icon::SQUARE, "Mark as not done")
            } else {
                (icon::CHECK_SQUARE, "Mark as done")
            };
            action_item(
                ui,
                true,
                &l(ic, label),
                &cmd_enter,
                Action::SetTask(id, Some(!done)),
                out,
            );
            action_item(
                ui,
                true,
                &l(icon::X_SQUARE, "Remove checkbox"),
                "",
                Action::SetTask(id, None),
                out,
            );
        }
    }
    action_item(
        ui,
        true,
        &l(icon::NOTE_PENCIL, "Note …"),
        "",
        Action::EditNote(id),
        out,
    );
    let has_image = map.get(id).is_some_and(|n| n.image.is_some());
    ui.menu_button(l(icon::IMAGE, "Image"), |ui| {
        let insert = if has_image {
            "Replace image …"
        } else {
            "Insert image …"
        };
        action_item(
            ui,
            true,
            &l(icon::IMAGE_SQUARE, insert),
            "",
            Action::InsertImage(id),
            out,
        );
        action_item(
            ui,
            true,
            &l(icon::CLIPBOARD, "Paste image"),
            "",
            Action::PasteImage(id),
            out,
        );
        action_item(
            ui,
            has_image,
            &l(icon::X_SQUARE, "Remove image"),
            "",
            Action::RemoveImage(id),
            out,
        );
        ui.separator();
        ui.weak("Tip: drag images from the Finder onto a node");
    });
    // Folien-Layout nur für Hauptzweige (jeder ist eine Folie)
    if map.depth(id) == 1 {
        let current = map.get(id).and_then(|n| n.layout);
        let label = match current {
            Some(layout) => l(layout.icon(), &format!("Slide layout: {}", layout.label())),
            None => l(icon::PRESENTATION, "Slide layout"),
        };
        ui.menu_button(label, |ui| {
            if let Some(choice) = crate::diagrams::layout_menu(ui, current) {
                *out = Some(Action::SetLayout(id, choice));
            }
        });
    }
    ui.menu_button(l(icon::PALETTE, "Color"), |ui| {
        ui.horizontal(|ui| {
            for (i, color) in theme.branches.iter().enumerate() {
                let swatch = egui::Button::new("")
                    .fill(*color)
                    .min_size(Vec2::splat(24.0));
                if ui.add(swatch).clicked() {
                    *out = Some(Action::SetColor(id, Some(i)));
                    ui.close_menu();
                }
            }
        });
        if ui.button("Automatic").clicked() {
            *out = Some(Action::SetColor(id, None));
            ui.close_menu();
        }
    });
    ui.separator();
    action_item(
        ui,
        true,
        &l(icon::COPY, "Copy"),
        &copy,
        Action::Copy(id),
        out,
    );
    action_item(
        ui,
        !is_root,
        &l(icon::SCISSORS, "Cut"),
        &cut,
        Action::Cut(id),
        out,
    );
    action_item(
        ui,
        !is_root,
        &l(icon::COPY_SIMPLE, "Duplicate"),
        &dup,
        Action::Duplicate(id),
        out,
    );
    ui.separator();
    action_item(
        ui,
        !is_root,
        &l(icon::TRASH, "Delete"),
        "Del",
        Action::Delete(id),
        out,
    );
}

/// Ein sichtbarer Knoten mit fertig berechneter Bildschirmposition
struct Laid {
    id: u64,
    rect: Rect,
    galley: Arc<egui::Galley>,
    style: NodeStyle,
    collapsed: bool,
    /// Auf welcher Seite des Elternknotens er liegt (+1 rechts, -1 links)
    side: f32,
    hit: bool,
    task: Option<bool>,
    /// Mit Vorschaubild: als Karte zeichnen
    card: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Wird beim Beenden gesichert und beim Start wiederhergestellt
#[derive(Deserialize)]
struct SavedState {
    /// Name statt Nummer, damit neue Farbschemas die Auswahl nicht verschieben
    #[serde(default)]
    theme_name: Option<String>,
    docs: Vec<Document>,
    active: usize,
    #[serde(default = "yes")]
    show_welcome: bool,
    #[serde(default = "yes")]
    build_bullets: bool,
    #[serde(default)]
    effects: present::Effects,
    #[serde(default)]
    prompt: ai_prompt::PromptForm,
}

fn yes() -> bool {
    true
}

#[derive(Serialize)]
struct SavedStateRef<'a> {
    theme_name: &'a str,
    docs: &'a [Document],
    active: usize,
    show_welcome: bool,
    build_bullets: bool,
    effects: present::Effects,
    prompt: &'a ai_prompt::PromptForm,
}

pub struct MindMapApp {
    docs: Vec<Document>,
    active: usize,
    theme: usize,
    applied_theme: Option<usize>,
    /// Tab wurde gedrückt (wird abgefangen, bevor egui damit den Fokus wechselt)
    tab_pressed: bool,
    /// Ctrl+Tab / Ctrl+Shift+Tab
    tab_switch: i32,
    /// Textfeld des Knotens soll im nächsten Frame den Fokus bekommen
    focus_pending: bool,
    search: String,
    search_focus: bool,
    show_help: bool,
    status: String,
    window_title: String,
    canvas_rect: Rect,
    /// Vorschau neben dem Markdown-Editor
    md_preview: bool,
    /// Offenes Notizfenster: Knoten und Stand vorher (für Rückgängig)
    note_edit: Option<(u64, MindMap)>,
    fullscreen: bool,
    /// Zeitpunkt des letzten eingefügten Textes (⌘V ohne Text = Bild einfügen)
    last_paste: f64,
    /// Bildschirm-Rechtecke der Knoten im letzten Frame (Ziel beim Hineinziehen von Bildern)
    node_rects: HashMap<u64, Rect>,
    /// Tipp-Karte mit Beispiel-Button auf leeren Mindmaps
    show_welcome: bool,
    /// Stichpunkte auf Folien nacheinander aufdecken
    build_bullets: bool,
    /// Übergänge, Hintergrund usw. für Präsentation und Tour
    effects: present::Effects,
    /// Eingaben im „AI prompt“-Formular
    prompt: ai_prompt::PromptForm,
}

impl MindMapApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        // Cmd +/- soll die Mindmap zoomen, nicht die ganze Oberfläche
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        install_fonts(&cc.egui_ctx);
        cc.egui_ctx.style_mut(apply_spacing);

        let mut app = Self {
            docs: Vec::new(),
            active: 0,
            theme: DEFAULT_THEME,
            applied_theme: None,
            tab_pressed: false,
            tab_switch: 0,
            focus_pending: false,
            search: String::new(),
            search_focus: false,
            show_help: false,
            status: "Ready".into(),
            window_title: String::new(),
            canvas_rect: Rect::NOTHING,
            md_preview: true,
            note_edit: None,
            fullscreen: false,
            last_paste: 0.0,
            node_rects: HashMap::new(),
            show_welcome: true,
            build_bullets: true,
            effects: present::Effects::default(),
            prompt: ai_prompt::PromptForm::default(),
        };

        if let Some(state) = cc
            .storage
            .and_then(|s| eframe::get_value::<SavedState>(s, eframe::APP_KEY))
        {
            if let Some(name) = state.theme_name {
                // Farbschemas hießen früher deutsch
                let name = match name.as_str() {
                    "Hell" => "Light",
                    "Dunkel" => "Dark",
                    "Ozean" => "Ocean",
                    "Sonnenuntergang" => "Sunset",
                    "Wald" => "Forest",
                    other => other,
                };
                app.theme = THEMES
                    .iter()
                    .position(|t| t.name == name)
                    .unwrap_or(DEFAULT_THEME);
            }
            app.docs = state.docs;
            app.active = state.active;
            app.show_welcome = state.show_welcome;
            app.build_bullets = state.build_bullets;
            app.effects = state.effects;
            app.effects.migrate();
            app.prompt = state.prompt;
        } else if Path::new("mindmap.json").exists() {
            // Datei aus der ersten Version übernehmen
            app.open_path(PathBuf::from("mindmap.json"));
        }
        if app.docs.is_empty() {
            app.new_tab();
        }
        app.active = app.active.min(app.docs.len() - 1);
        // Gespeicherten Modus wiederherstellen, aber nie direkt in die Präsentation
        for doc in &mut app.docs {
            let mode = if doc.mode.is_presenting() {
                Mode::Mindmap
            } else {
                doc.mode
            };
            doc.set_mode(mode);
        }
        app
    }

    fn doc(&self) -> &Document {
        &self.docs[self.active]
    }

    fn doc_mut(&mut self) -> &mut Document {
        &mut self.docs[self.active]
    }

    // -- Tabs und Dateien -----------------------------------------------------

    fn new_tab(&mut self) {
        self.close_note();
        if let Some(doc) = self.docs.get_mut(self.active) {
            doc.end_edit();
        }
        self.docs
            .push(Document::new(MindMap::new("Central topic"), None));
        self.active = self.docs.len() - 1;
    }

    fn select_tab(&mut self, i: usize) {
        if i < self.docs.len() {
            self.close_note();
            self.doc_mut().end_edit();
            self.active = i;
        }
    }

    fn open_dialog(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("Mind map", &["mindmap", "json"])
            .add_filter("Markdown / Text", &["md", "markdown", "txt"])
            .pick_files()
        else {
            return;
        };
        for path in paths {
            self.open_path(path);
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        let path = path.canonicalize().unwrap_or(path);
        if let Some(i) = self
            .docs
            .iter()
            .position(|d| d.path.as_deref() == Some(path.as_path()))
        {
            self.select_tab(i);
            return;
        }
        match Document::open(&path) {
            Ok(doc) => {
                self.push_doc(doc);
                self.status = format!("Opened: {}", path.display());
            }
            Err(e) => self.status = format!("Could not open file: {e}"),
        }
    }

    /// Zeigt ein Dokument an: ersetzt einen leeren, unberührten Tab, sonst neuer Tab.
    fn push_doc(&mut self, doc: Document) {
        self.close_note();
        let replace = self
            .docs
            .get(self.active)
            .is_some_and(|d| !d.dirty && d.path.is_none() && d.map.nodes.len() == 1);
        if replace {
            self.docs[self.active] = doc;
        } else {
            self.docs.push(doc);
            self.active = self.docs.len() - 1;
        }
    }

    fn open_template(&mut self, i: usize) {
        let Some((name, _, text)) = TEMPLATES.get(i) else {
            return;
        };
        let Some(map) = MindMap::from_outline(text) else {
            return;
        };
        let mut doc = Document::new(map, None);
        doc.layout_pending = true;
        self.push_doc(doc);
        self.status = format!("New mind map from the “{name}” template");
    }

    fn open_example(&mut self, i: usize) {
        let Some((name, text)) = EXAMPLES.get(i) else {
            return;
        };
        let Some(mut map) = MindMap::from_outline(text) else {
            return;
        };
        // Die Beispielbilder sind in die App eingebaut
        images::resolve(&mut map, None);
        let mut doc = Document::new(map, None);
        doc.layout_pending = true;
        self.push_doc(doc);
        self.status = format!("Opened example “{name}” – press F5 to present");
    }

    /// Gibt `false` zurück, wenn nicht gespeichert wurde (z. B. Dialog abgebrochen).
    fn save_doc(&mut self, i: usize) -> bool {
        let is_mindmap = |p: &PathBuf| {
            p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                e.eq_ignore_ascii_case("mindmap") || e.eq_ignore_ascii_case("json")
            })
        };
        match self.docs[i].path.clone().filter(is_mindmap) {
            Some(path) => self.write_doc(i, &path),
            None => self.save_doc_as(i),
        }
    }

    fn save_doc_as(&mut self, i: usize) -> bool {
        self.docs[i].end_edit();
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Mind map", &["mindmap"])
            .set_file_name(format!("{}.mindmap", self.docs[i].title()));
        if let Some(dir) = self.docs[i].path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        match dialog.save_file() {
            Some(path) => self.write_doc(i, &path),
            None => false,
        }
    }

    fn write_doc(&mut self, i: usize, path: &Path) -> bool {
        match self.docs[i].save_to(path) {
            Ok(()) => {
                self.status = format!("Saved: {}", path.display());
                true
            }
            Err(e) => {
                self.status = format!("Could not save: {e}");
                false
            }
        }
    }

    /// Wohin ein hineingezogenes Bild gehört: in der Präsentation an die aktuelle Folie,
    /// in der Mindmap an den Knoten unter dem Mauszeiger, sonst an den ausgewählten
    fn drop_target(&self, ctx: &egui::Context) -> Option<u64> {
        let doc = self.doc();
        let root = doc.map.root()?;
        let fallback = doc
            .selected
            .filter(|id| doc.map.get(*id).is_some())
            .unwrap_or(root);
        Some(match doc.mode {
            Mode::Present => match doc.slide {
                0 => root,
                n => doc.map.children(root).nth(n - 1).map_or(root, |c| c.id),
            },
            Mode::Mindmap => ctx
                .input(|i| i.pointer.latest_pos())
                .and_then(|p| {
                    self.node_rects
                        .iter()
                        .find(|(_, r)| r.contains(p))
                        .map(|(id, _)| *id)
                })
                .unwrap_or(fallback),
            _ => fallback,
        })
    }

    /// Bilddateien einlesen und einfügen; andere Dateien (.mindmap, .md) öffnen
    fn add_image_files(&mut self, target: u64, paths: Vec<PathBuf>) {
        let (pictures, others): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|p| images::is_image_path(p));
        for path in others {
            self.open_path(path);
        }
        if pictures.is_empty() {
            return;
        }
        let mut found = Vec::new();
        let mut failed = Vec::new();
        for path in &pictures {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            match images::import_file(path) {
                Ok(img) => found.push((caption(&name), img)),
                Err(e) => failed.push(format!("{name}: {e}")),
            }
        }
        self.add_images(target, found);
        if !failed.is_empty() {
            self.status = format!("Could not read {}", failed.join(", "));
        }
    }

    /// Ein Bild kommt an den Knoten selbst, mehrere werden je ein neuer Unterpunkt
    fn add_images(&mut self, target: u64, found: Vec<(String, (String, StoredImage))>) {
        let doc = &mut self.docs[self.active];
        if found.is_empty() || doc.map.get(target).is_none() {
            return;
        }
        doc.end_edit();
        doc.checkpoint();
        let count = found.len();
        if count == 1 {
            let (_, img) = found.into_iter().next().unwrap();
            images::attach(&mut doc.map, target, img);
        } else {
            for (name, img) in found {
                let child = doc.map.add_child_text(target, &name);
                images::attach(&mut doc.map, child, img);
            }
        }
        doc.selected = Some(target);
        doc.layout_pending = true;
        let text = doc
            .map
            .get(target)
            .map(|n| n.text.clone())
            .unwrap_or_default();
        self.status = if count == 1 {
            format!("Image added to “{text}”")
        } else {
            format!("{count} images added under “{text}”")
        };
    }

    fn close_tab(&mut self, i: usize) {
        if i >= self.docs.len() {
            return;
        }
        self.close_note();
        self.docs[i].end_edit();
        if self.docs[i].dirty {
            self.active = i;
            let answer = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Unsaved changes")
                .set_description(format!(
                    "“{}” has been modified. Do you want to save your changes?",
                    self.docs[i].title()
                ))
                .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                    "Save".into(),
                    "Don't Save".into(),
                    "Cancel".into(),
                ))
                .show();
            match answer {
                rfd::MessageDialogResult::Yes => {
                    if !self.save_doc(i) {
                        return;
                    }
                }
                rfd::MessageDialogResult::Custom(s) if s == "Save" => {
                    if !self.save_doc(i) {
                        return;
                    }
                }
                rfd::MessageDialogResult::No => {}
                rfd::MessageDialogResult::Custom(s) if s == "Don't Save" => {}
                _ => return,
            }
        }
        self.docs.remove(i);
        if self.docs.is_empty() {
            self.new_tab();
        } else {
            if self.active > i {
                self.active -= 1;
            }
            self.active = self.active.min(self.docs.len() - 1);
        }
    }

    fn export_nodes(&self, ctx: &egui::Context) -> Vec<ExportNode> {
        let theme = &THEMES[self.theme];
        let map = &self.doc().map;
        map.nodes
            .iter()
            .filter(|n| !map.is_hidden(n.id))
            .map(|n| {
                let style = node_style(map, theme, n.id);
                ExportNode {
                    center: Pos2::new(n.pos[0], n.pos[1]),
                    size: node_size(ctx, map, theme, n),
                    parent: n.parent.map(|p| map.pos(p).to_pos2()),
                    fill: style.fill,
                    text_color: style.text,
                    accent: style.accent,
                    outlined: style.outlined,
                    font_size: style.font_size,
                    edge_width: style.edge_width,
                    text: n.text.clone(),
                    task: n.task,
                }
            })
            .collect()
    }

    fn export(&mut self, ctx: &egui::Context, kind: ExportKind) {
        self.doc_mut().end_edit();
        let (ext, filter) = match kind {
            ExportKind::Png => ("png", "PNG image"),
            ExportKind::Svg => ("svg", "SVG graphic"),
            ExportKind::Markdown => ("md", "Markdown"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(filter, &[ext])
            .set_file_name(format!("{}.{ext}", self.doc().title()))
            .save_file()
        else {
            return;
        };
        let theme = &THEMES[self.theme];
        let result = match kind {
            ExportKind::Markdown => std::fs::write(&path, self.doc().map.to_markdown_with_images())
                .map_err(|e| e.to_string()),
            ExportKind::Svg => {
                let svg = export::to_svg(&self.export_nodes(ctx), theme);
                std::fs::write(&path, svg).map_err(|e| e.to_string())
            }
            ExportKind::Png => {
                export::svg_to_png(&export::to_svg(&self.export_nodes(ctx), theme), &path)
            }
        };
        self.status = match result {
            Ok(()) => format!("Exported: {}", path.display()),
            Err(e) => format!("Export failed: {e}"),
        };
    }

    // -- Befehle ausführen ----------------------------------------------------

    fn zoom_by(&mut self, factor: f32) {
        let area = self.canvas_rect;
        if area.is_positive() {
            let doc = self.doc_mut();
            let zoom = doc.zoom * factor;
            zoom_at(doc, area, area.center(), zoom);
        }
    }

    fn find_next(&mut self) {
        let query = self.search.trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        let doc = &mut self.docs[self.active];
        let hits: Vec<u64> = doc
            .map
            .nodes
            .iter()
            .filter(|n| n.text.to_lowercase().contains(&query))
            .map(|n| n.id)
            .collect();
        if hits.is_empty() {
            self.status = format!("Nothing found for “{}”", self.search.trim());
            return;
        }
        let next = doc
            .selected
            .and_then(|s| hits.iter().position(|h| *h == s))
            .map(|i| (i + 1) % hits.len())
            .unwrap_or(0);
        let id = hits[next];
        doc.map.expand_to(id);
        doc.selected = Some(id);
        doc.reveal = Some(id);
        self.status = format!("Match {} of {}", next + 1, hits.len());
    }

    fn run(&mut self, ctx: &egui::Context, cmd: Command) {
        let selected = self.doc().selected;
        match cmd {
            Command::NewTab => self.new_tab(),
            Command::Open => self.open_dialog(),
            Command::Save => {
                self.save_doc(self.active);
            }
            Command::SaveAs => {
                self.save_doc_as(self.active);
            }
            Command::Export(kind) => self.export(ctx, kind),
            Command::CloseTab(i) => self.close_tab(i),
            Command::SelectTab(i) => self.select_tab(i),
            Command::NextTab(step) => {
                let n = self.docs.len() as i32;
                self.select_tab((self.active as i32 + step).rem_euclid(n) as usize);
            }
            Command::Undo => self.doc_mut().undo(),
            Command::Redo => self.doc_mut().redo(),
            Command::Copy => {
                if let Some(id) = selected {
                    self.apply(ctx, Action::Copy(id));
                }
            }
            Command::Cut => {
                if let Some(id) = selected {
                    self.apply(ctx, Action::Cut(id));
                }
            }
            Command::Duplicate => {
                if let Some(id) = selected {
                    self.apply(ctx, Action::Duplicate(id));
                }
            }
            Command::InsertImage => {
                if let Some(id) = selected {
                    self.apply(ctx, Action::InsertImage(id));
                }
            }
            Command::PasteImage => {
                if let Some(id) = selected {
                    self.apply(ctx, Action::PasteImage(id));
                }
            }
            Command::Delete => {
                if let Some(id) = selected {
                    self.apply(ctx, Action::Delete(id));
                }
            }
            Command::AutoLayout => self.apply(ctx, Action::AutoLayout),
            Command::CollapseAll | Command::ExpandAll => {
                let doc = self.doc_mut();
                doc.checkpoint();
                doc.map
                    .set_all_collapsed(matches!(cmd, Command::CollapseAll));
                if doc.selected.is_some_and(|id| doc.map.is_hidden(id)) {
                    doc.selected = doc.map.root();
                }
            }
            Command::ZoomIn => self.zoom_by(1.25),
            Command::ZoomOut => self.zoom_by(0.8),
            Command::ZoomReset => {
                let doc_zoom = self.doc().zoom;
                self.zoom_by(1.0 / doc_zoom);
            }
            Command::Fit => self.apply(ctx, Action::Fit),
            Command::Find => self.search_focus = true,
            Command::FindNext => self.find_next(),
            Command::AddChild => {
                if let Some(id) = selected.or(self.doc().map.root()) {
                    self.apply(ctx, Action::AddChild(id));
                }
            }
            Command::Help => self.show_help = !self.show_help,
            Command::Theme(i) => self.theme = i,
            Command::SetMode(mode) => self.set_mode(ctx, mode),
            Command::ToggleMarkdown => {
                let mode = if self.doc().mode == Mode::Markdown {
                    Mode::Mindmap
                } else {
                    Mode::Markdown
                };
                self.set_mode(ctx, mode);
            }
            Command::TogglePreview => self.md_preview = !self.md_preview,
            Command::CopyPrompt => {
                ctx.copy_text(ai_prompt::build_prompt(&self.prompt));
                self.status =
                    "Prompt copied – paste it into your AI, then paste the answer into a new Markdown tab"
                        .into();
            }
            Command::NewMarkdownTab => {
                self.new_tab();
                self.set_mode(ctx, Mode::Markdown);
                let doc = self.doc_mut();
                doc.md_text.clear();
                doc.md_focus = true;
                self.status = "Paste your AI's answer here (⌘V)".into();
            }
            Command::OpenExample(i) => self.open_example(i),
            Command::OpenTemplate(i) => self.open_template(i),
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        let doc = &mut self.docs[self.active];
        match action {
            Action::AddChild(id) => {
                if doc.map.get(id).is_some() {
                    doc.checkpoint();
                    let child = doc.map.add_child(id);
                    doc.begin_edit(child, false);
                    self.focus_pending = true;
                }
            }
            Action::AddSibling(id) => {
                // An der Wurzel gibt es keine Geschwister, also Kind
                let parent = doc.map.parent_of(id).unwrap_or(id);
                doc.checkpoint();
                let sibling = doc.map.add_child(parent);
                doc.begin_edit(sibling, false);
                self.focus_pending = true;
            }
            Action::Rename(id) => {
                doc.begin_edit(id, true);
                self.focus_pending = true;
            }
            Action::Delete(id) => {
                // Die Wurzel bleibt immer stehen
                if let Some(parent) = doc.map.parent_of(id) {
                    doc.checkpoint();
                    doc.map.remove(id);
                    doc.selected = Some(parent);
                }
            }
            Action::Toggle(id) => {
                if doc.map.has_children(id) {
                    doc.checkpoint();
                    if let Some(n) = doc.map.get_mut(id) {
                        n.collapsed = !n.collapsed;
                    }
                }
            }
            Action::SetColor(id, color) => {
                doc.checkpoint();
                if let Some(n) = doc.map.get_mut(id) {
                    n.color = color;
                }
            }
            Action::InsertImage(id) => {
                let Some(paths) = rfd::FileDialog::new()
                    .set_title("Insert image")
                    .add_filter("Images", &images::EXTENSIONS)
                    .pick_files()
                else {
                    return;
                };
                self.add_image_files(id, paths);
            }
            Action::PasteImage(id) => match images::from_clipboard() {
                Some(found) => self.add_images(id, vec![("Image".into(), found)]),
                None => self.status = "There is no image on the clipboard".into(),
            },
            Action::RemoveImage(id) => {
                if doc.map.get(id).is_some_and(|n| n.image.is_some()) {
                    doc.checkpoint();
                    if let Some(n) = doc.map.get_mut(id) {
                        n.image = None;
                    }
                    doc.layout_pending = true;
                }
            }
            Action::Copy(id) => {
                ctx.copy_text(doc.map.clip_json(id));
                self.status = "Copied – select another node and paste to insert it there".into();
            }
            Action::Cut(id) => {
                if let Some(parent) = doc.map.parent_of(id) {
                    ctx.copy_text(doc.map.clip_json(id));
                    doc.checkpoint();
                    doc.map.remove(id);
                    doc.selected = Some(parent);
                    self.status = "Cut".into();
                }
            }
            Action::Duplicate(id) => {
                if let Some(parent) = doc.map.parent_of(id) {
                    doc.checkpoint();
                    let nodes = doc.map.extract(id);
                    doc.selected = doc.map.insert_subtree(parent, nodes);
                }
            }
            Action::Paste(text) => {
                let target = doc
                    .selected
                    .filter(|id| doc.map.get(*id).is_some())
                    .or(doc.map.root());
                let Some(target) = target else { return };
                doc.checkpoint();
                // Eigener kopierter Teilbaum, sonst Text als Gliederung
                let first = match serde_json::from_str::<Clip>(&text) {
                    Ok(clip) => {
                        for (key, img) in clip.images {
                            doc.map.images.entry(key).or_insert(img);
                        }
                        doc.map.insert_subtree(target, clip.mindmap_clip)
                    }
                    Err(_) => doc
                        .map
                        .add_outline(target, &parse_outline(&text))
                        .first()
                        .copied(),
                };
                doc.selected = first.or(Some(target));
            }
            Action::AutoLayout => {
                doc.checkpoint();
                doc.layout_pending = true;
            }
            Action::Fit => {
                doc.fit_pending = true;
                doc.fit_smooth = true;
            }
            Action::SetLayout(id, layout) => {
                if doc.map.get(id).is_some_and(|n| n.layout != layout) {
                    doc.checkpoint();
                    if let Some(n) = doc.map.get_mut(id) {
                        n.layout = layout;
                    }
                }
            }
            Action::ToggleDone(id) | Action::CycleTask(id) | Action::SetTask(id, _) => {
                let current = doc.map.get(id).and_then(|n| n.task);
                let new = match action {
                    Action::ToggleDone(_) => current.map(|done| !done),
                    Action::CycleTask(_) => Some(current == Some(false)),
                    Action::SetTask(_, task) => task,
                    _ => current,
                };
                if new != current {
                    doc.checkpoint();
                    if let Some(n) = doc.map.get_mut(id) {
                        n.task = new;
                    }
                }
            }
            Action::SetTheme(i) => self.theme = i,
            Action::OpenExample(i) => self.open_example(i),
            Action::OpenTemplate(i) => self.open_template(i),
            Action::HideWelcome => {
                self.show_welcome = false;
                self.status = "Tips hidden – turn them back on in the Help menu".into();
            }
            Action::EditNote(id) => {
                doc.end_edit();
                doc.selected = Some(id);
                self.note_edit = Some((id, doc.map.clone()));
            }
        }
    }

    // -- Tastatur -------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) -> Vec<Command> {
        let typing = ctx.wants_keyboard_input();
        let active = self.active;
        let mut out = Vec::new();
        ctx.input_mut(|i| {
            let mut check = |sc: KeyboardShortcut, cmd: Command| {
                if i.consume_shortcut(&sc) {
                    out.push(cmd);
                }
            };
            // Varianten mit Shift zuerst, sonst greift die ohne
            check(SC_SAVE_AS, Command::SaveAs);
            check(SC_SAVE, Command::Save);
            check(SC_OPEN, Command::Open);
            check(SC_NEW_TAB, Command::NewTab);
            check(SC_NEW, Command::NewTab);
            check(SC_CLOSE, Command::CloseTab(active));
            check(SC_FIND, Command::Find);
            check(SC_FIT, Command::Fit);
            check(SC_ZOOM_IN, Command::ZoomIn);
            check(shortcut(CMD, Key::Equals), Command::ZoomIn);
            check(SC_ZOOM_OUT, Command::ZoomOut);
            check(SC_HELP, Command::Help);
            check(SC_MARKDOWN, Command::ToggleMarkdown);
            check(SC_PRESENT, Command::SetMode(Mode::Present));
            check(SC_TOUR, Command::SetMode(Mode::Tour));
            if !typing {
                check(SC_REDO, Command::Redo);
                check(SC_UNDO, Command::Undo);
                check(SC_DUPLICATE, Command::Duplicate);
                check(SC_LAYOUT, Command::AutoLayout);
            }
            let digits = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            for (n, key) in digits.into_iter().enumerate() {
                check(shortcut(CMD, key), Command::SelectTab(n));
            }
        });
        out
    }

    fn node_keys(&mut self, ctx: &egui::Context) -> Option<Action> {
        let typing = ctx.wants_keyboard_input();
        let tab = std::mem::take(&mut self.tab_pressed);
        let doc = &mut self.docs[self.active];

        // Tab legt immer ein Kind an – auch direkt aus dem Textfeld heraus
        if tab && (doc.editing.is_some() || !typing) {
            let target = doc
                .editing
                .or(doc.selected)
                .filter(|id| doc.map.get(*id).is_some());
            doc.end_edit();
            return target.map(Action::AddChild);
        }
        // Während getippt wird, gehören die übrigen Tasten dem Textfeld
        if doc.editing.is_some() || typing {
            return None;
        }

        let mut paste = None;
        let (mut copy, mut cut, mut v_released) = (false, false, false);
        let now = ctx.input(|i| i.time);
        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Copy => copy = true,
                    egui::Event::Cut => cut = true,
                    egui::Event::Paste(text) => paste = Some(text.clone()),
                    // Liegt nur ein Bild in der Zwischenablage, meldet egui kein „Paste“ –
                    // aber das Loslassen von V bei gedrückter ⌘-Taste kommt an
                    egui::Event::Key {
                        key: Key::V,
                        pressed: false,
                        modifiers,
                        ..
                    } if modifiers.command => v_released = true,
                    _ => {}
                }
            }
        });
        if let Some(text) = paste {
            self.last_paste = now;
            return Some(Action::Paste(text));
        }
        if v_released && now - self.last_paste > 1.0 {
            let target = doc
                .selected
                .filter(|id| doc.map.get(*id).is_some())
                .or(doc.map.root())?;
            return Some(Action::PasteImage(target));
        }

        let pressed = |key: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        let arrows = [
            (Key::ArrowLeft, Vec2::new(-1.0, 0.0)),
            (Key::ArrowRight, Vec2::X),
            (Key::ArrowUp, Vec2::new(0.0, -1.0)),
            (Key::ArrowDown, Vec2::Y),
        ];

        let Some(sel) = doc.selected.filter(|id| doc.map.get(*id).is_some()) else {
            doc.selected = None;
            // Ohne Auswahl wählt eine Pfeiltaste den Mittelpunkt
            if arrows.iter().any(|(k, _)| pressed(*k)) {
                doc.selected = doc.map.root();
            }
            return None;
        };

        if copy {
            return Some(Action::Copy(sel));
        }
        if cut {
            return Some(Action::Cut(sel));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SC_TASK)) {
            return Some(Action::CycleTask(sel));
        }
        if pressed(Key::Enter) {
            return Some(Action::AddSibling(sel));
        }
        if pressed(Key::Delete) || pressed(Key::Backspace) {
            return Some(Action::Delete(sel));
        }
        if pressed(Key::F2) {
            return Some(Action::Rename(sel));
        }
        if pressed(Key::Space) {
            return Some(Action::Toggle(sel));
        }
        if pressed(Key::Escape) {
            doc.selected = None;
            return None;
        }
        for (key, dir) in arrows {
            if pressed(key) {
                if let Some(next) = doc.map.neighbor(sel, dir) {
                    doc.selected = Some(next);
                    doc.reveal = Some(next);
                }
                return None;
            }
        }
        None
    }

    // -- Oberfläche -----------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        let mut out = None;
        let active = self.active;
        let (can_undo, can_redo, has_sel) = {
            let d = self.doc();
            (d.can_undo(), d.can_redo(), d.selected.is_some())
        };
        let current_theme = self.theme;
        let mode = self.doc().mode;
        let md_preview = self.md_preview;
        let accent = THEMES[self.theme].branches[0];
        let l = |ic: &str, text: &str| format!("{ic}  {text}");

        egui::menu::bar(ui, |ui| {
            ui.label(egui::RichText::new(icon::GRAPH).size(20.0).color(accent));
            ui.label(egui::RichText::new("Mindful").strong().size(15.0));
            ui.add_space(6.0);

            ui.menu_button("File", |ui| {
                ui.set_min_width(240.0);
                cmd_item(
                    ui,
                    true,
                    &l(icon::FILE_PLUS, "New tab"),
                    Some(&SC_NEW_TAB),
                    Command::NewTab,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::FOLDER_OPEN, "Open …"),
                    Some(&SC_OPEN),
                    Command::Open,
                    &mut out,
                );
                ui.menu_button(l(icon::FILE_DASHED, "New from template"), |ui| {
                    ui.set_min_width(200.0);
                    for (i, (name, ic, _)) in TEMPLATES.iter().enumerate() {
                        cmd_item(
                            ui,
                            true,
                            &l(ic, name),
                            None,
                            Command::OpenTemplate(i),
                            &mut out,
                        );
                    }
                });
                ui.menu_button(l(icon::BOOK_OPEN, "Open example"), |ui| {
                    ui.set_min_width(180.0);
                    for (i, (name, _)) in EXAMPLES.iter().enumerate() {
                        cmd_item(
                            ui,
                            true,
                            &l(icon::PLANET, name),
                            None,
                            Command::OpenExample(i),
                            &mut out,
                        );
                    }
                });
                ui.separator();
                cmd_item(
                    ui,
                    true,
                    &l(icon::FLOPPY_DISK, "Save"),
                    Some(&SC_SAVE),
                    Command::Save,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::FLOPPY_DISK, "Save as …"),
                    Some(&SC_SAVE_AS),
                    Command::SaveAs,
                    &mut out,
                );
                ui.menu_button(l(icon::EXPORT, "Export"), |ui| export_menu(ui, &mut out));
                ui.separator();
                cmd_item(
                    ui,
                    true,
                    &l(icon::X, "Close tab"),
                    Some(&SC_CLOSE),
                    Command::CloseTab(active),
                    &mut out,
                );
            });

            ui.menu_button("Edit", |ui| {
                ui.set_min_width(250.0);
                cmd_item(
                    ui,
                    can_undo,
                    &l(icon::ARROW_COUNTER_CLOCKWISE, "Undo"),
                    Some(&SC_UNDO),
                    Command::Undo,
                    &mut out,
                );
                cmd_item(
                    ui,
                    can_redo,
                    &l(icon::ARROW_CLOCKWISE, "Redo"),
                    Some(&SC_REDO),
                    Command::Redo,
                    &mut out,
                );
                ui.separator();
                cmd_item(
                    ui,
                    has_sel,
                    &l(icon::COPY, "Copy"),
                    Some(&SC_COPY),
                    Command::Copy,
                    &mut out,
                );
                cmd_item(
                    ui,
                    has_sel,
                    &l(icon::SCISSORS, "Cut"),
                    Some(&SC_CUT),
                    Command::Cut,
                    &mut out,
                );
                cmd_item(
                    ui,
                    has_sel,
                    &l(icon::COPY_SIMPLE, "Duplicate"),
                    Some(&SC_DUPLICATE),
                    Command::Duplicate,
                    &mut out,
                );
                cmd_item(
                    ui,
                    has_sel,
                    &l(icon::TRASH, "Delete"),
                    None,
                    Command::Delete,
                    &mut out,
                );
                ui.separator();
                cmd_item(
                    ui,
                    has_sel,
                    &l(icon::IMAGE_SQUARE, "Insert image …"),
                    None,
                    Command::InsertImage,
                    &mut out,
                );
                cmd_item(
                    ui,
                    has_sel,
                    &l(icon::CLIPBOARD, "Paste image"),
                    None,
                    Command::PasteImage,
                    &mut out,
                );
                ui.separator();
                cmd_item(
                    ui,
                    true,
                    &l(icon::TREE_STRUCTURE, "Auto layout"),
                    Some(&SC_LAYOUT),
                    Command::AutoLayout,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::ARROWS_IN_SIMPLE, "Collapse all branches"),
                    None,
                    Command::CollapseAll,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::ARROWS_OUT_SIMPLE, "Expand all branches"),
                    None,
                    Command::ExpandAll,
                    &mut out,
                );
                ui.separator();
                cmd_item(
                    ui,
                    true,
                    &l(icon::MAGNIFYING_GLASS, "Find"),
                    Some(&SC_FIND),
                    Command::Find,
                    &mut out,
                );
            });

            ui.menu_button("View", |ui| {
                ui.set_min_width(240.0);
                cmd_item(
                    ui,
                    true,
                    &l(icon::MAGNIFYING_GLASS_PLUS, "Zoom in"),
                    Some(&SC_ZOOM_IN),
                    Command::ZoomIn,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::MAGNIFYING_GLASS_MINUS, "Zoom out"),
                    Some(&SC_ZOOM_OUT),
                    Command::ZoomOut,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::MAGNIFYING_GLASS, "Actual size"),
                    None,
                    Command::ZoomReset,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::ARROWS_OUT, "Fit to window"),
                    Some(&SC_FIT),
                    Command::Fit,
                    &mut out,
                );
                ui.separator();
                cmd_item(
                    ui,
                    true,
                    &l(icon::TEXT_ALIGN_LEFT, "Markdown editor"),
                    Some(&SC_MARKDOWN),
                    Command::ToggleMarkdown,
                    &mut out,
                );
                cmd_item(
                    ui,
                    mode != Mode::Mindmap,
                    &l(icon::GRAPH, "Mind map"),
                    None,
                    Command::SetMode(Mode::Mindmap),
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::PRESENTATION, "Presentation"),
                    Some(&SC_PRESENT),
                    Command::SetMode(Mode::Present),
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::PATH, "Camera tour"),
                    Some(&SC_TOUR),
                    Command::SetMode(Mode::Tour),
                    &mut out,
                );
                ui.checkbox(&mut self.build_bullets, "Reveal bullet points one by one");
                ui.menu_button(l(icon::SPARKLE, "Presentation effects"), |ui| {
                    present::effects_menu(ui, &mut self.effects, true, true)
                });
                ui.separator();
                ui.menu_button(l(icon::PALETTE, "Theme"), |ui| {
                    theme_menu(ui, current_theme, &mut out)
                });
            });

            ui.menu_button("Help", |ui| {
                ui.set_min_width(220.0);
                cmd_item(
                    ui,
                    true,
                    &l(icon::KEYBOARD, "Keyboard shortcuts"),
                    Some(&SC_HELP),
                    Command::Help,
                    &mut out,
                );
                cmd_item(
                    ui,
                    true,
                    &l(icon::ROCKET_LAUNCH, "Example: The Universe"),
                    None,
                    Command::OpenExample(0),
                    &mut out,
                );
                ui.separator();
                ui.checkbox(&mut self.show_welcome, "Tips on empty mind maps");
            });

            // Werkzeugleiste
            ui.separator();
            let mut tool = |ui: &mut egui::Ui,
                            ic: &str,
                            tip: &str,
                            sc: Option<&KeyboardShortcut>,
                            enabled: bool,
                            cmd: Command| {
                if icon_button(ui, ic, tip, sc, enabled) {
                    out = Some(cmd);
                }
            };
            tool(
                ui,
                icon::FILE_PLUS,
                "New tab",
                Some(&SC_NEW_TAB),
                true,
                Command::NewTab,
            );
            tool(
                ui,
                icon::FOLDER_OPEN,
                "Open",
                Some(&SC_OPEN),
                true,
                Command::Open,
            );
            tool(
                ui,
                icon::FLOPPY_DISK,
                "Save",
                Some(&SC_SAVE),
                true,
                Command::Save,
            );
            ui.separator();
            tool(
                ui,
                icon::ARROW_COUNTER_CLOCKWISE,
                "Undo",
                Some(&SC_UNDO),
                can_undo,
                Command::Undo,
            );
            tool(
                ui,
                icon::ARROW_CLOCKWISE,
                "Redo",
                Some(&SC_REDO),
                can_redo,
                Command::Redo,
            );
            ui.separator();
            if mode == Mode::Mindmap {
                tool(
                    ui,
                    icon::PLUS_CIRCLE,
                    "New child (Tab)",
                    None,
                    true,
                    Command::AddChild,
                );
                tool(
                    ui,
                    icon::TRASH,
                    "Delete (Del)",
                    None,
                    has_sel,
                    Command::Delete,
                );
                ui.separator();
                tool(
                    ui,
                    icon::TREE_STRUCTURE,
                    "Auto layout",
                    Some(&SC_LAYOUT),
                    true,
                    Command::AutoLayout,
                );
                tool(
                    ui,
                    icon::ARROWS_OUT,
                    "Fit to window",
                    Some(&SC_FIT),
                    true,
                    Command::Fit,
                );
                tool(
                    ui,
                    icon::MAGNIFYING_GLASS_MINUS,
                    "Zoom out",
                    Some(&SC_ZOOM_OUT),
                    true,
                    Command::ZoomOut,
                );
                tool(
                    ui,
                    icon::MAGNIFYING_GLASS_PLUS,
                    "Zoom in",
                    Some(&SC_ZOOM_IN),
                    true,
                    Command::ZoomIn,
                );
            } else {
                let tip = if md_preview {
                    "Hide preview"
                } else {
                    "Show preview"
                };
                tool(
                    ui,
                    icon::SIDEBAR_SIMPLE,
                    tip,
                    None,
                    true,
                    Command::TogglePreview,
                );
            }
            tool(
                ui,
                icon::PLAY,
                "Start presentation",
                Some(&SC_PRESENT),
                true,
                Command::SetMode(Mode::Present),
            );
            tool(
                ui,
                icon::PATH,
                "Start camera tour",
                Some(&SC_TOUR),
                true,
                Command::SetMode(Mode::Tour),
            );
            ui.separator();
            ui.menu_button(egui::RichText::new(icon::EXPORT).size(17.0), |ui| {
                export_menu(ui, &mut out)
            })
            .response
            .on_hover_text("Export");
            ui.menu_button(egui::RichText::new(icon::PALETTE).size(17.0), |ui| {
                theme_menu(ui, current_theme, &mut out)
            })
            .response
            .on_hover_text("Theme");

            // Suche rechts in der Leiste
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search …")
                        .desired_width(170.0)
                        .margin(Vec2::new(8.0, 4.0)),
                );
                if std::mem::take(&mut self.search_focus) {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                        out = Some(Command::FindNext);
                        resp.request_focus();
                    } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                        self.search.clear();
                    }
                }
                ui.label(egui::RichText::new(icon::MAGNIFYING_GLASS).size(16.0));
                let query = self.search.trim().to_lowercase();
                if !query.is_empty() {
                    let n = self.docs[active]
                        .map
                        .nodes
                        .iter()
                        .filter(|n| n.text.to_lowercase().contains(&query))
                        .count();
                    ui.weak(format!("{n} matches"));
                }
                ui.separator();

                // „AI prompt“ mit Aufklappmenü
                let popup_id = ui.make_persistent_id("ai_prompt_popup");
                let ai = ui
                    .button(format!(
                        "{}  AI prompt  {}",
                        icon::MAGIC_WAND,
                        icon::CARET_DOWN
                    ))
                    .on_hover_text("Build a prompt that lets your AI write a mind map");
                if ai.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                egui::popup::popup_below_widget(
                    ui,
                    popup_id,
                    &ai,
                    egui::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_width(460.0);
                        match ai_prompt::form_ui(ui, &mut self.prompt) {
                            Some(ai_prompt::PromptAction::Copy) => out = Some(Command::CopyPrompt),
                            Some(ai_prompt::PromptAction::NewMarkdownTab) => {
                                out = Some(Command::NewMarkdownTab);
                                ui.memory_mut(|m| m.close_popup());
                            }
                            None => {}
                        }
                    },
                );
            });
        });
        out
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, theme: &Theme) -> Option<Command> {
        let mut out = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, doc) in self.docs.iter().enumerate() {
                let active = i == self.active;
                let (fill, stroke) = if active {
                    (theme.background, Stroke::new(1.0_f32, theme.grid))
                } else {
                    (Color32::TRANSPARENT, Stroke::NONE)
                };
                egui::Frame::default()
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let dot = if active {
                                theme.branches[0]
                            } else {
                                theme.grid
                            };
                            ui.label(egui::RichText::new(icon::GRAPH).color(dot));
                            let mut title = doc.title();
                            if doc.dirty {
                                title.push_str(" •");
                            }
                            let mut text = egui::RichText::new(title);
                            if active {
                                text = text.strong();
                            }
                            let label = ui.add(
                                egui::Label::new(text)
                                    .selectable(false)
                                    .sense(Sense::click()),
                            );
                            if label.clicked() {
                                out = Some(Command::SelectTab(i));
                            }
                            if label.middle_clicked() {
                                out = Some(Command::CloseTab(i));
                            }
                            if let Some(path) = &doc.path {
                                label.on_hover_text(path.display().to_string());
                            }
                            let close = ui.add(
                                egui::Button::new(egui::RichText::new(icon::X).size(12.0)).small(),
                            );
                            if close.on_hover_text("Close tab").clicked() {
                                out = Some(Command::CloseTab(i));
                            }
                        });
                    });
            }
            if icon_button(ui, icon::PLUS, "New tab", Some(&SC_NEW_TAB), true) {
                out = Some(Command::NewTab);
            }

            // Modus-Umschalter rechts
            let current = self.docs[self.active].mode;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::Frame::default()
                    .fill(theme.background)
                    .corner_radius(9.0)
                    .inner_margin(egui::Margin::same(3))
                    .show(ui, |ui| {
                        // Das rechtsbündige Layout legt von rechts nach links an –
                        // darum rückwärts durchgehen, damit links „Markdown“ steht
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let modes = [
                                (Mode::Markdown, icon::TEXT_ALIGN_LEFT, "Markdown"),
                                (Mode::Mindmap, icon::GRAPH, "Mind map"),
                                (Mode::Present, icon::PRESENTATION, "Presentation"),
                                (Mode::Tour, icon::PATH, "Tour"),
                            ];
                            for (mode, ic, label) in modes.into_iter().rev() {
                                if ui
                                    .selectable_label(current == mode, format!("{ic}  {label}"))
                                    .clicked()
                                {
                                    out = Some(Command::SetMode(mode));
                                }
                            }
                        });
                    });
            });
        });
        out
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(self.status.as_str());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let doc = &self.docs[self.active];
                if doc.mode == Mode::Markdown {
                    let words = doc
                        .md_text
                        .split_whitespace()
                        .filter(|w| w.chars().any(char::is_alphanumeric))
                        .count();
                    ui.label(format!("{} {words} words", icon::TEXT_AA));
                    ui.separator();
                    ui.label(format!(
                        "{} {} nodes",
                        icon::TREE_STRUCTURE,
                        doc.map.nodes.len()
                    ));
                    ui.separator();
                    ui.weak("## heading = branch · - bullet = child · text = note");
                } else {
                    ui.label(format!(
                        "{} {:.0} %",
                        icon::MAGNIFYING_GLASS,
                        doc.zoom * 100.0
                    ));
                    ui.separator();
                    ui.label(format!(
                        "{} {} nodes",
                        icon::TREE_STRUCTURE,
                        doc.map.nodes.len()
                    ));
                    ui.separator();
                    ui.weak(format!(
                        "{} Right-click: menu · F1: shortcuts",
                        icon::KEYBOARD
                    ));
                }
            });
        });
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        let f = |sc: &KeyboardShortcut| ctx.format_shortcut(sc);
        let cmd = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        let rows: Vec<(String, &str)> = vec![
            ("Tab".into(), "Add child (also while typing)"),
            ("Enter".into(), "Add sibling / finish typing"),
            ("F2 · double-click".into(), "Rename"),
            ("Del".into(), "Delete node and its children"),
            ("Space".into(), "Collapse / expand"),
            (f(&SC_TASK), "Add checkbox / tick it off"),
            ("Arrow keys".into(), "Move selection"),
            ("Esc".into(), "Clear selection"),
            (f(&SC_COPY), "Copy branch"),
            (f(&SC_CUT), "Cut branch"),
            (f(&SC_PASTE), "Paste (plain text becomes an outline)"),
            (f(&SC_DUPLICATE), "Duplicate"),
            (f(&SC_UNDO), "Undo"),
            (f(&SC_REDO), "Redo"),
            (f(&SC_LAYOUT), "Auto layout"),
            (f(&SC_FIND), "Find (Enter: next match)"),
            (f(&SC_FIT), "Fit to window"),
            (format!("{} / {}", f(&SC_ZOOM_IN), f(&SC_ZOOM_OUT)), "Zoom"),
            (format!("{cmd} + scroll · pinch"), "Zoom at the pointer"),
            ("Drag canvas · scroll".into(), "Pan the view"),
            ("Drag node".into(), "Move node and its children"),
            (f(&SC_NEW_TAB), "New tab"),
            (f(&SC_CLOSE), "Close tab"),
            ("Ctrl + Tab".into(), "Next tab"),
            (format!("{cmd} 1 … 9"), "Jump to tab"),
            (f(&SC_SAVE), "Save"),
            (f(&SC_SAVE_AS), "Save as"),
            (f(&SC_OPEN), "Open (also imports Markdown)"),
            (f(&SC_MARKDOWN), "Toggle Markdown editor"),
            (f(&SC_PRESENT), "Start presentation"),
            (f(&SC_TOUR), "Start camera tour"),
            (
                "G".into(),
                "Slide strip (also: move the mouse to the bottom edge)",
            ),
            ("L".into(), "Laser pointer on/off"),
            ("→ · Space · click".into(), "Next slide"),
            ("← · right-click".into(), "Previous slide"),
            ("F · Esc".into(), "Full screen · end presentation"),
        ];
        egui::Window::new("Keyboard shortcuts")
            .open(&mut self.show_help)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("help")
                    .num_columns(2)
                    .spacing([24.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (keys, what) in rows {
                            ui.strong(keys);
                            ui.label(what);
                            ui.end_row();
                        }
                    });
            });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let theme = &THEMES[self.theme];
        let query = self.search.trim().to_lowercase();
        let focus_pending = &mut self.focus_pending;
        let doc = &mut self.docs[self.active];
        let ctx = ui.ctx().clone();
        let (bg, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let area = bg.rect;
        self.canvas_rect = area;
        let mut action = None;

        let now = ui.input(|i| i.time);

        // Anordnen und "alles anzeigen" brauchen die echten Knotengrößen
        if doc.layout_pending || doc.fit_pending {
            let sizes: HashMap<u64, Vec2> = doc
                .map
                .nodes
                .iter()
                .map(|n| (n.id, node_size(&ctx, &doc.map, theme, n)))
                .collect();
            if doc.layout_pending {
                // Alte Positionen merken: die Knoten gleiten von dort an ihren neuen Platz
                let before = doc.map.nodes.iter().map(|n| (n.id, n.pos)).collect();
                doc.map.auto_layout(&sizes);
                doc.layout_anim = Some((before, now));
                doc.layout_pending = false;
                doc.fit_pending = true;
                doc.fit_smooth = true;
            }
            if doc.fit_pending {
                if let Some((pan, zoom)) = fit_target(doc, area, &sizes) {
                    if doc.fit_smooth {
                        doc.view_anim = Some(ViewAnim {
                            from_pan: doc.pan,
                            from_zoom: doc.zoom,
                            to_pan: pan,
                            to_zoom: zoom,
                            start: now,
                        });
                    } else {
                        doc.pan = pan;
                        doc.zoom = zoom;
                    }
                }
                doc.fit_pending = false;
                doc.fit_smooth = false;
            }
        }

        // Weiche Kamerafahrt
        if let Some(v) = doc.view_anim {
            let t = ((now - v.start) / GLIDE).clamp(0.0, 1.0) as f32;
            let e = ease_in_out_cubic(t);
            doc.zoom = (v.from_zoom.ln() + (v.to_zoom.ln() - v.from_zoom.ln()) * e).exp();
            doc.pan = v.from_pan + (v.to_pan - v.from_pan) * e;
            if t >= 1.0 {
                doc.view_anim = None;
            } else {
                ctx.request_repaint();
            }
        }

        // Zoomen (Cmd+Scrollen, Pinch) an der Maus, sonst Scrollen = Verschieben
        if ui.rect_contains_pointer(area) {
            let (zoom_delta, scroll, hover) =
                ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
            if zoom_delta != 1.0 {
                if let Some(p) = hover {
                    doc.view_anim = None;
                    let zoom = doc.zoom * zoom_delta;
                    zoom_at(doc, area, p, zoom);
                }
            } else if scroll != Vec2::ZERO {
                doc.view_anim = None;
                doc.pan += scroll;
            }
        }
        if bg.dragged() {
            doc.view_anim = None;
            doc.pan += bg.drag_delta();
        }
        if bg.clicked() {
            doc.end_edit();
            doc.selected = None;
        }

        let zoom = doc.zoom;
        let origin = area.center() + doc.pan;
        let to_screen = |p: [f32; 2]| origin + Vec2::new(p[0], p[1]) * zoom;

        // Hintergrund mit Punktraster, das sich mitbewegt
        painter.rect_filled(area, 0.0, theme.background);
        let mut spacing = 28.0 * zoom;
        while spacing < 14.0 {
            spacing *= 2.0;
        }
        let mut y = area.top() + (origin.y - area.top()).rem_euclid(spacing);
        while y < area.bottom() {
            let mut x = area.left() + (origin.x - area.left()).rem_euclid(spacing);
            while x < area.right() {
                painter.circle_filled(Pos2::new(x, y), 1.3, theme.grid);
                x += spacing;
            }
            y += spacing;
        }

        // Nach dem Anordnen gleiten die Knoten von ihren alten Plätzen herüber
        let mut gliding: HashMap<u64, [f32; 2]> = HashMap::new();
        if let Some((from, start)) = &doc.layout_anim {
            let e = ease_in_out_cubic(((now - *start) / GLIDE).clamp(0.0, 1.0) as f32);
            if e < 1.0 {
                for n in &doc.map.nodes {
                    let a = from.get(&n.id).copied().unwrap_or(n.pos);
                    gliding.insert(
                        n.id,
                        [a[0] + (n.pos[0] - a[0]) * e, a[1] + (n.pos[1] - a[1]) * e],
                    );
                }
            }
        }
        if doc.layout_anim.is_some() {
            if gliding.is_empty() {
                doc.layout_anim = None;
            } else {
                ctx.request_repaint();
            }
        }

        // Sichtbare Knoten berechnen
        let mut layout: Vec<Laid> = doc
            .map
            .nodes
            .iter()
            .filter(|n| !doc.map.is_hidden(n.id))
            .map(|n| {
                let style = node_style(&doc.map, theme, n.id);
                let galley = painter.layout_no_wrap(
                    n.text.clone(),
                    FontId::proportional(style.font_size * zoom),
                    style.text,
                );
                let task_space = if n.task.is_some() { TASK_SPACE } else { 0.0 };
                let mut size = (galley.size()
                    + (style.padding + Vec2::new(task_space, 0.0)) * zoom)
                    .max(MIN_NODE * zoom);
                let thumb = thumb_size(&doc.map, n);
                if let Some(thumb) = thumb {
                    size = with_thumb(size, thumb * zoom);
                }
                let side = match n.parent {
                    Some(p) if n.pos[0] < doc.map.pos(p).x => -1.0,
                    _ => 1.0,
                };
                Laid {
                    id: n.id,
                    rect: Rect::from_center_size(
                        to_screen(gliding.get(&n.id).copied().unwrap_or(n.pos)),
                        size,
                    ),
                    galley,
                    style,
                    collapsed: n.collapsed && doc.map.has_children(n.id),
                    side,
                    hit: !query.is_empty() && n.text.to_lowercase().contains(&query),
                    task: n.task,
                    card: thumb.is_some(),
                }
            })
            .collect();
        // Bild wird hineingezogen: Knoten unter dem Mauszeiger hervorheben
        let file_hover = ctx.input(|i| {
            (!i.raw.hovered_files.is_empty())
                .then(|| i.pointer.latest_pos())
                .flatten()
        });
        if let Some(p) = file_hover {
            if let Some(l) = layout.iter_mut().rev().find(|l| l.rect.contains(p)) {
                l.hit = true;
            }
        }
        let rects: HashMap<u64, Rect> = layout.iter().map(|l| (l.id, l.rect)).collect();
        self.node_rects = rects.clone();

        // Ausgewählten Knoten ins Bild holen, falls er außerhalb liegt
        if let Some(id) = doc.reveal.take() {
            if let Some(r) = rects.get(&id) {
                if !area.shrink(30.0).contains_rect(*r) {
                    doc.pan += area.center() - r.center();
                    ctx.request_repaint();
                }
            }
        }

        // Geschwungene Verbindungslinien, unter den Knoten
        for l in &layout {
            let Some(parent) = doc.map.parent_of(l.id).and_then(|p| rects.get(&p)) else {
                continue;
            };
            let (a, b) = (parent.center(), l.rect.center());
            let dx = (b.x - a.x) * 0.5;
            painter.add(CubicBezierShape::from_points_stroke(
                [a, a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0), b],
                false,
                Color32::TRANSPARENT,
                Stroke::new(
                    l.style.edge_width * zoom.clamp(0.5, 2.0),
                    l.style.accent.gamma_multiply(0.8),
                ),
            ));
        }

        for l in &layout {
            let (id, rect) = (l.id, l.rect);
            let resp = ui.interact(rect, ui.id().with(("node", id)), Sense::click_and_drag());

            if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
                doc.selected = Some(id);
            }
            if resp.double_clicked() {
                action = Some(Action::Rename(id));
            }
            if resp.drag_started() {
                doc.checkpoint();
                doc.layout_anim = None;
            }
            if resp.dragged() {
                // Knoten samt Unterbaum verschieben
                let d = resp.drag_delta() / zoom;
                for nid in doc.map.subtree(id) {
                    if let Some(n) = doc.map.get_mut(nid) {
                        n.pos[0] += d.x;
                        n.pos[1] += d.y;
                    }
                }
            }
            resp.context_menu(|ui| node_menu(ui, theme, &doc.map, id, &mut action));

            paint_node(
                &painter,
                theme,
                rect,
                &l.style,
                zoom,
                doc.selected == Some(id),
                l.hit,
                l.card,
            );
            // Vorschaubild oben, Text und Checkbox darunter
            let label = match doc.map.get(id) {
                Some(n) => paint_thumb(&painter, &doc.map, n, rect, zoom),
                None => rect,
            };

            // Checkbox zum Anklicken
            if let Some(done) = l.task {
                let r = paint_task(&painter, label, &l.style, done, zoom);
                let check = ui.interact(
                    r.expand(4.0 * zoom),
                    ui.id().with(("task", id)),
                    Sense::click(),
                );
                let tip = if done {
                    "Mark as not done"
                } else {
                    "Mark as done"
                };
                if check.on_hover_text(tip).clicked() {
                    action = Some(Action::ToggleDone(id));
                }
            }

            // Notiz-Symbol oben rechts; die Notiz erscheint beim Drüberfahren
            let note = doc
                .map
                .get(id)
                .map(|n| n.note.trim().to_string())
                .unwrap_or_default();
            if !note.is_empty() {
                let c = rect.right_top() + Vec2::new(-10.0, 2.0) * zoom;
                painter.circle_filled(c, 9.0 * zoom, theme.card);
                painter.circle_stroke(c, 9.0 * zoom, Stroke::new(1.5 * zoom, l.style.accent));
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    icon::NOTE,
                    FontId::proportional(10.0 * zoom),
                    l.style.accent,
                );
                if doc.editing != Some(id) && !resp.dragged() {
                    resp.on_hover_text(note);
                }
            }

            // Zähler für eingeklappte Unterpunkte; Klick klappt aus
            if l.collapsed {
                let count = doc.map.subtree(id).len() - 1;
                let c = rect.center() + Vec2::new(l.side * (rect.width() / 2.0 + 16.0 * zoom), 0.0);
                painter.line_segment(
                    [
                        rect.center() + Vec2::new(l.side * rect.width() / 2.0, 0.0),
                        c,
                    ],
                    Stroke::new(2.0 * zoom, l.style.accent),
                );
                painter.circle_filled(c, 11.0 * zoom, l.style.accent);
                painter.text(
                    c,
                    Align2::CENTER_CENTER,
                    count.to_string(),
                    FontId::proportional(11.0 * zoom),
                    theme.on_accent,
                );
                let badge = ui.interact(
                    Rect::from_center_size(c, Vec2::splat(22.0 * zoom)),
                    ui.id().with(("badge", id)),
                    Sense::click(),
                );
                if badge.on_hover_text("Expand").clicked() {
                    action = Some(Action::Toggle(id));
                }
            }

            if doc.editing == Some(id) {
                let mut lost = false;
                if let Some(node) = doc.map.get_mut(id) {
                    let mut field = label.shrink2(Vec2::new(12.0, 4.0) * zoom);
                    if l.task.is_some() {
                        field.min.x += TASK_SPACE * zoom;
                    }
                    let edit = ui.put(
                        field,
                        egui::TextEdit::singleline(&mut node.text)
                            .font(FontId::proportional(l.style.font_size * zoom))
                            .text_color(l.style.text)
                            .frame(false)
                            .horizontal_align(Align::Center)
                            .vertical_align(Align::Center)
                            .hint_text(DEFAULT_TEXT),
                    );
                    if *focus_pending {
                        edit.request_focus();
                        *focus_pending = false;
                    } else if edit.lost_focus() {
                        // Enter, Esc oder Klick daneben beendet das Bearbeiten
                        lost = true;
                    }
                }
                if lost {
                    doc.end_edit();
                }
            } else {
                paint_label(&painter, label, l.galley.clone(), &l.style, l.task, zoom);
            }
        }

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let hint = format!(
                "{}  Drop on a node to add the image – several images become new points",
                icon::IMAGE
            );
            let pos = Pos2::new(area.center().x, area.bottom() - 30.0);
            let galley = painter.layout_no_wrap(hint, FontId::proportional(15.0), theme.on_accent);
            let pill = Rect::from_center_size(pos, galley.size() + Vec2::new(28.0, 14.0));
            painter.rect_filled(pill, pill.height() / 2.0, theme.root);
            painter.galley(pill.center() - galley.size() / 2.0, galley, theme.on_accent);
            ctx.request_repaint();
        }

        // Tipp-Karte auf einer leeren, unberührten Mindmap
        let pristine = doc.map.nodes.len() == 1 && doc.path.is_none() && !doc.dirty;
        if self.show_welcome && pristine {
            let card = Rect::from_center_size(
                Pos2::new(area.center().x, area.bottom() - 70.0),
                Vec2::new(area.width().min(760.0), 80.0),
            );
            let mut card_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(card)
                    .layout(Layout::top_down(Align::Center)),
            );
            egui::Frame::default()
                .fill(theme.card)
                .stroke(Stroke::new(1.0_f32, theme.grid))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(16, 10))
                .show(&mut card_ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(icon::LIGHTBULB)
                                .size(22.0)
                                .color(theme.selection),
                        );
                        ui.vertical(|ui| {
                            ui.strong("Empty mind map");
                            ui.weak("Tab adds a child, right-click opens the menu.");
                        });
                        ui.add_space(10.0);
                        if ui
                            .button(format!("{}  Example: The Universe", icon::ROCKET_LAUNCH))
                            .clicked()
                        {
                            action = Some(Action::OpenExample(0));
                        }
                        ui.menu_button(format!("{}  Templates", icon::FILE_DASHED), |ui| {
                            ui.set_min_width(200.0);
                            for (i, (name, ic, _)) in TEMPLATES.iter().enumerate() {
                                if ui.button(format!("{ic}  {name}")).clicked() {
                                    action = Some(Action::OpenTemplate(i));
                                    ui.close_menu();
                                }
                            }
                        });
                        let close = ui.add(egui::Button::new(icon::X).frame(false));
                        if close.on_hover_text("Hide tips").clicked() {
                            action = Some(Action::HideWelcome);
                        }
                    });
                });
        }

        // Rechtsklick auf die freie Fläche
        let root = doc.map.root();
        let current_theme = self.theme;
        bg.context_menu(|ui| {
            ui.set_min_width(220.0);
            let layout_sc = ui.ctx().format_shortcut(&SC_LAYOUT);
            let fit_sc = ui.ctx().format_shortcut(&SC_FIT);
            if let Some(root) = root {
                action_item(
                    ui,
                    true,
                    "New main branch",
                    "",
                    Action::AddChild(root),
                    &mut action,
                );
            }
            action_item(
                ui,
                true,
                "Auto layout",
                &layout_sc,
                Action::AutoLayout,
                &mut action,
            );
            action_item(ui, true, "Fit to window", &fit_sc, Action::Fit, &mut action);
            ui.separator();
            ui.menu_button("Theme", |ui| {
                for (i, t) in THEMES.iter().enumerate() {
                    if ui.radio(current_theme == i, t.name).clicked() {
                        action = Some(Action::SetTheme(i));
                        ui.close_menu();
                    }
                }
            });
        });

        action
    }

    fn set_mode(&mut self, ctx: &egui::Context, mode: Mode) {
        let was = self.doc().mode;
        if was == mode {
            return;
        }
        self.close_note();
        self.doc_mut().set_mode(mode);
        // Präsentation läuft im Vollbild
        if mode.is_presenting() || was.is_presenting() {
            self.fullscreen = mode.is_presenting();
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }
    }

    fn close_note(&mut self) {
        if let Some((_, before)) = self.note_edit.take() {
            self.doc_mut().record(before);
        }
    }

    fn note_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.note_edit.as_ref().map(|(id, _)| *id) else {
            return;
        };
        let doc = &mut self.docs[self.active];
        let Some(node) = doc.map.get_mut(id) else {
            self.note_edit = None;
            return;
        };
        let mut open = true;
        let mut changed = false;
        egui::Window::new(format!("{}  {}", icon::NOTE_PENCIL, node.text))
            .id(egui::Id::new("note_window"))
            .open(&mut open)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                let edit = egui::TextEdit::multiline(&mut node.note)
                    .desired_rows(8)
                    .desired_width(f32::INFINITY)
                    .hint_text("Note for this node …");
                changed = ui.add(edit).changed();
                ui.weak("Shown in the Markdown editor and on the slides.");
            });
        if changed {
            doc.dirty = true;
        }
        if !open {
            self.close_note();
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let doc = self.doc();
        let title = format!(
            "{}{} – Mindful",
            doc.title(),
            if doc.dirty { " •" } else { "" }
        );
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}

impl eframe::App for MindMapApp {
    /// egui wechselt bei Tab sonst den Fokus durch die Buttons der Leiste.
    /// Darum wird Tab hier abgefangen, bevor egui es sieht.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // Im Markdown-Editor rückt Tab ein und bleibt deshalb unangetastet
        let markdown = self
            .docs
            .get(self.active)
            .is_some_and(|d| d.mode == Mode::Markdown);
        raw_input.events.retain(|event| match event {
            egui::Event::Key {
                key: Key::Tab,
                modifiers,
                ..
            } if markdown && !modifiers.ctrl => true,
            egui::Event::Key {
                key: Key::Tab,
                pressed,
                modifiers,
                ..
            } => {
                if *pressed {
                    if modifiers.ctrl {
                        self.tab_switch += if modifiers.shift { -1 } else { 1 };
                    } else if modifiers.is_none() {
                        self.tab_pressed = true;
                    }
                }
                false
            }
            _ => true,
        });
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_theme != Some(self.theme) {
            ctx.set_visuals(visuals_for(&THEMES[self.theme]));
            self.applied_theme = Some(self.theme);
        }
        // Hineingezogene Dateien: Bilder an einen Knoten hängen, Mindmaps öffnen
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            match self.drop_target(ctx) {
                Some(target) => self.add_image_files(target, dropped),
                None => dropped.into_iter().for_each(|p| self.open_path(p)),
            }
        }

        let theme = &THEMES[self.theme];
        let mode = self.doc().mode;

        // Folien oder Kamerafahrt: nur die Präsentation, keine Leisten
        if mode.is_presenting() {
            self.tab_pressed = false;
            let build_bullets = self.build_bullets;
            let mut event = None;
            egui::CentralPanel::default()
                .frame(egui::Frame::default().fill(theme.background))
                .show(ctx, |ui| {
                    let doc = &mut self.docs[self.active];
                    let fx = &mut self.effects;
                    event = if mode == Mode::Tour {
                        tour::show(ui, doc, theme, fx)
                    } else {
                        present::show(ui, doc, theme, build_bullets, fx)
                    };
                });
            match event {
                Some(present::Event::Exit) => self.set_mode(ctx, Mode::Mindmap),
                Some(present::Event::ToggleFullscreen) => {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                }
                None => {}
            }
            self.update_title(ctx);
            return;
        }

        let mut commands = self.shortcuts(ctx);
        if self.tab_switch != 0 {
            commands.push(Command::NextTab(std::mem::take(&mut self.tab_switch)));
        }

        let bar = egui::Frame::default()
            .fill(theme.card)
            .inner_margin(egui::Margin::symmetric(12, 6));
        egui::TopBottomPanel::top("menu")
            .frame(bar)
            .show(ctx, |ui| {
                commands.extend(self.menu_bar(ui));
            });
        egui::TopBottomPanel::top("tabs")
            .frame(bar)
            .show(ctx, |ui| {
                commands.extend(self.tab_bar(ui, theme));
            });
        egui::TopBottomPanel::bottom("status")
            .frame(bar)
            .show(ctx, |ui| self.status_bar(ui));

        let mut action = None;
        if mode == Mode::Mindmap {
            action = self.node_keys(ctx);
        } else {
            self.tab_pressed = false;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(theme.background))
            .show(ctx, |ui| match mode {
                Mode::Markdown => {
                    markdown_view::show(ui, &mut self.docs[self.active], theme, self.md_preview)
                }
                _ => {
                    if let Some(a) = self.canvas(ui) {
                        action = Some(a);
                    }
                }
            });

        self.help_window(ctx);
        self.note_window(ctx);

        if let Some(a) = action {
            self.apply(ctx, a);
        }
        for cmd in commands {
            self.run(ctx, cmd);
        }
        self.update_title(ctx);
    }

    /// Alle Tabs samt ungespeicherter Änderungen sichern; beim nächsten Start
    /// ist alles wieder da.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = SavedStateRef {
            theme_name: THEMES[self.theme].name,
            docs: &self.docs,
            active: self.active,
            show_welcome: self.show_welcome,
            build_bullets: self.build_bullets,
            effects: self.effects,
            prompt: &self.prompt,
        };
        eframe::set_value(storage, eframe::APP_KEY, &state);
    }
}
