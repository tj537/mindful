use crate::icons;
use base64::Engine;
use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

pub const DEFAULT_TEXT: &str = "New idea";
/// Text of the centre node in a new, empty tab
pub const ROOT_TEXT: &str = "Central topic";

/// Does the node still hold the placeholder? Then clicking it starts editing right away.
pub fn is_placeholder(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t == DEFAULT_TEXT || t == ROOT_TEXT
}

/// Line height for automatic arranging
const ROW: f32 = 54.0;
/// Horizontal distance between parent and child nodes when arranging
const GAP_X: f32 = 60.0;

/// How a slide is shown (for main branches). Without a layout: bullet list.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SlideLayout {
    Timeline,
    Cycle,
    Pyramid,
    Funnel,
    Compare,
    Matrix,
    Venn,
    Number,
    Quote,
    /// Bar chart made of points like "Solar: 42 %"
    Bars,
    /// Pie or donut chart
    Pie,
    /// Line chart
    Line,
}

impl SlideLayout {
    pub const ALL: [SlideLayout; 12] = [
        SlideLayout::Timeline,
        SlideLayout::Cycle,
        SlideLayout::Pyramid,
        SlideLayout::Funnel,
        SlideLayout::Compare,
        SlideLayout::Matrix,
        SlideLayout::Venn,
        SlideLayout::Number,
        SlideLayout::Quote,
        SlideLayout::Bars,
        SlideLayout::Pie,
        SlideLayout::Line,
    ];

    /// Name in the Markdown comment, e.g. `<!-- timeline -->`
    pub fn name(self) -> &'static str {
        match self {
            SlideLayout::Timeline => "timeline",
            SlideLayout::Cycle => "cycle",
            SlideLayout::Pyramid => "pyramid",
            SlideLayout::Funnel => "funnel",
            SlideLayout::Compare => "compare",
            SlideLayout::Matrix => "matrix",
            SlideLayout::Venn => "venn",
            SlideLayout::Number => "number",
            SlideLayout::Quote => "quote",
            SlideLayout::Bars => "bars",
            SlideLayout::Pie => "pie",
            SlideLayout::Line => "line",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|l| l.name().eq_ignore_ascii_case(name.trim()))
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Node {
    pub id: u64,
    pub text: String,
    /// Longer text for the node (Markdown paragraphs, presentation note)
    #[serde(default)]
    pub note: String,
    /// Position relative to the centre of the canvas
    pub pos: [f32; 2],
    pub parent: Option<u64>,
    #[serde(default)]
    pub collapsed: bool,
    /// Own colour (index into the colour scheme's branch colours), inherited by sub-points
    #[serde(default)]
    pub color: Option<usize>,
    /// Checkbox: `None` = none, `Some(false)` = open, `Some(true)` = done
    #[serde(default)]
    pub task: Option<bool>,
    /// Slide layout (only takes effect on main branches)
    #[serde(default)]
    pub layout: Option<SlideLayout>,
    /// Picture for the node: key into `MindMap::images`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Icon before the text (short name from `icons::ICONS`, e.g. "rocket")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// An inserted picture, already scaled down and encoded (JPEG or PNG).
/// It is saved inside the file so the file doesn't need a picture folder.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct StoredImage {
    pub width: u32,
    pub height: u32,
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

impl StoredImage {
    pub fn aspect(&self) -> f32 {
        self.width.max(1) as f32 / self.height.max(1) as f32
    }
}

/// Save image data as Base64 text instead of a list of numbers
mod base64_bytes {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(text.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

/// Start of the block with the image data at the end of a Markdown file
const IMAGES_START: &str = "<!-- mindful-images";

impl Node {
    fn new(id: u64, text: &str, pos: Vec2, parent: Option<u64>) -> Self {
        Self {
            id,
            text: text.into(),
            note: String::new(),
            pos: [pos.x, pos.y],
            parent,
            collapsed: false,
            color: None,
            task: None,
            layout: None,
            image: None,
            icon: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MindMap {
    pub nodes: Vec<Node>,
    pub next_id: u64,
    /// Pictures of the nodes. `Arc` so the undo snapshots don't copy them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, Arc<StoredImage>>,
}

/// Clipboard contents when copying a subtree
#[derive(Serialize, Deserialize)]
pub struct Clip {
    pub mindmap_clip: Vec<Node>,
    #[serde(default)]
    pub images: BTreeMap<String, Arc<StoredImage>>,
}

/// One entry of an outline (from Markdown or indented text)
#[derive(Debug, PartialEq)]
pub struct OutlineItem {
    pub level: usize,
    pub text: String,
    pub note: String,
    pub task: Option<bool>,
    pub layout: Option<SlideLayout>,
    pub image: Option<String>,
    pub icon: Option<String>,
}

impl MindMap {
    pub fn new(title: &str) -> Self {
        Self {
            nodes: vec![Node::new(0, title, Vec2::ZERO, None)],
            next_id: 1,
            images: BTreeMap::new(),
        }
    }

    /// Picture of a node, if there is one and it is loaded
    #[cfg(test)]
    pub fn image_of(&self, id: u64) -> Option<(&str, &StoredImage)> {
        let key = self.get(id)?.image.as_deref()?;
        self.images.get(key).map(|img| (key, img.as_ref()))
    }

    /// Pictures that a node still refers to
    fn used_images(&self, nodes: &[Node]) -> BTreeMap<String, Arc<StoredImage>> {
        nodes
            .iter()
            .filter_map(|n| n.image.as_ref())
            .filter_map(|k| self.images.get(k).map(|img| (k.clone(), img.clone())))
            .collect()
    }

    /// Remove pictures that no node uses any more (before saving)
    pub fn prune_images(&mut self) {
        self.images = self.used_images(&self.nodes);
    }

    pub fn root(&self) -> Option<u64> {
        self.nodes.iter().find(|n| n.parent.is_none()).map(|n| n.id)
    }

    pub fn get(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn pos(&self, id: u64) -> Vec2 {
        self.get(id)
            .map(|n| Vec2::new(n.pos[0], n.pos[1]))
            .unwrap_or_default()
    }

    pub fn parent_of(&self, id: u64) -> Option<u64> {
        self.get(id).and_then(|n| n.parent)
    }

    pub fn children(&self, id: u64) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.parent == Some(id))
    }

    pub fn has_children(&self, id: u64) -> bool {
        self.children(id).next().is_some()
    }

    pub fn depth(&self, id: u64) -> usize {
        let mut depth = 0;
        let mut cur = self.parent_of(id);
        while let Some(p) = cur {
            depth += 1;
            cur = self.parent_of(p);
        }
        depth
    }

    /// The main branch (child of the root) the node belongs to.
    pub fn branch_of(&self, id: u64) -> Option<u64> {
        let mut cur = id;
        loop {
            let parent = self.parent_of(cur)?;
            if self.parent_of(parent).is_none() {
                return Some(cur);
            }
            cur = parent;
        }
    }

    /// Invisible if any ancestor is collapsed.
    pub fn is_hidden(&self, id: u64) -> bool {
        let mut cur = self.parent_of(id);
        while let Some(p) = cur {
            if self.get(p).is_some_and(|n| n.collapsed) {
                return true;
            }
            cur = self.parent_of(p);
        }
        false
    }

    /// Expands all ancestors so the node becomes visible.
    pub fn expand_to(&mut self, id: u64) {
        let mut ancestors = Vec::new();
        let mut cur = self.parent_of(id);
        while let Some(p) = cur {
            ancestors.push(p);
            cur = self.parent_of(p);
        }
        for n in &mut self.nodes {
            if ancestors.contains(&n.id) {
                n.collapsed = false;
            }
        }
    }

    /// Collapsing only affects the main branches, expanding affects all nodes.
    pub fn set_all_collapsed(&mut self, collapsed: bool) {
        let root = self.root();
        let targets: Vec<u64> = self
            .nodes
            .iter()
            .filter(|n| {
                !collapsed || (n.parent.is_some() && n.parent == root && self.has_children(n.id))
            })
            .map(|n| n.id)
            .collect();
        for n in &mut self.nodes {
            if targets.contains(&n.id) {
                n.collapsed = collapsed;
            }
        }
    }

    /// Colour index of the node: its own colour, otherwise that of an ancestor,
    /// otherwise the number of its main branch. `None` means the root colour.
    pub fn color_index(&self, id: u64) -> Option<usize> {
        let mut cur = id;
        loop {
            let node = self.get(cur)?;
            if node.color.is_some() {
                return node.color;
            }
            let parent = node.parent?;
            if self.parent_of(parent).is_none() {
                return self.children(parent).position(|n| n.id == cur);
            }
            cur = parent;
        }
    }

    /// Done and total checkboxes below the node (not counting the node itself),
    /// `None` if there are none below it.
    pub fn task_progress(&self, id: u64) -> Option<(usize, usize)> {
        let tasks: Vec<bool> = self
            .subtree(id)
            .into_iter()
            .skip(1)
            .filter_map(|n| self.get(n).and_then(|n| n.task))
            .collect();
        (!tasks.is_empty()).then(|| (tasks.iter().filter(|done| **done).count(), tasks.len()))
    }

    /// The node itself plus all descendants.
    pub fn subtree(&self, id: u64) -> Vec<u64> {
        let mut result = vec![id];
        let mut i = 0;
        while i < result.len() {
            let cur = result[i];
            result.extend(self.children(cur).map(|n| n.id));
            i += 1;
        }
        result
    }

    /// Next node in direction `dir` (for the arrow keys).
    pub fn neighbor(&self, from: u64, dir: Vec2) -> Option<u64> {
        let origin = self.pos(from);
        self.nodes
            .iter()
            .filter(|n| n.id != from && !self.is_hidden(n.id))
            .filter_map(|n| {
                let v = self.pos(n.id) - origin;
                let along = v.dot(dir);
                if along <= 1.0 {
                    return None;
                }
                let across = (v - dir * along).length();
                Some((n.id, along + across * 2.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Where a new child of `parent_id` goes.
    fn next_child_pos(&self, parent_id: u64) -> Vec2 {
        let n = self.children(parent_id).count() as f32;
        let parent_pos = self.pos(parent_id);
        match self.parent_of(parent_id) {
            // Children of the root: spread in a circle around the centre
            None => {
                let angle = n * std::f32::consts::TAU / 6.0;
                let radius = 230.0 + (n / 6.0).floor() * 90.0;
                parent_pos + Vec2::new(angle.cos(), angle.sin()) * radius
            }
            // Deeper levels: away from the grandparent, siblings staggered alternately
            Some(gp) => {
                let away = parent_pos - self.pos(gp);
                let dir = if away.length() > 1.0 {
                    away.normalized()
                } else {
                    Vec2::X
                };
                let side = if (n as u32).is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                let offset = ((n + 1.0) / 2.0).floor() * 55.0 * side;
                parent_pos + dir * 200.0 + dir.rot90() * offset
            }
        }
    }

    pub fn add_child_text(&mut self, parent_id: u64, text: &str) -> u64 {
        let pos = self.next_child_pos(parent_id);
        if let Some(parent) = self.get_mut(parent_id) {
            parent.collapsed = false;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(Node::new(id, text, pos, Some(parent_id)));
        id
    }

    pub fn add_child(&mut self, parent_id: u64) -> u64 {
        self.add_child_text(parent_id, "")
    }

    /// Removes the node together with its subtree.
    pub fn remove(&mut self, id: u64) {
        let doomed = self.subtree(id);
        self.nodes.retain(|n| !doomed.contains(&n.id));
    }

    /// Copy of the subtree, top node first.
    pub fn extract(&self, id: u64) -> Vec<Node> {
        self.subtree(id)
            .into_iter()
            .filter_map(|i| self.get(i).cloned())
            .collect()
    }

    pub fn clip_json(&self, id: u64) -> String {
        let nodes = self.extract(id);
        serde_json::to_string(&Clip {
            images: self.used_images(&nodes),
            mindmap_clip: nodes,
        })
        .unwrap_or_default()
    }

    /// Inserts a copied subtree under `parent`, with new IDs.
    /// Returns the ID of the inserted top node.
    pub fn insert_subtree(&mut self, parent: u64, nodes: Vec<Node>) -> Option<u64> {
        let first = nodes.first()?.clone();
        let delta = self.next_child_pos(parent) - Vec2::new(first.pos[0], first.pos[1]);
        let mut ids = HashMap::new();
        for n in &nodes {
            ids.insert(n.id, self.next_id);
            self.next_id += 1;
        }
        for mut n in nodes {
            let old = n.id;
            n.id = ids[&old];
            n.parent = if old == first.id {
                Some(parent)
            } else {
                n.parent.and_then(|p| ids.get(&p).copied()).or(Some(parent))
            };
            n.pos = [n.pos[0] + delta.x, n.pos[1] + delta.y];
            self.nodes.push(n);
        }
        if let Some(p) = self.get_mut(parent) {
            p.collapsed = false;
        }
        Some(ids[&first.id])
    }

    // -- Automatic layout -----------------------------------------------------

    fn subtree_height(&self, id: u64, sizes: &HashMap<u64, Vec2>) -> f32 {
        // Nodes with a picture are taller than one line
        let own = sizes.get(&id).map_or(ROW, |s| (s.y + 20.0).max(ROW));
        if self.get(id).is_some_and(|n| n.collapsed) {
            return own;
        }
        let sum: f32 = self
            .children(id)
            .map(|c| self.subtree_height(c.id, sizes))
            .sum();
        sum.max(own)
    }

    /// Classic mind map layout: main branches right and left of the centre,
    /// sub-points below each other. `sizes` are the node sizes.
    pub fn auto_layout(&mut self, sizes: &HashMap<u64, Vec2>) {
        let Some(root) = self.root() else { return };
        let width = |id: u64| sizes.get(&id).map_or(90.0, |s| s.x);
        let root_pos = self.pos(root);
        let mut out = HashMap::new();
        out.insert(root, root_pos);

        let kids: Vec<u64> = self.children(root).map(|n| n.id).collect();
        let right = kids.len().div_ceil(2);
        for (side, group) in [(1.0, &kids[..right]), (-1.0, &kids[right..])] {
            let total: f32 = group.iter().map(|k| self.subtree_height(*k, sizes)).sum();
            let mut y = root_pos.y - total / 2.0;
            for &k in group {
                let h = self.subtree_height(k, sizes);
                let x = root_pos.x + side * (width(root) / 2.0 + GAP_X * 1.5 + width(k) / 2.0);
                self.place(k, Vec2::new(x, y + h / 2.0), side, sizes, &mut out);
                y += h;
            }
        }

        for n in &mut self.nodes {
            if let Some(p) = out.get(&n.id) {
                n.pos = [p.x, p.y];
            }
        }
    }

    fn place(
        &self,
        id: u64,
        pos: Vec2,
        side: f32,
        sizes: &HashMap<u64, Vec2>,
        out: &mut HashMap<u64, Vec2>,
    ) {
        let width = |id: u64| sizes.get(&id).map_or(90.0, |s| s.x);
        let old = self.pos(id);
        out.insert(id, pos);

        if self.get(id).is_some_and(|n| n.collapsed) {
            // Collapsed sub-points simply move along
            for d in self.subtree(id).into_iter().skip(1) {
                out.insert(d, self.pos(d) + (pos - old));
            }
            return;
        }

        let kids: Vec<u64> = self.children(id).map(|n| n.id).collect();
        let total: f32 = kids.iter().map(|k| self.subtree_height(*k, sizes)).sum();
        let mut y = pos.y - total / 2.0;
        for k in kids {
            let h = self.subtree_height(k, sizes);
            let x = pos.x + side * (width(id) / 2.0 + GAP_X + width(k) / 2.0);
            self.place(k, Vec2::new(x, y + h / 2.0), side, sizes, out);
            y += h;
        }
    }

    // -- Markdown / outlines -------------------------------------------------

    /// Centre as `#`, main branches as `##`, everything below as a bullet list.
    /// Notes are written as a paragraph below their node.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let Some(root) = self.root().and_then(|r| self.get(r)) else {
            return out;
        };
        out.push_str(&format!(
            "# {}{}{}\n",
            task_box(root.task),
            icons::prefix(root.icon.as_deref()),
            root.text
        ));
        push_image(root, None, &mut out);
        push_note(&root.note, None, &mut out);
        for branch in self.children(root.id) {
            out.push_str(&format!(
                "\n## {}{}{}{}\n",
                task_box(branch.task),
                icons::prefix(branch.icon.as_deref()),
                branch.text,
                layout_comment(branch.layout)
            ));
            push_image(branch, None, &mut out);
            push_note(&branch.note, None, &mut out);
            if self.has_children(branch.id) {
                out.push('\n');
                self.markdown_bullets(branch.id, 0, &mut out);
            }
        }
        out
    }

    fn markdown_bullets(&self, id: u64, level: usize, out: &mut String) {
        for child in self.children(id) {
            let indent = "  ".repeat(level);
            out.push_str(&format!(
                "{indent}- {}{}{}\n",
                task_box(child.task),
                icons::prefix(child.icon.as_deref()),
                child.text
            ));
            push_image(child, Some(&format!("{indent}  ")), out);
            push_note(&child.note, Some(&format!("{indent}  ")), out);
            self.markdown_bullets(child.id, level + 1, out);
        }
    }

    /// Markdown plus image data in a comment block at the end – that way an
    /// exported file stays complete, and other programs don't show the block.
    pub fn to_markdown_with_images(&self) -> String {
        let mut out = self.to_markdown();
        let used = self.used_images(&self.nodes);
        if !used.is_empty() {
            out.push_str(&format!("\n{IMAGES_START}\n"));
            for (key, img) in used {
                out.push_str(&format!(
                    "{key} {}x{} {}\n",
                    img.width,
                    img.height,
                    base64::engine::general_purpose::STANDARD.encode(&img.data)
                ));
            }
            out.push_str("-->\n");
        }
        out
    }

    /// Counterpart of `to_markdown_with_images`
    pub fn from_markdown_with_images(text: &str) -> Option<MindMap> {
        let (outline, images) = match text.find(IMAGES_START) {
            Some(start) => (&text[..start], &text[start + IMAGES_START.len()..]),
            None => (text, ""),
        };
        let mut map = MindMap::from_outline(outline)?;
        for line in images.lines() {
            let mut parts = line.split_whitespace();
            let (Some(key), Some(size), Some(data)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let Some((w, h)) = size.split_once('x') else {
                continue;
            };
            let (Ok(width), Ok(height), Ok(data)) = (
                w.parse(),
                h.parse(),
                base64::engine::general_purpose::STANDARD.decode(data),
            ) else {
                continue;
            };
            map.images.insert(
                key.to_string(),
                Arc::new(StoredImage {
                    width,
                    height,
                    data,
                }),
            );
        }
        Some(map)
    }

    /// Attaches an outline below `parent`.
    /// Returns the IDs of the new top-level nodes.
    pub fn add_outline(&mut self, parent: u64, items: &[OutlineItem]) -> Vec<u64> {
        let base = items.iter().map(|i| i.level).min().unwrap_or(0);
        let mut stack: Vec<(usize, u64)> = Vec::new();
        let mut top = Vec::new();
        for item in items {
            let level = item.level - base;
            while stack.last().is_some_and(|(l, _)| *l >= level) {
                stack.pop();
            }
            let p = stack.last().map(|(_, id)| *id).unwrap_or(parent);
            let id = self.add_child_text(p, &item.text);
            if let Some(n) = self.get_mut(id) {
                n.note = item.note.clone();
                n.task = item.task;
                n.layout = item.layout;
                n.image = item.image.clone();
                n.icon = item.icon.clone();
            }
            if stack.is_empty() {
                top.push(id);
            }
            stack.push((level, id));
        }
        top
    }

    /// Builds a mind map from Markdown or indented text.
    pub fn from_outline(text: &str) -> Option<MindMap> {
        let mut map = MindMap::new("");
        map.apply_outline(text);
        (map.nodes.len() > 1 || !map.nodes[0].text.is_empty()).then_some(map)
    }

    /// Applies an (edited) outline to the mind map.
    /// Nodes that already existed keep their position, colour and collapsed state:
    /// they are recognised under the same parent by their text,
    /// otherwise by their order.
    pub fn apply_outline(&mut self, text: &str) {
        let items = parse_outline(text);
        let min = items.iter().map(|i| i.level).min().unwrap_or(0);
        let single_root = items.first().is_some_and(|i| i.level == min)
            && items.iter().filter(|i| i.level == min).count() == 1;
        let (root_text, root_note, root_task, root_image, root_icon, rest) = match items.first() {
            Some(first) if single_root => (
                first.text.clone(),
                first.note.clone(),
                first.task,
                first.image.clone(),
                first.icon.clone(),
                &items[1..],
            ),
            Some(_) => (
                "Mind map".to_string(),
                String::new(),
                None,
                None,
                None,
                &items[..],
            ),
            None => (String::new(), String::new(), None, None, None, &items[..]),
        };

        let images = std::mem::take(&mut self.images);
        let old = std::mem::replace(
            self,
            MindMap {
                nodes: Vec::new(),
                next_id: self.next_id,
                images,
            },
        );
        let mut used = HashSet::new();

        let root = match old.root().and_then(|r| old.get(r)) {
            Some(r) => Node {
                text: root_text,
                note: root_note,
                task: root_task,
                image: root_image,
                icon: root_icon,
                ..r.clone()
            },
            None => {
                let mut n = Node::new(self.next_id, &root_text, Vec2::ZERO, None);
                self.next_id += 1;
                n.note = root_note;
                n.task = root_task;
                n.image = root_image;
                n.icon = root_icon;
                n
            }
        };
        let root_id = root.id;
        used.insert(root_id);
        self.nodes.push(root);

        let base = rest.iter().map(|i| i.level).min().unwrap_or(0);
        let mut stack: Vec<(usize, u64)> = Vec::new();
        for item in rest {
            let level = item.level - base;
            while stack.last().is_some_and(|(l, _)| *l >= level) {
                stack.pop();
            }
            let parent = stack.last().map(|(_, id)| *id).unwrap_or(root_id);
            let index = self.children(parent).count();

            let by_text = old
                .children(parent)
                .find(|n| !used.contains(&n.id) && n.text == item.text);
            let by_index = || {
                old.children(parent)
                    .nth(index)
                    .filter(|n| !used.contains(&n.id))
            };
            let id = match by_text.or_else(by_index).cloned() {
                Some(n) => {
                    let id = n.id;
                    used.insert(id);
                    self.nodes.push(Node {
                        text: item.text.clone(),
                        note: item.note.clone(),
                        task: item.task,
                        layout: item.layout,
                        image: item.image.clone(),
                        icon: item.icon.clone(),
                        parent: Some(parent),
                        ..n
                    });
                    id
                }
                None => {
                    let id = self.add_child_text(parent, &item.text);
                    if let Some(n) = self.get_mut(id) {
                        n.note = item.note.clone();
                        n.task = item.task;
                        n.layout = item.layout;
                        n.image = item.image.clone();
                        n.icon = item.icon.clone();
                    }
                    id
                }
            };
            stack.push((level, id));
        }
    }
}

/// Writes a note as a paragraph. Under headings (`indent == None`)
/// with a blank line before it, under bullets indented directly below.
fn push_note(note: &str, indent: Option<&str>, out: &mut String) {
    if note.trim().is_empty() {
        return;
    }
    if indent.is_none() {
        out.push('\n');
    }
    for line in note.trim_end().lines() {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str(indent.unwrap_or(""));
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
}

/// Picture as its own line `![](key)` directly below the entry
fn push_image(node: &Node, indent: Option<&str>, out: &mut String) {
    let Some(key) = &node.image else { return };
    if indent.is_none() {
        out.push('\n');
    }
    out.push_str(indent.unwrap_or(""));
    out.push_str(&format!("![]({key})\n"));
}

/// Recognises an image line `![description](path)` and returns the path
fn split_image(line: &str) -> Option<String> {
    let rest = line.strip_prefix("![")?.strip_suffix(')')?;
    let (_, target) = rest.split_once("](")?;
    let target = target.trim().trim_matches(['<', '>']);
    (!target.is_empty()).then(|| target.to_string())
}

/// Markdown notation of a checkbox
fn task_box(task: Option<bool>) -> &'static str {
    match task {
        None => "",
        Some(false) => "[ ] ",
        Some(true) => "[x] ",
    }
}

/// Markdown comment for a slide layout
fn layout_comment(layout: Option<SlideLayout>) -> String {
    layout
        .map(|l| format!(" <!-- {} -->", l.name()))
        .unwrap_or_default()
}

/// Splits a layout comment at the end of the line (`<!-- timeline -->`) off the text
fn split_layout(text: &str) -> (Option<SlideLayout>, String) {
    let t = text.trim_end();
    if let (Some(start), true) = (t.rfind("<!--"), t.ends_with("-->")) {
        let name = &t[start + 4..t.len() - 3];
        if let Some(layout) = SlideLayout::from_name(name) {
            return (Some(layout), t[..start].trim_end().to_string());
        }
    }
    (None, t.to_string())
}

/// Splits a checkbox (`[ ]`, `[x]`) off the text
fn split_task(text: &str) -> (Option<bool>, String) {
    let t = text.trim();
    for (prefix, done) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
        if let Some(rest) = t.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with(' ') {
                return (Some(done), rest.trim().to_string());
            }
        }
    }
    (None, t.to_string())
}

enum Line {
    Heading(usize, String, Option<bool>, Option<SlideLayout>),
    Bullet(usize, String, Option<bool>),
    Plain(usize, String),
    Image(String),
    Blank,
}

fn classify(line: &str) -> Line {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Line::Blank;
    }
    if let Some(target) = split_image(trimmed) {
        return Line::Image(target);
    }
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes > 0 && trimmed[hashes..].starts_with(' ') {
        let (layout, rest) = split_layout(&trimmed[hashes..]);
        let (task, text) = split_task(&rest);
        return Line::Heading(hashes, text, task, layout);
    }
    let indent: usize = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(bullet) {
            let (task, text) = split_task(rest);
            return Line::Bullet(indent, text, task);
        }
    }
    if trimmed == "-" || trimmed == "- [ ]" {
        return Line::Bullet(indent, String::new(), (trimmed != "-").then_some(false));
    }
    if let Some((num, rest)) = trimmed.split_once(". ") {
        if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
            let (task, text) = split_task(rest);
            return Line::Bullet(indent, text, task);
        }
    }
    Line::Plain(indent, trimmed.to_string())
}

/// Splits Markdown or indented text into an outline.
///
/// If the text contains headings or bullets, it counts as Markdown:
/// plain lines are then notes for the entry before them. Otherwise every line
/// is an entry and the indentation decides the level.
pub fn parse_outline(text: &str) -> Vec<OutlineItem> {
    let lines: Vec<Line> = text.lines().map(classify).collect();
    let markdown = lines
        .iter()
        .any(|l| matches!(l, Line::Heading(..) | Line::Bullet(..)));

    // Detect the indentation step (2 or 4 spaces, tab …)
    let unit = lines
        .iter()
        .filter_map(|l| match l {
            Line::Bullet(i, ..) if markdown => Some(*i),
            Line::Plain(i, _) if !markdown => Some(*i),
            _ => None,
        })
        .filter(|i| *i > 0)
        .min()
        .unwrap_or(2);

    let item = |level: usize, text: String, task: Option<bool>| {
        let (icon, text) = icons::split_icon(&text);
        OutlineItem {
            level,
            text,
            note: String::new(),
            task,
            layout: None,
            image: None,
            icon,
        }
    };
    let mut items: Vec<OutlineItem> = Vec::new();
    let mut heading_level = 0;
    let mut after_blank = false;
    for line in lines {
        let blank = matches!(line, Line::Blank);
        match line {
            Line::Heading(h, t, task, layout) => {
                heading_level = h;
                items.push(OutlineItem {
                    layout,
                    ..item(h - 1, t, task)
                });
            }
            Line::Bullet(indent, t, task) => {
                items.push(item(heading_level + indent / unit, t, task))
            }
            Line::Plain(indent, t) if !markdown => items.push(item(indent / unit, t, None)),
            Line::Plain(_, t) => match items.last_mut() {
                Some(last) => {
                    if !last.note.is_empty() {
                        last.note.push_str(if after_blank { "\n\n" } else { "\n" });
                    }
                    last.note.push_str(&t);
                }
                None => items.push(item(0, t, None)),
            },
            Line::Image(target) => {
                if let Some(last) = items.last_mut() {
                    last.image.get_or_insert(target);
                }
            }
            Line::Blank => {}
        }
        after_blank = blank;
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_round_trip_and_travel_with_the_file() {
        let md = "# Trip\n\n![](map-1.jpg)\n\n## Beach\n\n![](beach.jpg)\n\nSunny.\n\n- Shell\n  ![](shell.png)\n  Found it here.\n";
        let mut map = MindMap::from_outline(md).unwrap();
        assert_eq!(map.to_markdown(), md);
        let beach = map.children(map.root().unwrap()).next().unwrap().id;
        assert_eq!(map.get(beach).unwrap().image.as_deref(), Some("beach.jpg"));
        assert_eq!(map.get(beach).unwrap().note, "Sunny.");

        // Image data travels along in the comment block at the end
        let img = Arc::new(StoredImage {
            width: 4,
            height: 3,
            data: vec![1, 2, 3, 250],
        });
        map.images.insert("beach.jpg".into(), img.clone());
        map.images.insert("unused.jpg".into(), img.clone());
        let file = map.to_markdown_with_images();
        assert!(file.starts_with(md));
        assert!(!file.contains("unused.jpg"));
        let back = MindMap::from_markdown_with_images(&file).unwrap();
        assert_eq!(back.to_markdown(), md);
        assert_eq!(back.images.get("beach.jpg"), Some(&img));

        // … and in the .mindmap file (JSON) as Base64 text
        map.prune_images();
        let json = serde_json::to_string(&map).unwrap();
        assert!(json.contains("\"AQID+g==\""));
        let back: MindMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.images.len(), 1);

        // Copying takes the pictures along
        let clip: Clip = serde_json::from_str(&map.clip_json(beach)).unwrap();
        assert!(clip.images.contains_key("beach.jpg"));
    }

    #[test]
    fn markdown_round_trip() {
        let md = "# Urlaub\n\n## Packliste\n\n- Pass\n  - Kopie\n- Ladekabel\n\n## Route\n";
        let map = MindMap::from_outline(md).unwrap();
        assert_eq!(map.nodes.len(), 6);
        assert_eq!(map.get(map.root().unwrap()).unwrap().text, "Urlaub");
        assert_eq!(map.to_markdown(), md);
    }

    #[test]
    fn notes_round_trip() {
        let md = "# Projekt\n\nWorum es geht.\n\n## Planung\n\nErster Absatz.\n\nZweiter Absatz.\n\n- Budget\n  Grob 5000 Euro\n";
        let map = MindMap::from_outline(md).unwrap();
        let planung = map.nodes.iter().find(|n| n.text == "Planung").unwrap();
        assert_eq!(planung.note, "Erster Absatz.\n\nZweiter Absatz.");
        let budget = map.nodes.iter().find(|n| n.text == "Budget").unwrap();
        assert_eq!(budget.note, "Grob 5000 Euro");
        assert_eq!(map.to_markdown(), md);
    }

    #[test]
    fn checkboxes_round_trip_and_progress() {
        let md =
            "# Trip\n\n## Packing\n\n- [x] Passport\n- [ ] Charger\n- Snacks\n  - [X] Apples\n";
        let map = MindMap::from_outline(md).unwrap();
        let find = |t: &str| map.nodes.iter().find(|n| n.text == t).unwrap();
        assert_eq!(find("Passport").task, Some(true));
        assert_eq!(find("Charger").task, Some(false));
        assert_eq!(find("Snacks").task, None);
        assert_eq!(find("Apples").task, Some(true));
        assert_eq!(map.task_progress(find("Packing").id), Some((2, 3)));
        assert_eq!(map.task_progress(find("Charger").id), None);
        // [X] becomes [x] when written
        assert_eq!(map.to_markdown(), md.replace("[X]", "[x]"));
    }

    #[test]
    fn slide_layouts_in_markdown() {
        let md = "# Space\n\n## Spaceflight <!-- timeline -->\n\n- 1957\n- 1969\n\n## Galaxies\n";
        let map = MindMap::from_outline(md).unwrap();
        let find = |t: &str| map.nodes.iter().find(|n| n.text == t).unwrap();
        assert_eq!(find("Spaceflight").layout, Some(SlideLayout::Timeline));
        assert_eq!(find("Galaxies").layout, None);
        assert_eq!(map.to_markdown(), md);
        // Unknown comments stay part of the text
        let map = MindMap::from_outline("# A\n\n## B <!-- hello -->\n").unwrap();
        assert!(map.nodes.iter().any(|n| n.text == "B <!-- hello -->"));
    }

    #[test]
    fn indented_text_with_four_spaces() {
        let items = parse_outline("Projekt\n    Planung\n        Budget\n    Umsetzung");
        let levels: Vec<usize> = items.iter().map(|i| i.level).collect();
        assert_eq!(levels, vec![0, 1, 2, 1]);
    }

    #[test]
    fn apply_outline_keeps_existing_nodes() {
        let mut map = MindMap::new("Mitte");
        let a = map.add_child_text(0, "A");
        let b = map.add_child_text(0, "B");
        map.get_mut(b).unwrap().color = Some(4);
        map.get_mut(b).unwrap().pos = [123.0, 456.0];

        // As in the editor: changes arrive one after another
        map.apply_outline("# Mitte\n\n## B\n"); // A deleted
        map.apply_outline("# Mitte\n\n## B \n"); // B is renamed …
        map.apply_outline("# Mitte\n\n## B neu\n");
        map.apply_outline("# Mitte\n\n## B neu\n\n## C\n"); // C added
        assert!(map.get(a).is_none());
        let b_node = map.get(b).unwrap();
        assert_eq!(b_node.text, "B neu");
        assert_eq!(b_node.color, Some(4));
        assert_eq!(b_node.pos, [123.0, 456.0]);
        assert_eq!(map.children(0).count(), 2);
    }

    #[test]
    fn paste_subtree_gets_new_ids() {
        let mut map = MindMap::new("Mitte");
        let a = map.add_child_text(0, "A");
        map.add_child_text(a, "A1");
        let clip = map.extract(a);
        let copy = map.insert_subtree(0, clip).unwrap();
        assert_ne!(copy, a);
        assert_eq!(map.nodes.len(), 5);
        assert_eq!(map.children(copy).count(), 1);
        assert_eq!(map.parent_of(copy), Some(0));
    }

    #[test]
    fn auto_layout_puts_branches_left_and_right() {
        let mut map = MindMap::new("Mitte");
        let ids: Vec<u64> = (0..4)
            .map(|i| map.add_child_text(0, &format!("Zweig {i}")))
            .collect();
        map.auto_layout(&HashMap::new());
        assert!(map.pos(ids[0]).x > 0.0 && map.pos(ids[1]).x > 0.0);
        assert!(map.pos(ids[2]).x < 0.0 && map.pos(ids[3]).x < 0.0);
        assert!(map.pos(ids[0]).y < map.pos(ids[1]).y);
    }

    #[test]
    fn collapse_hides_descendants() {
        let mut map = MindMap::new("Mitte");
        let a = map.add_child_text(0, "A");
        let a1 = map.add_child_text(a, "A1");
        map.get_mut(a).unwrap().collapsed = true;
        assert!(map.is_hidden(a1));
        map.expand_to(a1);
        assert!(!map.is_hidden(a1));
    }
}

#[cfg(test)]
mod example_tests {
    use super::*;

    #[test]
    fn templates_parse_and_round_trip() {
        for md in [
            include_str!("../assets/templates/brainstorming.md"),
            include_str!("../assets/templates/project-plan.md"),
            include_str!("../assets/templates/swot.md"),
            include_str!("../assets/templates/weekly-plan.md"),
            include_str!("../assets/templates/meeting-notes.md"),
            include_str!("../assets/templates/book-summary.md"),
        ] {
            let map = MindMap::from_outline(md).unwrap();
            assert!(map.children(map.root().unwrap()).count() >= 4);
            // In the Markdown editor the template appears exactly as it is written
            assert_eq!(map.to_markdown(), md);
        }
    }

    #[test]
    fn universe_example_is_complete() {
        let md = include_str!("../assets/examples/universe.md");
        let map = MindMap::from_outline(md).unwrap();
        let root = map.root().unwrap();
        assert_eq!(map.get(root).unwrap().text, "The Universe");
        let branches: Vec<&Node> = map.children(root).collect();
        assert_eq!(branches.len(), 16);
        // Pictures: gallery, full screen and picture next to the text
        assert_eq!(map.nodes.iter().filter(|n| n.image.is_some()).count(), 5);
        // Every slide layout occurs exactly once, plus normal bullet lists
        for layout in SlideLayout::ALL {
            assert_eq!(
                branches.iter().filter(|b| b.layout == Some(layout)).count(),
                1,
                "{}",
                layout.name()
            );
        }
        // The checklist: 2 of 6 done
        let checklist = branches
            .iter()
            .find(|b| b.text == "Stargazing Checklist")
            .unwrap();
        assert_eq!(map.task_progress(checklist.id), Some((2, 6)));
        assert!(branches
            .iter()
            .all(|b| !b.note.is_empty() && (map.has_children(b.id) || b.image.is_some())));
        assert_eq!(map.to_markdown(), md);
    }

    #[test]
    fn weltall_example_is_complete() {
        let md = include_str!("../assets/examples/weltall.md");
        let map = MindMap::from_outline(md).unwrap();
        let root = map.root().unwrap();
        assert_eq!(map.get(root).unwrap().text, "Das Weltall");
        let branches: Vec<&Node> = map.children(root).collect();
        assert_eq!(branches.len(), 16);
        assert_eq!(map.nodes.iter().filter(|n| n.image.is_some()).count(), 5);
        assert_eq!(
            branches.iter().filter(|b| b.layout.is_some()).count(),
            SlideLayout::ALL.len()
        );
        assert!(branches
            .iter()
            .all(|b| !b.note.is_empty() && (map.has_children(b.id) || b.image.is_some())));
        // Opening it in the Markdown editor gives exactly the same text
        assert_eq!(map.to_markdown(), md);
    }
}
