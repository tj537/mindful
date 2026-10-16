use crate::model::{MindMap, DEFAULT_TEXT};
use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const UNDO_LIMIT: usize = 200;

/// Ansicht eines Dokuments
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Mindmap,
    Markdown,
    Present,
    /// Kamerafahrt durch die Mindmap
    Tour,
}

impl Mode {
    /// Folien oder Kamerafahrt: Vollbild, keine Leisten
    pub fn is_presenting(self) -> bool {
        matches!(self, Mode::Present | Mode::Tour)
    }
}

/// Kamera der Tour: welcher Punkt der Mindmap in der Mitte ist und wie groß
#[derive(Clone, Copy, Default)]
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
}

/// Weiche Kamerafahrt der Mindmap-Ansicht (z. B. bei „Alles anzeigen“)
#[derive(Clone, Copy)]
pub struct ViewAnim {
    pub from_pan: Vec2,
    pub from_zoom: f32,
    pub to_pan: Vec2,
    pub to_zoom: f32,
    pub start: f64,
}

/// Zustand der Kamerafahrt (nicht gespeichert)
#[derive(Default)]
pub struct TourState {
    /// Aktuelle Station (0 = Übersicht)
    pub stop: usize,
    /// Vorherige Station, für den Scheinwerfer-Übergang
    pub prev_stop: usize,
    /// Kamera beim Start des aktuellen Flugs
    pub from: Camera,
    /// Startzeit des aktuellen Flugs (egui-Zeit in Sekunden)
    pub started: f64,
    /// Rückwärts geblättert: Unterpunkte sofort zeigen statt aufwachsen lassen
    pub backwards: bool,
    /// Erster Frame steht noch aus: Kamera ohne Flug setzen
    pub needs_init: bool,
    /// Beim Aufdecken Ebene für Ebene: wie viele Ebenen unter dem Zweig sichtbar sind
    pub level: usize,
    /// Kamera im letzten Frame (für Warp-Streifen)
    pub last_cam: Option<(Camera, f64)>,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

/// Eine geöffnete Mindmap (ein Tab).
#[derive(Serialize, Deserialize)]
pub struct Document {
    pub map: MindMap,
    pub path: Option<PathBuf>,
    /// Ungespeicherte Änderungen
    pub dirty: bool,
    #[serde(default)]
    pub mode: Mode,

    #[serde(skip)]
    pub selected: Option<u64>,
    #[serde(skip)]
    pub editing: Option<u64>,
    /// Knoten, der ins Bild gerückt werden soll
    #[serde(skip)]
    pub reveal: Option<u64>,
    #[serde(skip)]
    edit_before: Option<MindMap>,
    #[serde(skip)]
    undo: Vec<MindMap>,
    #[serde(skip)]
    redo: Vec<MindMap>,

    #[serde(skip)]
    pub pan: Vec2,
    #[serde(skip, default = "one")]
    pub zoom: f32,
    /// Beim nächsten Zeichnen alles ins Bild holen
    #[serde(skip, default = "yes")]
    pub fit_pending: bool,
    /// Beim nächsten Zeichnen automatisch anordnen
    #[serde(skip)]
    pub layout_pending: bool,
    /// „Alles anzeigen“ weich animieren statt springen
    #[serde(skip)]
    pub fit_smooth: bool,
    /// Knoten gleiten nach dem Anordnen von den alten Positionen (mit Startzeit)
    #[serde(skip)]
    pub layout_anim: Option<(HashMap<u64, [f32; 2]>, f64)>,
    #[serde(skip)]
    pub view_anim: Option<ViewAnim>,

    /// Text im Markdown-Editor
    #[serde(skip)]
    pub md_text: String,
    /// Markdown-Editor soll im nächsten Frame den Fokus bekommen
    #[serde(skip)]
    pub md_focus: bool,
    /// Schon ein Rückgängig-Schritt für diese Markdown-Sitzung angelegt?
    #[serde(skip)]
    md_checkpointed: bool,
    /// Aktuelle Folie im Präsentationsmodus
    #[serde(skip)]
    pub slide: usize,
    /// Wie viele Stichpunkt-Gruppen der aktuellen Folie schon sichtbar sind
    #[serde(skip)]
    pub build: usize,
    /// Zeitpunkt, zu dem die letzte Gruppe eingeblendet wurde
    #[serde(skip)]
    pub build_time: f64,
    /// Zeitpunkt, zu dem die aktuelle Folie erschienen ist (für den Titel-Effekt)
    #[serde(skip)]
    pub slide_time: f64,
    /// Schnellwahl (Taste G) geöffnet
    #[serde(skip)]
    pub strip: bool,
    /// In der Schnellwahl markierte Folie bzw. Station
    #[serde(skip)]
    pub strip_cursor: usize,
    #[serde(skip)]
    pub tour: TourState,
    /// Referentenansicht im zweiten Fenster offen
    #[serde(skip)]
    pub presenter: bool,
    /// Start der Stoppuhr in der Referentenansicht
    #[serde(skip)]
    pub presenter_start: Option<f64>,
}

impl Document {
    pub fn new(map: MindMap, path: Option<PathBuf>) -> Self {
        Self {
            selected: map.root(),
            map,
            path,
            dirty: false,
            mode: Mode::Mindmap,
            editing: None,
            reveal: None,
            edit_before: None,
            undo: Vec::new(),
            redo: Vec::new(),
            pan: Vec2::ZERO,
            zoom: 1.0,
            fit_pending: true,
            layout_pending: false,
            fit_smooth: false,
            layout_anim: None,
            view_anim: None,
            md_text: String::new(),
            md_focus: false,
            md_checkpointed: false,
            slide: 0,
            build: 0,
            build_time: 0.0,
            slide_time: 0.0,
            strip: false,
            strip_cursor: 0,
            tour: TourState::default(),
            presenter: false,
            presenter_start: None,
        }
    }

    /// Öffnet eine .mindmap-Datei oder importiert Markdown/Text.
    pub fn open(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if matches!(ext.as_str(), "md" | "markdown" | "txt") {
            let mut map = MindMap::from_markdown_with_images(&text)
                .ok_or_else(|| "The file contains no outline".to_string())?;
            // Bildverweise wie ![](foto.jpg) neben der Datei gleich mit einlesen
            crate::images::resolve(&mut map, path.parent());
            // Import: ohne Pfad, damit Speichern nach einem .mindmap-Namen fragt
            let mut doc = Self::new(map, None);
            doc.layout_pending = true;
            doc.dirty = true;
            Ok(doc)
        } else {
            let map: MindMap =
                serde_json::from_str(&text).map_err(|e| format!("Not a valid mind map ({e})"))?;
            if map.root().is_none() {
                return Err("The mind map has no central topic".into());
            }
            Ok(Self::new(map, Some(path.to_path_buf())))
        }
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        self.end_edit();
        // Bilder, die kein Knoten mehr zeigt, nicht mitspeichern
        self.map.prune_images();
        let text = serde_json::to_string_pretty(&self.map).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// Dateiname, sonst der Text des Mittelpunkts
    pub fn title(&self) -> String {
        if let Some(stem) = self.path.as_ref().and_then(|p| p.file_stem()) {
            return stem.to_string_lossy().into_owned();
        }
        let root = self
            .map
            .root()
            .and_then(|r| self.map.get(r))
            .map(|n| n.text.trim().to_string())
            .unwrap_or_default();
        if root.is_empty() {
            return "Untitled".into();
        }
        let mut short: String = root.chars().take(24).collect();
        if root.chars().count() > 24 {
            short.push('…');
        }
        short
    }

    // -- Modi -------------------------------------------------------------------

    pub fn set_mode(&mut self, mode: Mode) {
        self.end_edit();
        match mode {
            Mode::Markdown => self.reload_markdown(),
            Mode::Present => {
                // Bei der Folie des ausgewählten Zweigs anfangen
                let root = self.map.root();
                self.slide = self
                    .selected
                    .and_then(|id| self.map.branch_of(id))
                    .and_then(|b| root.and_then(|r| self.map.children(r).position(|n| n.id == b)))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                self.build = 0;
                self.strip = false;
                // NaN = „gerade gestartet“: der erste Frame setzt die echte Zeit,
                // damit der Titel-Effekt auch auf der ersten Folie läuft
                self.slide_time = f64::NAN;
                // Stoppuhr der Referentenansicht startet neu
                self.presenter_start = None;
            }
            Mode::Tour => {
                self.tour = TourState {
                    needs_init: true,
                    ..TourState::default()
                };
                self.strip = false;
            }
            Mode::Mindmap => {
                if self.mode == Mode::Markdown {
                    self.fit_pending = true;
                }
            }
        }
        // Im Editor getippte Bildpfade (![](~/Bilder/foto.jpg)) beim Verlassen einlesen
        if self.mode == Mode::Markdown && mode != Mode::Markdown {
            let base = self
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf);
            crate::images::resolve(&mut self.map, base.as_deref());
        }
        self.mode = mode;
    }

    fn reload_markdown(&mut self) {
        self.md_text = self.map.to_markdown();
        self.md_checkpointed = false;
    }

    /// Nach jeder Änderung im Markdown-Editor: Text in die Mindmap übernehmen.
    pub fn markdown_changed(&mut self) {
        if !self.md_checkpointed {
            let before = self.map.clone();
            self.push_undo(before);
            self.md_checkpointed = true;
        }
        self.dirty = true;
        self.map.apply_outline(&self.md_text);
        self.fix_selection();
    }

    // -- Rückgängig / Wiederholen ---------------------------------------------

    fn push_undo(&mut self, before: MindMap) {
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Vor jeder Änderung aufrufen, damit sie rückgängig gemacht werden kann.
    pub fn checkpoint(&mut self) {
        self.end_edit();
        let before = self.map.clone();
        self.push_undo(before);
    }

    /// Merkt `before` als Rückgängig-Schritt, falls sich seitdem etwas geändert hat.
    pub fn record(&mut self, before: MindMap) {
        if before != self.map {
            self.push_undo(before);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        self.end_edit();
        if let Some(prev) = self.undo.pop() {
            let current = std::mem::replace(&mut self.map, prev);
            self.redo.push(current);
            self.dirty = true;
            self.fix_selection();
            if self.mode == Mode::Markdown {
                self.reload_markdown();
            }
        }
    }

    pub fn redo(&mut self) {
        self.end_edit();
        if let Some(next) = self.redo.pop() {
            let current = std::mem::replace(&mut self.map, next);
            self.undo.push(current);
            self.dirty = true;
            self.fix_selection();
            if self.mode == Mode::Markdown {
                self.reload_markdown();
            }
        }
    }

    fn fix_selection(&mut self) {
        if self.selected.is_some_and(|id| self.map.get(id).is_none()) {
            self.selected = self.map.root();
        }
    }

    // -- Text bearbeiten ------------------------------------------------------

    /// `record`: Textänderung als eigenen Rückgängig-Schritt merken
    /// (bei frisch angelegten Knoten nicht nötig, das Anlegen ist schon einer).
    pub fn begin_edit(&mut self, id: u64, record: bool) {
        self.end_edit();
        self.selected = Some(id);
        self.editing = Some(id);
        self.edit_before = record.then(|| self.map.clone());
    }

    pub fn end_edit(&mut self) {
        let Some(id) = self.editing.take() else {
            return;
        };
        if let Some(node) = self.map.get_mut(id) {
            if node.text.trim().is_empty() {
                node.text = DEFAULT_TEXT.into();
            }
        }
        if let Some(before) = self.edit_before.take() {
            if before != self.map {
                self.push_undo(before);
            }
        }
    }
}
