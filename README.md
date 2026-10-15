# Mindful

Mindmaps, Markdown und Präsentationen in einer schlanken Desktop-App – geschrieben in Rust mit [egui](https://github.com/emilk/egui).

![Beispiel-Mindmap „Das Weltall“](docs/beispiel-weltall.png)

## Funktionen

- **Drei Ansichten für dieselbe Mindmap**
  - **Mindmap** – Knoten anlegen, ziehen, einklappen, automatisch anordnen
  - **Markdown-Editor** – `#` Mitte, `##` Hauptzweige, `-` Unterpunkte, Fließtext als Notiz; live synchron mit Vorschau
  - **Präsentation** – Titelfolie plus eine Folie pro Hauptzweig, im Vollbild
- Mehrere Tabs; offene Mindmaps samt ungespeicherter Änderungen sind beim nächsten Start wieder da
- Speichern und Öffnen (`.mindmap`), Markdown- und Text-Import
- Export als PNG, SVG und Markdown
- Rückgängig/Wiederholen, Suche, Zoom, Kopieren/Einfügen, Notizen und eigene Farben pro Knoten
- 8 Farbschemas, darunter Catppuccin, und [Phosphor](https://phosphoricons.com)-Icons
- Beispiel „Das Weltall“ zum Ausprobieren: **Datei → Beispiel öffnen**

## Starten

Voraussetzung ist [Rust](https://rustup.rs).

```bash
cargo run
```

## Mac-App bauen

```bash
./scripts/build-mac.sh
```

Das Ergebnis liegt in `dist/Mindful.app` und lässt sich in den Programme-Ordner und ins Dock ziehen.

## Tastenkürzel (Auswahl)

| Taste | Aktion |
|---|---|
| Tab / Enter | Unterpunkt / Nachbar anlegen |
| F2 · Doppelklick | Umbenennen |
| Leertaste | Ein- und ausklappen |
| ⌘E | Markdown-Editor ein/aus |
| F5 | Präsentation starten |
| ⌘L | Automatisch anordnen |
| ⌘Z / ⇧⌘Z | Rückgängig / Wiederholen |
| F1 | Alle Tastenkürzel |

## Aufbau

| Datei | Inhalt |
|---|---|
| `src/model.rs` | Mindmap-Daten, Anordnen, Markdown-Abgleich |
| `src/document.rs` | Ein Tab: Datei, Modus, Rückgängig |
| `src/app.rs` | Fenster, Menüs, Mindmap-Fläche, Tastatur |
| `src/markdown_view.rs` | Markdown-Editor und Vorschau |
| `src/present.rs` | Präsentationsmodus |
| `src/export.rs` | SVG- und PNG-Export |
| `src/theme.rs` | Farbschemas, Schrift, Widget-Stil |
| `assets/` | App-Icon und Beispiele |
