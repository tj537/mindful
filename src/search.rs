//! Search: which nodes match and where in a text the matches are.
//!
//! A node matches when its text, its description or its speaker notes contain the
//! query (ignoring case). Matches are marked in the node labels and in the Markdown editor.

use crate::model::{MindMap, Node};
use eframe::egui::{text::LayoutJob, Color32};
use std::ops::Range;

/// Byte ranges of all matches of `query` in `text`, ignoring case
pub fn ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    // Compare char by char, so byte positions stay those of the original text
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|(_, c)| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= lower.len() {
        if lower[i..i + needle.len()] == needle[..] {
            let start = chars[i].0;
            let end = chars
                .get(i + needle.len())
                .map_or(text.len(), |(pos, _)| *pos);
            out.push(start..end);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Where a node matches
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Hit {
    pub text: bool,
    pub note: bool,
    pub speaker_notes: bool,
}

impl Hit {
    pub fn any(self) -> bool {
        self.text || self.note || self.speaker_notes
    }

    /// Short hint for the status bar, e.g. "in the description"
    pub fn place(self) -> &'static str {
        match (self.text, self.note, self.speaker_notes) {
            (true, ..) => "",
            (false, true, _) => " (in the description)",
            (false, false, true) => " (in the speaker notes)",
            _ => "",
        }
    }
}

pub fn hit(node: &Node, query: &str) -> Hit {
    if query.trim().is_empty() {
        return Hit::default();
    }
    let q = query.trim();
    let has = |s: &str| !ranges(s, q).is_empty();
    Hit {
        text: has(&node.text),
        note: has(&node.note),
        speaker_notes: has(&node.speaker_notes),
    }
}

/// All matching nodes in the order of the outline (as in the Markdown text)
pub fn hits(map: &MindMap, query: &str) -> Vec<u64> {
    let Some(root) = map.root() else {
        return Vec::new();
    };
    // Depth first, children in their order – like reading the outline top to bottom
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if map.get(id).is_some_and(|n| hit(n, query).any()) {
            out.push(id);
        }
        let children: Vec<u64> = map.children(id).map(|n| n.id).collect();
        stack.extend(children.into_iter().rev());
    }
    out
}

/// Gives every match of `query` in the text of `job` the background `color`
pub fn mark(job: &mut LayoutJob, query: &str, color: Color32) {
    let found = ranges(&job.text, query.trim());
    if found.is_empty() {
        return;
    }
    let mut sections = Vec::with_capacity(job.sections.len() + found.len() * 2);
    for section in std::mem::take(&mut job.sections) {
        let range = section.byte_range.clone();
        // Cut points inside this section
        let mut cuts = vec![range.start, range.end];
        for m in &found {
            for p in [m.start, m.end] {
                if p > range.start && p < range.end {
                    cuts.push(p);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for (n, pair) in cuts.windows(2).enumerate() {
            let mut part = section.clone();
            part.byte_range = pair[0]..pair[1];
            if n > 0 {
                part.leading_space = 0.0;
            }
            if found.iter().any(|m| m.start <= pair[0] && pair[1] <= m.end) {
                part.format.background = color;
            }
            sections.push(part);
        }
    }
    job.sections = sections;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ignore_case_and_find_notes() {
        assert_eq!(ranges("Mars and MARS", "mars"), vec![0..4, 9..13]);
        // Byte positions stay right after umlauts
        assert_eq!(ranges("Größe Öl", "öl"), vec![8..11]);
        assert!(ranges("abc", "").is_empty());

        let mut map = MindMap::new("Space");
        let a = map.add_child_text(0, "Mars");
        let b = map.add_child_text(0, "Venus");
        map.get_mut(b).unwrap().note = "Hot like Mars".into();
        let c = map.add_child_text(0, "Moon");
        map.get_mut(c).unwrap().speaker_notes = "Mention mars here".into();
        let d = map.add_child_text(a, "Mars rover");
        // Outline order: a child comes right after its parent
        assert_eq!(hits(&map, "MARS"), vec![a, d, b, c]);
        assert_eq!(
            hit(map.get(b).unwrap(), "mars").place(),
            " (in the description)"
        );
        assert_eq!(
            hit(map.get(c).unwrap(), "mars").place(),
            " (in the speaker notes)"
        );

        let mut job = LayoutJob::simple_singleline(
            "Mars and Moon".into(),
            Default::default(),
            Color32::WHITE,
        );
        mark(&mut job, "and", Color32::RED);
        let marked: Vec<&str> = job
            .sections
            .iter()
            .filter(|s| s.format.background == Color32::RED)
            .map(|s| &job.text[s.byte_range.clone()])
            .collect();
        assert_eq!(marked, vec!["and"]);
    }
}
