//! „AI prompt“: ein kleines Formular, aus dem ein Prompt für eine beliebige KI
//! entsteht. Die KI antwortet mit Markdown im Format von Mindful, das man
//! direkt in den Markdown-Editor einfügen kann.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

/// Umfang der gewünschten Mindmap
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

/// Was im Formular eingetragen ist (wird gespeichert)
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptForm {
    pub topic: String,
    pub language: String,
    pub description: String,
    pub scope: Scope,
    /// Am Ende eine Checkliste anhängen lassen
    pub checklist: bool,
}

impl Default for PromptForm {
    fn default() -> Self {
        Self {
            topic: String::new(),
            language: "English".into(),
            description: String::new(),
            scope: Scope::default(),
            checklist: false,
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

/// Der fertige Prompt mit den Eingaben an den Platzhaltern
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
    let checklist = if form.checklist {
        "- End with one more \"## \" section: a practical checklist whose points start with \"- [ ] \".\n"
    } else {
        ""
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
{checklist}- No bold or italic text, links, tables, numbered lists, code blocks or emojis.
- Reply with the Markdown only: no introduction, no closing remarks, and do not wrap it in a code block.

Example of the format (the example is in English – write your answer in {language}):

# Coffee

How a small bean became the world's favourite drink.

## Origins

Coffee was first brewed in Ethiopia and Yemen.

- Discovered in Ethiopia
  Legend says a goat herder noticed his goats dancing.
- Spread through the Arab world
  - First coffee houses in Mecca

## Brewing

There are many ways to turn beans into coffee.

- Espresso
- Filter coffee
- French press
"###,
        scope = form.scope.instructions(),
    )
}

/// Inhalt des Aufklappmenüs. Gibt zurück, welcher Knopf gedrückt wurde.
pub fn form_ui(ui: &mut egui::Ui, form: &mut PromptForm) -> Option<PromptAction> {
    let mut action = None;
    ui.label(
        RichText::new(format!("{}  Prompt for your AI", icon::MAGIC_WAND))
            .strong()
            .size(16.0),
    );
    ui.weak("Describe your topic. Mindful builds a prompt that makes ChatGPT, Claude & co. answer in the right Markdown format.");
    ui.add_space(6.0);

    egui::Grid::new("ai_prompt_form")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Topic");
            ui.add(
                egui::TextEdit::singleline(&mut form.topic)
                    .hint_text("e.g. Renewable energy")
                    .desired_width(f32::INFINITY),
            );
            ui.end_row();

            ui.label("Language");
            ui.vertical(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut form.language)
                        .hint_text("e.g. English")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for lang in LANGUAGES {
                        if ui
                            .selectable_label(form.language.trim() == lang, lang)
                            .clicked()
                        {
                            form.language = lang.into();
                        }
                    }
                });
            });
            ui.end_row();

            ui.label("Description");
            ui.add(
                egui::TextEdit::multiline(&mut form.description)
                    .hint_text(
                        "What should it cover? Who is it for? Anything to focus on or leave out?",
                    )
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            );
            ui.end_row();

            ui.label("Scope");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    for scope in [Scope::Short, Scope::Medium, Scope::Detailed] {
                        ui.selectable_value(&mut form.scope, scope, scope.label());
                    }
                });
                ui.weak(form.scope.instructions());
            });
            ui.end_row();

            ui.label("");
            ui.checkbox(&mut form.checklist, "Add a checklist at the end");
            ui.end_row();
        });

    ui.add_space(8.0);
    let ready = !form.topic.trim().is_empty();
    ui.horizontal(|ui| {
        let copy = ui
            .add_enabled(
                ready,
                egui::Button::new(RichText::new(format!("{}  Copy prompt", icon::COPY)).strong()),
            )
            .on_disabled_hover_text("Enter a topic first");
        if copy.clicked() {
            action = Some(PromptAction::Copy);
        }
        if ui
            .button(format!("{}  New Markdown tab", icon::CLIPBOARD_TEXT))
            .on_hover_text("Opens an empty Markdown editor to paste the AI's answer into")
            .clicked()
        {
            action = Some(PromptAction::NewMarkdownTab);
        }
    });
    ui.add_space(4.0);
    ui.weak("1. Copy the prompt   2. Paste it into your AI   3. Paste the answer into a new Markdown tab");

    egui::CollapsingHeader::new("Show prompt")
        .id_salt("ai_prompt_preview")
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(build_prompt(form)).monospace().size(11.0));
                });
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MindMap;

    #[test]
    fn prompt_contains_the_inputs() {
        let form = PromptForm {
            topic: "Bees".into(),
            language: "Deutsch".into(),
            description: "For a school talk".into(),
            scope: Scope::Short,
            checklist: true,
        };
        let prompt = build_prompt(&form);
        assert!(prompt.contains("Topic: Bees"));
        assert!(prompt.contains("write everything in Deutsch"));
        assert!(prompt.contains("For a school talk"));
        assert!(prompt.contains("4 main branches"));
        assert!(prompt.contains("- [ ] "));
        assert!(!build_prompt(&PromptForm::default()).contains("checklist"));
    }

    #[test]
    fn example_in_the_prompt_is_valid_mindful_markdown() {
        let prompt = build_prompt(&PromptForm::default());
        let example = &prompt[prompt.find("# Coffee").unwrap()..];
        let map = MindMap::from_outline(example).unwrap();
        assert_eq!(map.children(map.root().unwrap()).count(), 2);
        assert_eq!(map.to_markdown(), example);
    }
}
