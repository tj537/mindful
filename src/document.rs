use crate::model::{MindMap, DEFAULT_TEXT};
use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const UNDO_LIMIT: usize = 200;

/// View of a document
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Mindmap,
    Markdown,
    Present,
    /// Camera tour through the mind map
    Tour,
}

impl Mode {
    /// Slides or camera tour: full screen, no bars
    pub fn is_presenting(self) -> bool {
        matches!(self, Mode::Present | Mode::Tour)
    }
}

/// Camera of the tour: which point of the mind map is in the centre and how large
#[derive(Clone, Copy, Default)]
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
}

/// Smooth camera move of the mind map view (e.g. for "Show all")
#[derive(Clone, Copy)]
pub struct ViewAnim {
    pub from_pan: Vec2,
    pub from_zoom: f32,
    pub to_pan: Vec2,
    pub to_zoom: f32,
    pub start: f64,
}

/// State of the camera tour (not saved)
#[derive(Default)]
pub struct TourState {
    /// Current stop (0 = overview)
    pub stop: usize,
    /// Previous stop, for the spotlight transition
    pub prev_stop: usize,
    /// Camera at the start of the current flight
    pub from: Camera,
    /// Start time of the current flight (egui time in seconds)
    pub started: f64,
    /// Paged backwards: show sub-points at once instead of letting them grow
    pub backwards: bool,
    /// The first frame is still pending: set the camera without a flight
    pub needs_init: bool,
    /// When revealing level by level: how many levels below the branch are visible
    pub level: usize,
    /// Camera in the last frame (for warp streaks)
    pub last_cam: Option<(Camera, f64)>,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

/// An open mind map (one tab).
#[derive(Serialize, Deserialize)]
pub struct Document {
    pub map: MindMap,
    pub path: Option<PathBuf>,
    /// Unsaved changes
    pub dirty: bool,
    #[serde(default)]
    pub mode: Mode,

    #[serde(skip)]
    pub selected: Option<u64>,
    #[serde(skip)]
    pub editing: Option<u64>,
    /// Node that should be scrolled into view
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
    /// Bring everything into view on the next draw
    #[serde(skip, default = "yes")]
    pub fit_pending: bool,
    /// Arrange automatically on the next draw
    #[serde(skip)]
    pub layout_pending: bool,
    /// Animate "Show all" smoothly instead of jumping
    #[serde(skip)]
    pub fit_smooth: bool,
    /// Nodes glide from their old positions after arranging (with start time)
    #[serde(skip)]
    pub layout_anim: Option<(HashMap<u64, [f32; 2]>, f64)>,
    #[serde(skip)]
    pub view_anim: Option<ViewAnim>,

    /// Text in the Markdown editor
    #[serde(skip)]
    pub md_text: String,
    /// Search in the Markdown editor: byte position of the current match
    #[serde(skip)]
    pub md_find: Option<usize>,
    /// The editor should scroll to `md_find` in the next frame
    #[serde(skip)]
    pub md_find_scroll: bool,
    /// The Markdown editor should get the focus in the next frame
    #[serde(skip)]
    pub md_focus: bool,
    /// Already created an undo step for this Markdown session?
    #[serde(skip)]
    md_checkpointed: bool,
    /// Current slide in presentation mode
    #[serde(skip)]
    pub slide: usize,
    /// How many bullet groups of the current slide are already visible
    #[serde(skip)]
    pub build: usize,
    /// Time at which the last group was faded in
    #[serde(skip)]
    pub build_time: f64,
    /// Time at which the current slide appeared (for the title effect)
    #[serde(skip)]
    pub slide_time: f64,
    /// Quick jump (key G) open
    #[serde(skip)]
    pub strip: bool,
    /// Slide or stop highlighted in the quick jump
    #[serde(skip)]
    pub strip_cursor: usize,
    #[serde(skip)]
    pub tour: TourState,
    /// Presenter view open in the second window
    #[serde(skip)]
    pub presenter: bool,
    /// Start of the stopwatch in the presenter view
    #[serde(skip)]
    pub presenter_start: Option<f64>,
    /// QR code for the phone remote is shown
    #[serde(skip)]
    pub remote_open: bool,
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
            md_find: None,
            md_find_scroll: false,
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
            remote_open: false,
        }
    }

    /// Opens a .mindmap file or imports Markdown/text.
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
            // Read image references like ![](photo.jpg) next to the file right away
            crate::images::resolve(&mut map, path.parent());
            // Import: without a path, so saving asks for a .mindmap name
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
        // Don't save pictures that no node shows any more
        self.map.prune_images();
        let text = serde_json::to_string_pretty(&self.map).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// File name, otherwise the text of the centre node
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

    // -- Modes ------------------------------------------------------------------

    pub fn set_mode(&mut self, mode: Mode) {
        self.end_edit();
        match mode {
            Mode::Markdown => self.reload_markdown(),
            Mode::Present => {
                // Start at the slide of the selected branch
                let root = self.map.root();
                self.slide = self
                    .selected
                    .and_then(|id| self.map.branch_of(id))
                    .and_then(|b| root.and_then(|r| self.map.children(r).position(|n| n.id == b)))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                self.build = 0;
                self.strip = false;
                // NaN = "just started": the first frame sets the real time,
                // so the title effect also runs on the first slide
                self.slide_time = f64::NAN;
                // The presenter view's stopwatch starts again
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
        // Read image paths typed in the editor (![](~/Pictures/photo.jpg)) when leaving
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

    /// After every change in the Markdown editor: apply the text to the mind map.
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

    // -- Undo / redo ----------------------------------------------------------

    fn push_undo(&mut self, before: MindMap) {
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Call before every change so it can be undone.
    pub fn checkpoint(&mut self) {
        self.end_edit();
        let before = self.map.clone();
        self.push_undo(before);
    }

    /// Records `before` as an undo step if anything has changed since.
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

    // -- Editing text ---------------------------------------------------------

    /// `record`: record the text change as its own undo step
    /// (not needed for freshly created nodes, creating them already is one).
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
