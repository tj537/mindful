//! "AI prompt": a small form that produces a prompt for any AI.
//! The AI answers with Markdown in Mindful's format, which can be pasted
//! straight into the Markdown editor.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

/// Scope of the requested mind map
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Scope {
    Short,
    #[default]
    Medium,
    Detailed,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Scope::Short => "Short",
            Scope::Medium => "Medium",
            Scope::Detailed => "Detailed",
        }
    }

    fn instructions(self) -> &'static str {
        match self {
            Scope::Short => "4 main branches with 2–3 bullet points each, hardly any sub-points",
            Scope::Medium => {
                "6 main branches with 3–4 bullet points each, sub-points where they help"
            }
            Scope::Detailed => {
                "8 main branches with 4–6 bullet points each, with sub-points and short notes"
            }
        }
    }
}

/// What is filled in on the form (saved)
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptForm {
    pub topic: String,
    pub language: String,
    pub description: String,
    pub scope: Scope,
    /// The AI picks a slide layout for matching sections (timeline, cycle …)
    pub layouts: bool,
    /// When it was last copied (for the short "Copied" feedback)
    #[serde(skip)]
    pub copied_at: Option<f64>,
}

impl Default for PromptForm {
    fn default() -> Self {
        Self {
            topic: String::new(),
            language: "English".into(),
            description: String::new(),
            scope: Scope::default(),
            layouts: true,
            copied_at: None,
        }
    }
}

pub enum PromptAction {
    Copy,
    NewMarkdownTab,
}

const LANGUAGES: [&str; 6] = [
    "English",
    "Deutsch",
    "Español",
    "Français",
    "Italiano",
    "Português",
];

/// Explains the slide layouts to the AI so it picks the right one for each section
const LAYOUT_GUIDE: &str = r###"
Slide layouts:
Every "## " section also becomes a slide in Mindful's presentation mode. By default its points are shown as a bullet list. When the content of a section has a clear shape, turn it into a diagram by adding a layout tag at the end of the heading line, for example "## The Water Cycle <!-- cycle -->". Pick the layout from what the content is, not for decoration:
- <!-- timeline --> for events or phases in chronological order. 3–6 points; each point is a short date or phase ("1969", "Week 1"), with exactly one sub-point saying what happened.
- <!-- cycle --> for a process that repeats, where the last step leads back to the first. 3–6 stages, each with one short sub-point.
- <!-- pyramid --> for levels that build on each other or a hierarchy. 3–5 points from the narrow top (smallest or most specific) down to the broad base; each may have one sub-point, shown beside the level.
- <!-- funnel --> for something that narrows down step by step, like a selection process or a sales funnel. 3–5 points from the broadest at the top to the most selective at the bottom; each may have one sub-point.
- <!-- compare --> for 2–4 options or categories side by side. Each point is the name of one option, with 2–4 short sub-points.
- <!-- matrix --> for exactly 4 points that form a 2×2 grid, such as strengths, weaknesses, opportunities and threats. Each point has 1–3 sub-points.
- <!-- venn --> for 2 or 3 ideas that overlap. Give each idea as a short label of at most 3 words, then one more point saying what they have in common. No sub-points.
- <!-- number --> for one striking figure. The first point must start with the number, e.g. "13.8 billion years ago: the Big Bang" or "$4.2 million in sales"; up to 3 sub-points or further points become small facts below it.
- <!-- quote --> for a famous quotation. A single point holding the quote, with exactly one sub-point naming who said it. Only use real, well-known quotes and never invent who said them.
- <!-- bars --> for comparing amounts. 3–8 points, each written as "Label: number" with an optional unit, e.g. "Solar: 42 %" or "Germany: 83 million". No sub-points.
- <!-- pie --> for the parts of a whole. 2–6 points written as "Label: number", ideally percentages that add up to 100 %. No sub-points.
- <!-- line --> for a trend over time. 3–8 points written as "Year or period: number", in chronological order. No sub-points.
For the three charts, only use figures you are confident are correct; if you are unsure, choose a different layout.
Use a mix: about half of the sections should get a layout, each layout at most once or twice, and the rest stay plain bullet lists. Never put a layout tag on the "# " heading or on bullet points.
"###;

/// The finished prompt with the inputs at the placeholders
pub fn build_prompt(form: &PromptForm) -> String {
    let topic = form.topic.trim();
    let language = match form.language.trim() {
        "" => "English",
        other => other,
    };
    let description = match form.description.trim() {
        "" => "No further details – use your own judgement about what matters most.",
        other => other,
    };
    let (layout_guide, example_tag) = if form.layouts {
        (LAYOUT_GUIDE, " <!-- timeline -->")
    } else {
        ("", "")
    };
    format!(
        r###"Please create a mind map for the app "Mindful". Write it as Markdown in exactly the format described below, so I can paste it straight into the app.

Topic: {topic}
What it should cover: {description}
Language: write everything in {language}.
Scope: {scope}.

Format rules:
- The first line is "# " followed by the central topic.
- Below it, after a blank line, one short paragraph (1–2 sentences) that introduces the topic. It becomes the subtitle of the title slide.
- Every main branch is a "## " heading, followed by a blank line, a one-sentence note and another blank line.
- Under each heading, the points as a bullet list starting with "- ". Indent sub-points by two spaces. Go at most three levels deep.
- Keep every bullet point short (about 8 words at most) – they appear as boxes in a mind map and on slides.
- If a bullet point needs an explanation, add a single line directly below it, indented two spaces further and without a dash. It becomes a note.
- Speaker notes: after the note of each "## " heading, add a blank line and one line "<!-- notes: … -->" with 1–2 sentences the presenter could say. They are only shown to the speaker, never on the slides.
- No bold or italic text, links, tables, numbered lists, code blocks or emojis.
- Reply with the Markdown only: no introduction, no closing remarks, and do not wrap it in a code block.
{layout_guide}
Example of the format (the example is in English – write your answer in {language}):

# Coffee

How a small bean became the world's favourite drink.

## Origins{example_tag}

Coffee travelled from Ethiopia to the rest of the world.

<!-- notes: Tell the legend of Kaldi, the goat herder whose goats danced after eating the berries. -->

- 1400s
  - Monks in Yemen brew it to stay awake
- 1550s
  - First coffee houses open in Istanbul
- 1650s
  - Coffee reaches Europe

## Brewing

There are many ways to turn beans into coffee.

<!-- notes: Ask the audience how they make their coffee at home. -->

- Espresso
  Hot water pressed through finely ground beans.
- Filter coffee
- French press
"###,
        scope = form.scope.instructions(),
    )
}

/// Small heading above a field
pub fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(text)
            .size(12.0)
            .strong()
            .color(ui.visuals().text_color().gamma_multiply(0.6)),
    );
}

/// Toggle switch instead of a checkbox
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, text: &str, hint: &str) {
    ui.horizontal(|ui| {
        let size = Vec2::new(34.0, 20.0);
        let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
        if resp.clicked() {
            *on = !*on;
            resp.mark_changed();
        }
        let t = ui.ctx().animate_bool_responsive(resp.id, *on);
        let accent = ui.visuals().selection.stroke.color;
        let off = ui.visuals().widgets.inactive.bg_fill;
        let track = off.lerp_to_gamma(accent, t);
        ui.painter().rect_filled(rect, rect.height() / 2.0, track);
        let knob = Pos2::new(
            rect.left() + rect.height() / 2.0 + t * (rect.width() - rect.height()),
            rect.center().y,
        );
        ui.painter()
            .circle_filled(knob, rect.height() / 2.0 - 3.0, Color32::WHITE);
        let label = ui.add(
            egui::Label::new(text)
                .selectable(false)
                .sense(Sense::click()),
        );
        if label.clicked() {
            *on = !*on;
        }
        resp.union(label).on_hover_text(hint);
    });
}

/// Segments of equal width, the selected one coloured
fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) {
    let width = ui.available_width();
    let height = 30.0;
    let (outer, _) = ui.allocate_exact_size(Vec2::new(width, height + 6.0), Sense::hover());
    let visuals = ui.visuals().clone();
    let accent = visuals.selection.stroke.color;
    ui.painter()
        .rect_filled(outer, 9.0, visuals.extreme_bg_color);
    let w = (outer.width() - 6.0) / options.len() as f32;
    for (i, (option, label)) in options.iter().enumerate() {
        let r = Rect::from_min_size(
            outer.min + Vec2::new(3.0 + i as f32 * w, 3.0),
            Vec2::new(w, height),
        );
        let resp = ui.interact(r, ui.id().with(("segment", i)), Sense::click());
        let selected = *value == *option;
        if selected {
            ui.painter().rect_filled(r, 7.0, accent);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(r, 7.0, visuals.text_color().gamma_multiply(0.06));
        }
        let color = if selected {
            Color32::WHITE
        } else {
            visuals.text_color().gamma_multiply(0.75)
        };
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(14.0),
            color,
        );
        if resp.clicked() {
            *value = *option;
        }
    }
}

/// Single-line text field with some room
fn text_field<'a>(text: &'a mut String, hint: &'a str) -> egui::TextEdit<'a> {
    egui::TextEdit::singleline(text)
        .hint_text(hint)
        .margin(Vec2::new(10.0, 7.0))
        .desired_width(f32::INFINITY)
}

/// Contents of the drop-down menu. Returns which button was pressed.
pub fn form_ui(ui: &mut egui::Ui, form: &mut PromptForm) -> Option<PromptAction> {
    let mut action = None;
    let accent = ui.visuals().selection.stroke.color;
    let weak = ui.visuals().text_color().gamma_multiply(0.6);
    let now = ui.input(|i| i.time);
    ui.spacing_mut().item_spacing.y = 6.0;

    // Header: icon in a coloured circle, title and subtitle
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
        ui.painter()
            .circle_filled(r.center(), 19.0, accent.gamma_multiply(0.18));
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            icon::MAGIC_WAND,
            egui::FontId::proportional(19.0),
            accent,
        );
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new("Prompt for your AI").strong().size(17.0));
            ui.label(
                RichText::new("Let ChatGPT, Claude & co. write a mind map in Mindful's format.")
                    .size(12.5)
                    .color(weak),
            );
        });
    });
    ui.add_space(8.0);

    field_label(ui, "TOPIC");
    ui.add(text_field(&mut form.topic, "e.g. Renewable energy"));

    field_label(ui, "LANGUAGE");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        for lang in LANGUAGES {
            let selected = form.language.trim() == lang;
            let (fill, color) = if selected {
                (accent, Color32::WHITE)
            } else {
                (Color32::TRANSPARENT, ui.visuals().text_color())
            };
            let chip = egui::Button::new(RichText::new(lang).size(13.0).color(color))
                .fill(fill)
                .stroke(Stroke::new(
                    1.0_f32,
                    if selected {
                        accent
                    } else {
                        ui.visuals().widgets.noninteractive.bg_stroke.color
                    },
                ))
                .corner_radius(14.0)
                .min_size(Vec2::new(0.0, 26.0));
            if ui.add(chip).clicked() {
                form.language = lang.into();
            }
        }
        let custom = !LANGUAGES.contains(&form.language.trim());
        let mut other = if custom {
            form.language.clone()
        } else {
            String::new()
        };
        let field = ui.add(
            egui::TextEdit::singleline(&mut other)
                .hint_text("Other …")
                .margin(Vec2::new(8.0, 4.0))
                .desired_width(90.0),
        );
        if field.changed() {
            form.language = other;
        }
    });

    field_label(ui, "WHAT SHOULD IT COVER?");
    ui.add(
        egui::TextEdit::multiline(&mut form.description)
            .hint_text("Who is it for? Anything to focus on or leave out?")
            .margin(Vec2::new(10.0, 8.0))
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );

    field_label(ui, "SCOPE");
    segmented(
        ui,
        &mut form.scope,
        &[
            (Scope::Short, Scope::Short.label()),
            (Scope::Medium, Scope::Medium.label()),
            (Scope::Detailed, Scope::Detailed.label()),
        ],
    );
    ui.label(
        RichText::new(form.scope.instructions())
            .size(12.5)
            .color(weak),
    );

    ui.add_space(4.0);
    toggle(
        ui,
        &mut form.layouts,
        "Use slide layouts",
        "The AI turns suitable sections into timelines, cycles, pyramids, comparisons and other diagrams",
    );

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);

    // Main button wide and coloured, the second step next to it
    let ready = !form.topic.trim().is_empty();
    let copied = form.copied_at.is_some_and(|t| now - t < 2.0);
    if copied {
        ui.ctx().request_repaint();
    }
    ui.horizontal(|ui| {
        let label = if copied {
            format!("{}  Copied", icon::CHECK)
        } else {
            format!("{}  Copy prompt", icon::COPY)
        };
        let primary = egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(if ready {
                accent
            } else {
                accent.gamma_multiply(0.35)
            })
            .corner_radius(9.0)
            .min_size(Vec2::new(ui.available_width() * 0.5, 36.0));
        let copy = ui
            .add_enabled(ready, primary)
            .on_disabled_hover_text("Enter a topic first");
        if copy.clicked() {
            form.copied_at = Some(now);
            action = Some(PromptAction::Copy);
        }
        let secondary = egui::Button::new(format!("{}  New Markdown tab", icon::CLIPBOARD_TEXT))
            .corner_radius(9.0)
            .stroke(Stroke::new(
                1.0_f32,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .min_size(Vec2::new(ui.available_width(), 36.0));
        if ui
            .add(secondary)
            .on_hover_text("Opens an empty Markdown editor to paste the AI's answer into")
            .clicked()
        {
            action = Some(PromptAction::NewMarkdownTab);
        }
    });

    // The three steps as numbered points
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (i, step) in [
            "Copy the prompt",
            "Paste it into your AI",
            "Paste the answer into a tab",
        ]
        .iter()
        .enumerate()
        {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 9.0, accent.gamma_multiply(0.2));
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                (i + 1).to_string(),
                egui::FontId::proportional(11.0),
                accent,
            );
            ui.label(RichText::new(*step).size(12.5).color(weak));
            ui.add_space(6.0);
        }
    });

    ui.add_space(4.0);
    egui::CollapsingHeader::new(RichText::new("Show prompt").size(13.0).color(weak))
        .id_salt("ai_prompt_preview")
        .show(ui, |ui| {
            egui::Frame::default()
                .fill(ui.visuals().extreme_bg_color)
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(build_prompt(form)).monospace().size(11.0));
                        });
                });
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MindMap, SlideLayout};

    #[test]
    fn prompt_contains_the_inputs() {
        let form = PromptForm {
            topic: "Bees".into(),
            language: "Deutsch".into(),
            description: "For a school talk".into(),
            scope: Scope::Short,
            layouts: true,
            copied_at: None,
        };
        let prompt = build_prompt(&form);
        assert!(prompt.contains("Topic: Bees"));
        assert!(prompt.contains("write everything in Deutsch"));
        assert!(prompt.contains("For a school talk"));
        assert!(prompt.contains("4 main branches"));
        assert!(prompt.contains("<!-- cycle -->"));
        let plain = build_prompt(&PromptForm {
            layouts: false,
            ..form
        });
        // No layout comments – speaker notes are fine
        assert!(SlideLayout::ALL
            .iter()
            .all(|l| !plain.contains(&format!("<!-- {} -->", l.name()))));
        assert!(plain.contains("<!-- notes:"));
    }

    #[test]
    fn example_in_the_prompt_is_valid_mindful_markdown() {
        let prompt = build_prompt(&PromptForm::default());
        let example = &prompt[prompt.find("# Coffee").unwrap()..];
        let map = MindMap::from_outline(example).unwrap();
        let root = map.root().unwrap();
        assert_eq!(map.children(root).count(), 2);
        assert_eq!(
            map.children(root).next().unwrap().layout,
            Some(SlideLayout::Timeline)
        );
        assert!(map
            .children(root)
            .all(|b| !b.note.is_empty() && !b.speaker_notes.is_empty()));
        assert_eq!(map.to_markdown(), example);
    }
}
