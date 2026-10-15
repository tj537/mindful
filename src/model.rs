use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_TEXT: &str = "Neuer Gedanke";

/// Zeilenhöhe beim automatischen Anordnen
const ROW: f32 = 54.0;
/// Waagerechter Abstand zwischen Eltern- und Kindknoten beim Anordnen
const GAP_X: f32 = 60.0;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Node {
    pub id: u64,
    pub text: String,
    /// Längerer Text zum Knoten (Markdown-Absätze, Präsentationsnotiz)
    #[serde(default)]
    pub note: String,
    /// Position relativ zur Mitte der Zeichenfläche
    pub pos: [f32; 2],
    pub parent: Option<u64>,
    #[serde(default)]
    pub collapsed: bool,
    /// Eigene Farbe (Index in die Zweigfarben des Farbschemas), vererbt sich an Unterpunkte
    #[serde(default)]
    pub color: Option<usize>,
}

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
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct MindMap {
    pub nodes: Vec<Node>,
    pub next_id: u64,
}

/// Inhalt der Zwischenablage beim Kopieren eines Teilbaums
#[derive(Serialize, Deserialize)]
pub struct Clip {
    pub mindmap_clip: Vec<Node>,
}

/// Ein Eintrag einer Gliederung (aus Markdown oder eingerücktem Text)
#[derive(Debug, PartialEq)]
pub struct OutlineItem {
    pub level: usize,
    pub text: String,
    pub note: String,
}

impl MindMap {
    pub fn new(title: &str) -> Self {
        Self {
            nodes: vec![Node::new(0, title, Vec2::ZERO, None)],
            next_id: 1,
        }
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

    /// Der Hauptzweig (Kind der Wurzel), zu dem der Knoten gehört.
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

    /// Unsichtbar, wenn irgendein Vorfahr eingeklappt ist.
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

    /// Klappt alle Vorfahren auf, damit der Knoten sichtbar wird.
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

    /// Einklappen betrifft nur die Hauptzweige, Ausklappen alle Knoten.
    pub fn set_all_collapsed(&mut self, collapsed: bool) {
        let root = self.root();
        let targets: Vec<u64> = self
            .nodes
            .iter()
            .filter(|n| !collapsed || (n.parent.is_some() && n.parent == root && self.has_children(n.id)))
            .map(|n| n.id)
            .collect();
        for n in &mut self.nodes {
            if targets.contains(&n.id) {
                n.collapsed = collapsed;
            }
        }
    }

    /// Farbindex des Knotens: eigene Farbe, sonst die eines Vorfahren,
    /// sonst die Nummer seines Hauptzweigs. `None` heißt Wurzelfarbe.
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

    /// Der Knoten selbst plus alle Nachfahren.
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

    /// Nächster Knoten in Richtung `dir` (für die Pfeiltasten).
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

    /// Wo ein neues Kind von `parent_id` hinkommt.
    fn next_child_pos(&self, parent_id: u64) -> Vec2 {
        let n = self.children(parent_id).count() as f32;
        let parent_pos = self.pos(parent_id);
        match self.parent_of(parent_id) {
            // Kinder der Wurzel: im Kreis um die Mitte verteilen
            None => {
                let angle = n * std::f32::consts::TAU / 6.0;
                let radius = 230.0 + (n / 6.0).floor() * 90.0;
                parent_pos + Vec2::new(angle.cos(), angle.sin()) * radius
            }
            // Tiefere Ebenen: vom Großelternknoten weg, Geschwister abwechselnd versetzt
            Some(gp) => {
                let away = parent_pos - self.pos(gp);
                let dir = if away.length() > 1.0 { away.normalized() } else { Vec2::X };
                let side = if (n as u32) % 2 == 0 { 1.0 } else { -1.0 };
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

    /// Entfernt den Knoten samt Unterbaum.
    pub fn remove(&mut self, id: u64) {
        let doomed = self.subtree(id);
        self.nodes.retain(|n| !doomed.contains(&n.id));
    }

    /// Kopie des Teilbaums, der oberste Knoten zuerst.
    pub fn extract(&self, id: u64) -> Vec<Node> {
        self.subtree(id)
            .into_iter()
            .filter_map(|i| self.get(i).cloned())
            .collect()
    }

    pub fn clip_json(&self, id: u64) -> String {
        serde_json::to_string(&Clip { mindmap_clip: self.extract(id) }).unwrap_or_default()
    }

    /// Fügt einen kopierten Teilbaum unter `parent` ein, mit neuen IDs.
    /// Gibt die ID des eingefügten obersten Knotens zurück.
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

    // -- Automatisch anordnen -------------------------------------------------

    fn subtree_height(&self, id: u64) -> f32 {
        if self.get(id).is_some_and(|n| n.collapsed) {
            return ROW;
        }
        let sum: f32 = self.children(id).map(|c| self.subtree_height(c.id)).sum();
        sum.max(ROW)
    }

    /// Klassisches Mindmap-Layout: Hauptzweige rechts und links der Mitte,
    /// Unterpunkte untereinander. `widths` sind die Knotenbreiten.
    pub fn auto_layout(&mut self, widths: &HashMap<u64, f32>) {
        let Some(root) = self.root() else { return };
        let width = |id: u64| widths.get(&id).copied().unwrap_or(90.0);
        let root_pos = self.pos(root);
        let mut out = HashMap::new();
        out.insert(root, root_pos);

        let kids: Vec<u64> = self.children(root).map(|n| n.id).collect();
        let right = (kids.len() + 1) / 2;
        for (side, group) in [(1.0, &kids[..right]), (-1.0, &kids[right..])] {
            let total: f32 = group.iter().map(|k| self.subtree_height(*k)).sum();
            let mut y = root_pos.y - total / 2.0;
            for &k in group {
                let h = self.subtree_height(k);
                let x = root_pos.x + side * (width(root) / 2.0 + GAP_X * 1.5 + width(k) / 2.0);
                self.place(k, Vec2::new(x, y + h / 2.0), side, widths, &mut out);
                y += h;
            }
        }

        for n in &mut self.nodes {
            if let Some(p) = out.get(&n.id) {
                n.pos = [p.x, p.y];
            }
        }
    }

    fn place(&self, id: u64, pos: Vec2, side: f32, widths: &HashMap<u64, f32>, out: &mut HashMap<u64, Vec2>) {
        let width = |id: u64| widths.get(&id).copied().unwrap_or(90.0);
        let old = self.pos(id);
        out.insert(id, pos);

        if self.get(id).is_some_and(|n| n.collapsed) {
            // Eingeklappte Unterpunkte wandern einfach mit
            for d in self.subtree(id).into_iter().skip(1) {
                out.insert(d, self.pos(d) + (pos - old));
            }
            return;
        }

        let kids: Vec<u64> = self.children(id).map(|n| n.id).collect();
        let total: f32 = kids.iter().map(|k| self.subtree_height(*k)).sum();
        let mut y = pos.y - total / 2.0;
        for k in kids {
            let h = self.subtree_height(k);
            let x = pos.x + side * (width(id) / 2.0 + GAP_X + width(k) / 2.0);
            self.place(k, Vec2::new(x, y + h / 2.0), side, widths, out);
            y += h;
        }
    }

    // -- Markdown / Gliederungen ---------------------------------------------

    /// Mitte als `#`, Hauptzweige als `##`, alles darunter als Aufzählung.
    /// Notizen stehen als Absatz unter ihrem Knoten.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let Some(root) = self.root().and_then(|r| self.get(r)) else {
            return out;
        };
        out.push_str(&format!("# {}\n", root.text));
        push_note(&root.note, None, &mut out);
        for branch in self.children(root.id) {
            out.push_str(&format!("\n## {}\n", branch.text));
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
            out.push_str(&format!("{indent}- {}\n", child.text));
            push_note(&child.note, Some(&format!("{indent}  ")), out);
            self.markdown_bullets(child.id, level + 1, out);
        }
    }

    /// Hängt eine Gliederung unter `parent`.
    /// Gibt die IDs der obersten neuen Knoten zurück.
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
            }
            if stack.is_empty() {
                top.push(id);
            }
            stack.push((level, id));
        }
        top
    }

    /// Baut eine Mindmap aus Markdown oder eingerücktem Text.
    pub fn from_outline(text: &str) -> Option<MindMap> {
        let mut map = MindMap::new("");
        map.apply_outline(text);
        (map.nodes.len() > 1 || !map.nodes[0].text.is_empty()).then_some(map)
    }

    /// Übernimmt eine (bearbeitete) Gliederung in die Mindmap.
    /// Knoten, die es schon gab, behalten Position, Farbe und Einklapp-Zustand:
    /// Sie werden unter demselben Elternknoten über den Text wiedererkannt,
    /// sonst über ihre Reihenfolge.
    pub fn apply_outline(&mut self, text: &str) {
        let items = parse_outline(text);
        let min = items.iter().map(|i| i.level).min().unwrap_or(0);
        let single_root = items.first().is_some_and(|i| i.level == min)
            && items.iter().filter(|i| i.level == min).count() == 1;
        let (root_text, root_note, rest) = match items.first() {
            Some(first) if single_root => (first.text.clone(), first.note.clone(), &items[1..]),
            Some(_) => ("Mindmap".to_string(), String::new(), &items[..]),
            None => (String::new(), String::new(), &items[..]),
        };

        let old = std::mem::replace(
            self,
            MindMap {
                nodes: Vec::new(),
                next_id: self.next_id,
            },
        );
        let mut used = HashSet::new();

        let root = match old.root().and_then(|r| old.get(r)) {
            Some(r) => Node {
                text: root_text,
                note: root_note,
                ..r.clone()
            },
            None => {
                let mut n = Node::new(self.next_id, &root_text, Vec2::ZERO, None);
                self.next_id += 1;
                n.note = root_note;
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
                        parent: Some(parent),
                        ..n
                    });
                    id
                }
                None => {
                    let id = self.add_child_text(parent, &item.text);
                    if let Some(n) = self.get_mut(id) {
                        n.note = item.note.clone();
                    }
                    id
                }
            };
            stack.push((level, id));
        }
    }
}

/// Schreibt eine Notiz als Absatz. Unter Überschriften (`indent == None`)
/// mit Leerzeile davor, unter Aufzählungen eingerückt direkt darunter.
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

enum Line {
    Heading(usize, String),
    Bullet(usize, String),
    Plain(usize, String),
    Blank,
}

fn classify(line: &str) -> Line {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Line::Blank;
    }
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes > 0 && trimmed[hashes..].starts_with(' ') {
        return Line::Heading(hashes, trimmed[hashes..].trim().to_string());
    }
    let indent: usize = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    for bullet in ["- [ ] ", "- [x] ", "- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(bullet) {
            return Line::Bullet(indent, rest.trim().to_string());
        }
    }
    if let Some((num, rest)) = trimmed.split_once(". ") {
        if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
            return Line::Bullet(indent, rest.trim().to_string());
        }
    }
    Line::Plain(indent, trimmed.to_string())
}

/// Zerlegt Markdown oder eingerückten Text in eine Gliederung.
///
/// Enthält der Text Überschriften oder Aufzählungen, gilt er als Markdown:
/// normale Zeilen sind dann Notizen zum Eintrag davor. Sonst ist jede Zeile
/// ein Eintrag und die Einrückung bestimmt die Ebene.
pub fn parse_outline(text: &str) -> Vec<OutlineItem> {
    let lines: Vec<Line> = text.lines().map(classify).collect();
    let markdown = lines
        .iter()
        .any(|l| matches!(l, Line::Heading(..) | Line::Bullet(..)));

    // Einrückungsschritt erkennen (2 oder 4 Leerzeichen, Tab …)
    let unit = lines
        .iter()
        .filter_map(|l| match l {
            Line::Bullet(i, _) if markdown => Some(*i),
            Line::Plain(i, _) if !markdown => Some(*i),
            _ => None,
        })
        .filter(|i| *i > 0)
        .min()
        .unwrap_or(2);

    let item = |level: usize, text: String| OutlineItem {
        level,
        text,
        note: String::new(),
    };
    let mut items: Vec<OutlineItem> = Vec::new();
    let mut heading_level = 0;
    let mut after_blank = false;
    for line in lines {
        let blank = matches!(line, Line::Blank);
        match line {
            Line::Heading(h, t) => {
                heading_level = h;
                items.push(item(h - 1, t));
            }
            Line::Bullet(indent, t) => items.push(item(heading_level + indent / unit, t)),
            Line::Plain(indent, t) if !markdown => items.push(item(indent / unit, t)),
            Line::Plain(_, t) => match items.last_mut() {
                Some(last) => {
                    if !last.note.is_empty() {
                        last.note.push_str(if after_blank { "\n\n" } else { "\n" });
                    }
                    last.note.push_str(&t);
                }
                None => items.push(item(0, t)),
            },
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

        // Wie im Editor: Änderungen kommen nacheinander an
        map.apply_outline("# Mitte\n\n## B\n"); // A gelöscht
        map.apply_outline("# Mitte\n\n## B \n"); // B wird umbenannt …
        map.apply_outline("# Mitte\n\n## B neu\n");
        map.apply_outline("# Mitte\n\n## B neu\n\n## C\n"); // C dazu
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
        let ids: Vec<u64> = (0..4).map(|i| map.add_child_text(0, &format!("Zweig {i}"))).collect();
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
    fn weltall_example_is_complete() {
        let md = include_str!("../assets/beispiele/weltall.md");
        let map = MindMap::from_outline(md).unwrap();
        let root = map.root().unwrap();
        assert_eq!(map.get(root).unwrap().text, "Das Weltall");
        let branches: Vec<&Node> = map.children(root).collect();
        assert_eq!(branches.len(), 7);
        assert!(branches.iter().all(|b| !b.note.is_empty() && map.has_children(b.id)));
        // Beim Öffnen im Markdown-Editor kommt exakt derselbe Text heraus
        assert_eq!(map.to_markdown(), md);
    }
}
