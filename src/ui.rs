use crate::app::{App, Focus, InputKind, Mode};
use crate::model::NewFileKind;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

const BG: Color = Color::Rgb(18, 18, 20);
const PANEL: Color = Color::Rgb(28, 28, 32);
const ACCENT: Color = Color::Rgb(220, 180, 90);
const MUTED: Color = Color::Rgb(140, 140, 148);
const TEXT: Color = Color::Rgb(230, 230, 225);
const OK: Color = Color::Rgb(140, 190, 130);
const WARN: Color = Color::Rgb(210, 140, 90);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.size();
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(body[0]);

    draw_scenes(frame, app, cols[0]);
    draw_files(frame, app, cols[1]);
    draw_preview(frame, app, body[1]);
    draw_status(frame, app, chunks[2]);

    match &app.mode {
        Mode::Help => draw_help(frame, area),
        Mode::Message(msg) => draw_modal(frame, area, "export", msg),
        Mode::ConfirmDelete { target } => {
            let msg = match target {
                crate::app::DeleteTarget::Scene(i) => {
                    let name = app
                        .project
                        .scenes
                        .get(*i)
                        .map(|s| s.slug.as_str())
                        .unwrap_or("scene");
                    format!("Delete scene {name} and all of its files?\n\ny confirm   n / Esc cancel")
                }
                crate::app::DeleteTarget::File { scene, file } => {
                    let name = app
                        .project
                        .scenes
                        .get(*scene)
                        .and_then(|s| s.files.get(*file))
                        .map(|f| f.name.as_str())
                        .unwrap_or("file");
                    format!("Delete {name}?\n\ny confirm   n / Esc cancel")
                }
            };
            draw_modal(frame, area, "confirm delete", &msg);
        }
        Mode::Input { kind, buffer } => draw_input(frame, area, app, *kind, buffer),
        Mode::Edit { .. } | Mode::Browse => {}
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " cutboard  ·  {}  ·  {}",
        app.project.meta.name,
        app.project.root.display()
    );
    let p = Paragraph::new(title)
        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(70, 70, 76)))
                .style(Style::default().bg(PANEL).fg(TEXT)),
        );
    frame.render_widget(p, area);
}

fn draw_scenes(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Scenes && matches!(app.mode, Mode::Browse);
    let items: Vec<ListItem> = if app.project.scenes.is_empty() {
        vec![ListItem::new(Span::styled(
            "  no scenes yet — press n",
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        ))]
    } else {
        app.project
            .scenes
            .iter()
            .enumerate()
            .map(|(i, scene)| {
                let selected = i == app.scene_idx;
                let marker = if selected { "▸" } else { " " };
                let status_style = match scene.meta.status {
                    crate::model::SceneStatus::Idea => Style::default().fg(MUTED),
                    crate::model::SceneStatus::Draft => Style::default().fg(TEXT),
                    crate::model::SceneStatus::VoReady => Style::default().fg(OK),
                    crate::model::SceneStatus::PictureReady => Style::default().fg(ACCENT),
                    crate::model::SceneStatus::Locked => Style::default().fg(WARN),
                };
                let dur = if scene.meta.duration_secs > 0 {
                    format!(" {}s", scene.meta.duration_secs)
                } else {
                    String::new()
                };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{marker} {}  ", scene.slug),
                        if selected {
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(TEXT)
                        },
                    ),
                    Span::styled(scene.meta.status.label(), status_style),
                    Span::styled(dur, Style::default().fg(MUTED)),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let block = Block::default()
        .title(" scenes ")
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(ACCENT)
        } else {
            Style::default().fg(Color::Rgb(70, 70, 76))
        })
        .style(Style::default().bg(PANEL).fg(TEXT));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_files(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Files && matches!(app.mode, Mode::Browse);
    let title = app
        .current_scene()
        .map(|s| format!(" files · {} ", s.meta.title))
        .unwrap_or_else(|| " files ".into());

    let items: Vec<ListItem> = match app.current_scene() {
        None => vec![ListItem::new(Span::styled(
            "  select or create a scene",
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        ))],
        Some(scene) if scene.files.is_empty() => vec![ListItem::new(Span::styled(
            "  no .txt files — press N",
            Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
        ))],
        Some(scene) => scene
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let selected = i == app.file_idx;
                let marker = if selected { "▸" } else { " " };
                let kind = file_kind_label(&f.name);
                let line = Line::from(vec![
                    Span::styled(
                        format!("{marker} {}  ", f.name),
                        if selected {
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(TEXT)
                        },
                    ),
                    Span::styled(format!("{kind}  {}w", f.words), Style::default().fg(MUTED)),
                ]);
                ListItem::new(line)
            })
            .collect(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(ACCENT)
        } else {
            Style::default().fg(Color::Rgb(70, 70, 76))
        })
        .style(Style::default().bg(PANEL).fg(TEXT));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    let editing = matches!(app.mode, Mode::Edit { .. });
    let title = if editing {
        " editor  ·  Ctrl-S save  ·  Esc cancel "
    } else {
        " preview "
    };
    let mut text = app.preview_text();
    if let Mode::Edit { cursor, .. } = &app.mode {
        // Insert a block cursor marker so the user can see position.
        let mut c = *cursor;
        if c > text.len() {
            c = text.len();
        }
        while c < text.len() && !text.is_char_boundary(c) {
            c += 1;
        }
        text.insert(c, '▎');
    }
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(TEXT))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(if editing {
                    Style::default().fg(OK)
                } else {
                    Style::default().fg(Color::Rgb(70, 70, 76))
                })
                .style(Style::default().bg(PANEL).fg(TEXT)),
        );
    frame.render_widget(p, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let help = " n scene  N file  e edit  E $EDITOR  s status  t duration  x export  d delete  r reload  ? help  q quit ";
    let line = Line::from(vec![
        Span::styled(format!(" {}  ", app.status), Style::default().fg(TEXT)),
        Span::styled(help, Style::default().fg(MUTED)),
    ]);
    let p = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(70, 70, 76)))
            .style(Style::default().bg(PANEL)),
    );
    frame.render_widget(p, area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let body = "\
cutboard — paper-edit TUI for mini-docs

Layout on disk
  project/                 project.toml
    01_hook/               scene.toml + *.txt
      narration.txt        VibeVoice / dialogue
      shots.txt            clip | AI | still list
      notes.txt            intent, rights, music
      comfy_prompt.txt     image/video gen notes

Keys
  Tab / ← →     scenes ↔ files
  j k / arrows  move
  n             new scene directory
  N             new .txt in current scene
  e / Enter     edit file in TUI
  E or o        open in $VISUAL / $EDITOR / nano
  s             cycle scene status
  t             set estimated duration (seconds)
  x             export full_script.txt, paper_edit.md, vibevoice/
  d             delete selected scene or file
  r             reload from disk
  q             quit
  ?             this help

Status path
  idea → draft → VO ready → pix ready → locked

Any key closes this panel.";
    draw_modal(frame, area, "help", body);
}

fn draw_input(frame: &mut Frame, area: Rect, app: &App, kind: InputKind, buffer: &str) {
    let (title, body) = match kind {
        InputKind::NewSceneTitle => (
            "new scene",
            format!("Title:\n{buffer}▎\n\nCreates NN_slug/ with narration.txt, shots.txt, notes.txt"),
        ),
        InputKind::PickFileKind => {
            let mut lines = String::from("Choose file kind (↑↓, Enter):\n\n");
            for (i, k) in NewFileKind::all().iter().enumerate() {
                let mark = if i == app.file_kind_idx { "▸" } else { " " };
                lines.push_str(&format!("{mark} {}\n", k.label()));
            }
            ("new file", lines)
        }
        InputKind::NewFileName { kind } => (
            "filename",
            format!(
                "{}:\n{buffer}▎\n\nSaved as a .txt inside the current scene directory",
                kind.label()
            ),
        ),
        InputKind::Duration => (
            "duration",
            format!("Estimated scene length in seconds:\n{buffer}▎"),
        ),
    };
    draw_modal(frame, area, title, &body);
}

fn draw_modal(frame: &mut Frame, area: Rect, title: &str, body: &str) {
    let popup = centered(area, 70, 70);
    frame.render_widget(Clear, popup);
    let p = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(TEXT))
        .block(
            Block::default()
                .title(format!(" {title} "))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .style(Style::default().bg(Color::Rgb(22, 22, 26))),
        );
    frame.render_widget(p, popup);
}

fn centered(area: Rect, pct_x: u16, pct_y: u16) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(popup)[1]
}

fn file_kind_label(name: &str) -> &'static str {
    match name {
        "shots.txt" | "assets.txt" => "shots",
        "notes.txt" => "notes",
        "comfy_prompt.txt" => "comfy",
        _ => "dialogue",
    }
}
