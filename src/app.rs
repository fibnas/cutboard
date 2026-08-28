use crate::model::{NewFileKind, Project};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Scenes,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Browse,
    Input { kind: InputKind, buffer: String },
    Edit { path: PathBuf, buffer: String, cursor: usize },
    ConfirmDelete { target: DeleteTarget },
    Help,
    Message(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    NewSceneTitle,
    NewFileName { kind: NewFileKind },
    PickFileKind,
    Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Scene(usize),
    File { scene: usize, file: usize },
}

pub struct App {
    pub project: Project,
    pub focus: Focus,
    pub scene_idx: usize,
    pub file_idx: usize,
    pub mode: Mode,
    pub file_kind_idx: usize,
    pub should_quit: bool,
    pub status: String,
    pub pending_editor: Option<PathBuf>,
}

impl App {
    pub fn open_or_create(path: &Path, create: bool, name: Option<String>) -> Result<Self> {
        let project = if create || !path.join(crate::model::PROJECT_FILE).exists() {
            let name = name.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("untitled")
                    .to_string()
            });
            if create || !path.exists() {
                Project::create(path, &name)?
            } else {
                Project::load(path)?
            }
        } else {
            Project::load(path)?
        };
        let status = format!(
            "{}  —  {} scenes  —  {}",
            project.meta.name,
            project.scenes.len(),
            project.root.display()
        );
        Ok(Self {
            project,
            focus: Focus::Scenes,
            scene_idx: 0,
            file_idx: 0,
            mode: Mode::Browse,
            file_kind_idx: 0,
            should_quit: false,
            status,
            pending_editor: None,
        })
    }

    pub fn current_scene(&self) -> Option<&crate::model::Scene> {
        self.project.scenes.get(self.scene_idx)
    }

    pub fn current_file(&self) -> Option<&crate::model::DialogueFile> {
        self.current_scene()?.files.get(self.file_idx)
    }

    pub fn preview_text(&self) -> String {
        if let Mode::Edit { buffer, .. } = &self.mode {
            return buffer.clone();
        }
        match self.current_file() {
            Some(f) => fs::read_to_string(&f.path).unwrap_or_else(|_| "(unreadable)".into()),
            None => "No .txt file in this scene.\nPress N to add narration, shots, notes, or a Comfy prompt.".into(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode.clone() {
            Mode::Help => {
                self.mode = Mode::Browse;
            }
            Mode::Message(_) => {
                self.mode = Mode::Browse;
            }
            Mode::ConfirmDelete { target } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_delete(target)?,
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.mode = Mode::Browse;
                    self.status = "delete cancelled".into();
                }
                _ => {}
            },
            Mode::Input { kind, mut buffer } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "cancelled".into();
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    self.mode = Mode::Input { kind, buffer };
                }
                KeyCode::Enter => self.submit_input(kind, buffer)?,
                KeyCode::Up if matches!(kind, InputKind::PickFileKind) => {
                    if self.file_kind_idx > 0 {
                        self.file_kind_idx -= 1;
                    }
                }
                KeyCode::Down if matches!(kind, InputKind::PickFileKind) => {
                    if self.file_kind_idx + 1 < NewFileKind::all().len() {
                        self.file_kind_idx += 1;
                    }
                }
                KeyCode::Char(c) if !matches!(kind, InputKind::PickFileKind) => {
                    if !c.is_control() {
                        buffer.push(c);
                        self.mode = Mode::Input { kind, buffer };
                    }
                }
                _ => {}
            },
            Mode::Edit {
                path,
                mut buffer,
                mut cursor,
            } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    self.status = "edit discarded".into();
                }
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    fs::write(&path, &buffer)?;
                    self.project.reload_scenes()?;
                    self.clamp_indices();
                    self.mode = Mode::Browse;
                    self.status = format!("saved {}", path.file_name().unwrap_or_default().to_string_lossy());
                }
                KeyCode::Backspace => {
                    if cursor > 0 {
                        let prev = prev_char_boundary(&buffer, cursor);
                        buffer.replace_range(prev..cursor, "");
                        cursor = prev;
                    }
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::Delete => {
                    if cursor < buffer.len() {
                        let next = next_char_boundary(&buffer, cursor);
                        buffer.replace_range(cursor..next, "");
                    }
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::Left => {
                    cursor = prev_char_boundary(&buffer, cursor);
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::Right => {
                    cursor = next_char_boundary(&buffer, cursor);
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::Home => {
                    cursor = line_start(&buffer, cursor);
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::End => {
                    cursor = line_end(&buffer, cursor);
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::Enter => {
                    buffer.insert(cursor, '\n');
                    cursor += 1;
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    buffer.insert(cursor, c);
                    cursor += c.len_utf8();
                    self.mode = Mode::Edit {
                        path,
                        buffer,
                        cursor,
                    };
                }
                _ => {}
            },
            Mode::Browse => self.handle_browse(key)?,
        }
        Ok(())
    }

    fn handle_browse(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Tab | KeyCode::Right => {
                self.focus = Focus::Files;
            }
            KeyCode::Left => {
                self.focus = Focus::Scenes;
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Char('n') => {
                self.mode = Mode::Input {
                    kind: InputKind::NewSceneTitle,
                    buffer: String::new(),
                };
                self.status = "new scene title — enter to create".into();
            }
            KeyCode::Char('N') => {
                if self.project.scenes.is_empty() {
                    self.status = "create a scene first (n)".into();
                } else {
                    self.file_kind_idx = 0;
                    self.mode = Mode::Input {
                        kind: InputKind::PickFileKind,
                        buffer: String::new(),
                    };
                    self.status = "choose file kind, then name it".into();
                }
            }
            KeyCode::Char('d') => {
                if self.focus == Focus::Scenes {
                    if self.project.scenes.is_empty() {
                        return Ok(());
                    }
                    self.mode = Mode::ConfirmDelete {
                        target: DeleteTarget::Scene(self.scene_idx),
                    };
                } else if self.current_file().is_some() {
                    self.mode = Mode::ConfirmDelete {
                        target: DeleteTarget::File {
                            scene: self.scene_idx,
                            file: self.file_idx,
                        },
                    };
                }
            }
            KeyCode::Char('e') => self.start_edit()?,
            KeyCode::Char('E') | KeyCode::Char('o') => self.open_external_editor()?,
            KeyCode::Char('s') => self.cycle_status()?,
            KeyCode::Char('t') => {
                self.mode = Mode::Input {
                    kind: InputKind::Duration,
                    buffer: self
                        .current_scene()
                        .map(|s| s.meta.duration_secs.to_string())
                        .unwrap_or_default(),
                };
                self.status = "duration in seconds".into();
            }
            KeyCode::Char('r') => {
                self.project.reload_scenes()?;
                self.clamp_indices();
                self.status = "reloaded from disk".into();
            }
            KeyCode::Char('x') => {
                let script = self.project.export_full_script()?;
                let paper = self.project.export_paper_edit()?;
                let chunks = self.project.export_vibevoice_chunks()?;
                self.mode = Mode::Message(format!(
                    "exported\n  {}\n  {}\n  {}/",
                    script.display(),
                    paper.display(),
                    chunks.display()
                ));
                self.status = "export written under export/".into();
            }
            KeyCode::Enter => {
                if self.focus == Focus::Files {
                    self.start_edit()?;
                } else {
                    self.focus = Focus::Files;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn move_sel(&mut self, delta: i32) {
        match self.focus {
            Focus::Scenes => {
                if self.project.scenes.is_empty() {
                    return;
                }
                let len = self.project.scenes.len() as i32;
                self.scene_idx = (self.scene_idx as i32 + delta).rem_euclid(len) as usize;
                self.file_idx = 0;
            }
            Focus::Files => {
                let len = self
                    .current_scene()
                    .map(|s| s.files.len())
                    .unwrap_or(0) as i32;
                if len == 0 {
                    return;
                }
                self.file_idx = (self.file_idx as i32 + delta).rem_euclid(len) as usize;
            }
        }
    }

    fn start_edit(&mut self) -> Result<()> {
        let file = match self.current_file() {
            Some(f) => f.clone(),
            None => {
                self.status = "no file to edit".into();
                return Ok(());
            }
        };
        let buffer = fs::read_to_string(&file.path)?;
        let cursor = buffer.len();
        self.mode = Mode::Edit {
            path: file.path,
            buffer,
            cursor,
        };
        self.status = "editing — Ctrl-S save, Esc cancel".into();
        Ok(())
    }

    fn open_external_editor(&mut self) -> Result<()> {
        let file = match self.current_file() {
            Some(f) => f.clone(),
            None => {
                self.status = "no file to open".into();
                return Ok(());
            }
        };
        self.pending_editor = Some(file.path);
        self.status = "opening $EDITOR".into();
        Ok(())
    }

    fn cycle_status(&mut self) -> Result<()> {
        if let Some(scene) = self.project.scenes.get_mut(self.scene_idx) {
            scene.meta.status = scene.meta.status.cycle();
            let label = scene.meta.status.label().to_string();
            self.project.save_scene_meta(self.scene_idx)?;
            self.status = format!("status → {label}");
        }
        Ok(())
    }

    fn submit_input(&mut self, kind: InputKind, buffer: String) -> Result<()> {
        match kind {
            InputKind::NewSceneTitle => {
                let title = buffer.trim();
                if title.is_empty() {
                    self.mode = Mode::Browse;
                    self.status = "empty title cancelled".into();
                    return Ok(());
                }
                let idx = self.project.add_scene(title)?;
                self.scene_idx = idx;
                self.file_idx = 0;
                self.focus = Focus::Files;
                self.mode = Mode::Browse;
                self.status = format!("created scene {title}");
            }
            InputKind::PickFileKind => {
                let kind = NewFileKind::all()[self.file_kind_idx];
                self.mode = Mode::Input {
                    kind: InputKind::NewFileName { kind },
                    buffer: kind.default_name().to_string(),
                };
                self.status = format!("filename for {}", kind.label());
            }
            InputKind::NewFileName { kind } => {
                let name = buffer.trim();
                if name.is_empty() {
                    self.mode = Mode::Browse;
                    self.status = "empty filename cancelled".into();
                    return Ok(());
                }
                self.project.add_file(self.scene_idx, name, kind)?;
                if let Some(scene) = self.project.scenes.get(self.scene_idx) {
                    let want = if name.to_lowercase().ends_with(".txt") {
                        name.to_string()
                    } else {
                        format!("{name}.txt")
                    };
                    if let Some(i) = scene.files.iter().position(|f| f.name == want) {
                        self.file_idx = i;
                    }
                }
                self.mode = Mode::Browse;
                self.status = format!("created {name}");
            }
            InputKind::Duration => {
                let secs: u32 = buffer.trim().parse().unwrap_or(0);
                if let Some(scene) = self.project.scenes.get_mut(self.scene_idx) {
                    scene.meta.duration_secs = secs;
                }
                self.project.save_scene_meta(self.scene_idx)?;
                self.status = format!("duration {secs}s");
                self.mode = Mode::Browse;
            }
        }
        Ok(())
    }

    fn confirm_delete(&mut self, target: DeleteTarget) -> Result<()> {
        match target {
            DeleteTarget::Scene(i) => {
                let name = self
                    .project
                    .scenes
                    .get(i)
                    .map(|s| s.slug.clone())
                    .unwrap_or_default();
                self.project.delete_scene(i)?;
                self.clamp_indices();
                self.status = format!("deleted scene {name}");
            }
            DeleteTarget::File { scene, file } => {
                let name = self
                    .project
                    .scenes
                    .get(scene)
                    .and_then(|s| s.files.get(file))
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                self.project.delete_file(scene, file)?;
                self.clamp_indices();
                self.status = format!("deleted {name}");
            }
        }
        self.mode = Mode::Browse;
        Ok(())
    }

    fn clamp_indices(&mut self) {
        if self.project.scenes.is_empty() {
            self.scene_idx = 0;
            self.file_idx = 0;
            self.focus = Focus::Scenes;
            return;
        }
        if self.scene_idx >= self.project.scenes.len() {
            self.scene_idx = self.project.scenes.len() - 1;
        }
        let nfiles = self
            .current_scene()
            .map(|s| s.files.len())
            .unwrap_or(0);
        if nfiles == 0 {
            self.file_idx = 0;
        } else if self.file_idx >= nfiles {
            self.file_idx = nfiles - 1;
        }
    }
}

fn prev_char_boundary(s: &str, mut i: usize) -> usize {
    if i == 0 {
        return 0;
    }
    i -= 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    i += 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn line_start(s: &str, i: usize) -> usize {
    s[..i].rfind('\n').map(|n| n + 1).unwrap_or(0)
}

fn line_end(s: &str, i: usize) -> usize {
    s[i..]
        .find('\n')
        .map(|n| i + n)
        .unwrap_or(s.len())
}

pub fn run_external_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "nano".into());
    let status = Command::new(&editor).arg(path).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => anyhow::bail!("{editor} exited {s}"),
        Err(err) => {
            // Fallback: write a hint file rather than fail the TUI hard.
            let _ = std::io::stderr().write_all(
                format!("could not launch {editor}: {err}\n").as_bytes(),
            );
            Ok(())
        }
    }
}
