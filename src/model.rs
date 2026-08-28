use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_FILE: &str = "project.toml";
pub const SCENE_FILE: &str = "scene.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneStatus {
    Idea,
    Draft,
    VoReady,
    PictureReady,
    Locked,
}

impl Default for SceneStatus {
    fn default() -> Self {
        Self::Idea
    }
}

impl SceneStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Draft => "draft",
            Self::VoReady => "vo_ready",
            Self::PictureReady => "picture_ready",
            Self::Locked => "locked",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "draft" => Self::Draft,
            "vo_ready" => Self::VoReady,
            "picture_ready" => Self::PictureReady,
            "locked" => Self::Locked,
            _ => Self::Idea,
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Self::Idea => Self::Draft,
            Self::Draft => Self::VoReady,
            Self::VoReady => Self::PictureReady,
            Self::PictureReady => Self::Locked,
            Self::Locked => Self::Idea,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Draft => "draft",
            Self::VoReady => "VO ready",
            Self::PictureReady => "pix ready",
            Self::Locked => "locked",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectMeta {
    pub name: String,
    pub default_speaker: String,
    pub created: String,
    pub notes: String,
}

impl ProjectMeta {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_speaker: "Narrator".into(),
            created: today(),
            notes: String::new(),
        }
    }

    pub fn to_ini(&self) -> String {
        format!(
            "name = {}\ndefault_speaker = {}\ncreated = {}\nnotes = {}\n",
            escape_ini(&self.name),
            escape_ini(&self.default_speaker),
            escape_ini(&self.created),
            escape_ini(&self.notes)
        )
    }

    pub fn from_ini(raw: &str) -> Result<Self> {
        let map = parse_ini(raw);
        Ok(Self {
            name: map_get(&map, "name").unwrap_or_else(|| "untitled".into()),
            default_speaker: map_get(&map, "default_speaker").unwrap_or_else(|| "Narrator".into()),
            created: map_get(&map, "created").unwrap_or_default(),
            notes: map_get(&map, "notes").unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SceneMeta {
    pub title: String,
    pub status: SceneStatus,
    pub duration_secs: u32,
    pub notes: String,
}

impl SceneMeta {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: SceneStatus::Idea,
            duration_secs: 0,
            notes: String::new(),
        }
    }

    pub fn to_ini(&self) -> String {
        format!(
            "title = {}\nstatus = {}\nduration_secs = {}\nnotes = {}\n",
            escape_ini(&self.title),
            self.status.as_str(),
            self.duration_secs,
            escape_ini(&self.notes)
        )
    }

    pub fn from_ini(raw: &str) -> Result<Self> {
        let map = parse_ini(raw);
        Ok(Self {
            title: map_get(&map, "title").unwrap_or_else(|| "untitled".into()),
            status: SceneStatus::parse(&map_get(&map, "status").unwrap_or_default()),
            duration_secs: map_get(&map, "duration_secs")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            notes: map_get(&map, "notes").unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DialogueFile {
    pub name: String,
    pub path: PathBuf,
    pub words: usize,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub slug: String,
    pub dir: PathBuf,
    pub meta: SceneMeta,
    pub files: Vec<DialogueFile>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
    pub scenes: Vec<Scene>,
}

impl Project {
    pub fn create(root: &Path, name: &str) -> Result<Self> {
        fs::create_dir_all(root)
            .with_context(|| format!("create project dir {}", root.display()))?;
        let meta = ProjectMeta::new(name);
        fs::write(root.join(PROJECT_FILE), meta.to_ini())?;
        let readme = format!(
            "# {}\n\nPaper-edit project managed by cutboard.\n\nLayout:\n- scene directories (NN_slug)\n- dialogue / notes / shots as .txt files inside each scene\n",
            name
        );
        fs::write(root.join("README.md"), readme)?;
        Self::load(root)
    }

    pub fn load(root: &Path) -> Result<Self> {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let meta_path = root.join(PROJECT_FILE);
        let meta = if meta_path.exists() {
            let raw = fs::read_to_string(&meta_path)?;
            ProjectMeta::from_ini(&raw).context("parse project.toml")?
        } else {
            let name = root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string();
            let meta = ProjectMeta::new(name);
            fs::write(&meta_path, meta.to_ini())?;
            meta
        };
        let mut project = Self {
            root,
            meta,
            scenes: Vec::new(),
        };
        project.reload_scenes()?;
        Ok(project)
    }

    pub fn reload_scenes(&mut self) -> Result<()> {
        let mut scenes = Vec::new();
        let mut entries: Vec<PathBuf> = fs::read_dir(&self.root)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| !n.starts_with('.') && n != "export")
                    .unwrap_or(false)
            })
            .collect();
        entries.sort();
        for dir in entries {
            if let Some(scene) = load_scene(dir)? {
                scenes.push(scene);
            }
        }
        self.scenes = scenes;
        Ok(())
    }

    pub fn next_scene_index(&self) -> u32 {
        self.scenes
            .iter()
            .filter_map(|s| {
                s.slug
                    .split_once('_')
                    .and_then(|(n, _)| n.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn add_scene(&mut self, title: &str) -> Result<usize> {
        let idx = self.next_scene_index();
        let slug = format!("{:02}_{}", idx, slugify(title));
        let dir = self.root.join(&slug);
        fs::create_dir_all(&dir)?;
        let meta = SceneMeta::new(title);
        fs::write(dir.join(SCENE_FILE), meta.to_ini())?;
        fs::write(
            dir.join("narration.txt"),
            default_narration(&self.meta.default_speaker, title),
        )?;
        fs::write(dir.join("shots.txt"), default_shots())?;
        fs::write(dir.join("notes.txt"), default_notes(title))?;
        self.reload_scenes()?;
        Ok(self
            .scenes
            .iter()
            .position(|s| s.slug == slug)
            .unwrap_or(0))
    }

    pub fn delete_scene(&mut self, index: usize) -> Result<()> {
        if let Some(scene) = self.scenes.get(index) {
            fs::remove_dir_all(&scene.dir)?;
        }
        self.reload_scenes()?;
        Ok(())
    }

    pub fn add_file(&mut self, scene_index: usize, name: &str, kind: NewFileKind) -> Result<()> {
        let scene = self
            .scenes
            .get(scene_index)
            .context("no scene selected")?;
        let filename = ensure_txt(name);
        let path = scene.dir.join(&filename);
        if path.exists() {
            anyhow::bail!("{filename} already exists");
        }
        let body = match kind {
            NewFileKind::Dialogue => default_narration(&self.meta.default_speaker, &scene.meta.title),
            NewFileKind::Shots => default_shots(),
            NewFileKind::Notes => default_notes(&scene.meta.title),
            NewFileKind::Comfy => default_comfy(&scene.meta.title),
            NewFileKind::Blank => String::new(),
        };
        fs::write(path, body)?;
        self.reload_scenes()?;
        Ok(())
    }

    pub fn delete_file(&mut self, scene_index: usize, file_index: usize) -> Result<()> {
        let path = self
            .scenes
            .get(scene_index)
            .and_then(|s| s.files.get(file_index))
            .map(|f| f.path.clone())
            .context("no file selected")?;
        fs::remove_file(path)?;
        self.reload_scenes()?;
        Ok(())
    }

    pub fn save_scene_meta(&self, scene_index: usize) -> Result<()> {
        let scene = self.scenes.get(scene_index).context("no scene")?;
        fs::write(scene.dir.join(SCENE_FILE), scene.meta.to_ini())?;
        Ok(())
    }

    pub fn export_full_script(&self) -> Result<PathBuf> {
        let export_dir = self.root.join("export");
        fs::create_dir_all(&export_dir)?;
        let mut out = String::new();
        out.push_str(&format!("# {}\n\n", self.meta.name));
        for scene in &self.scenes {
            out.push_str(&format!(
                "## {} ({}) — {}\n\n",
                scene.slug,
                scene.meta.title,
                scene.meta.status.label()
            ));
            for file in scene.files.iter().filter(|f| is_dialogue_name(&f.name)) {
                let body = fs::read_to_string(&file.path).unwrap_or_default();
                let body = body.trim();
                if body.is_empty() {
                    continue;
                }
                out.push_str(&format!("### {}\n\n{}\n\n", file.name, body));
            }
        }
        let path = export_dir.join("full_script.txt");
        fs::write(&path, out)?;
        Ok(path)
    }

    pub fn export_paper_edit(&self) -> Result<PathBuf> {
        let export_dir = self.root.join("export");
        fs::create_dir_all(&export_dir)?;
        let mut out = String::new();
        out.push_str(&format!("# {} — paper edit\n\n", self.meta.name));
        out.push_str("| Scene | Status | Dur | File | Words |\n|---|---|---|---|---|\n");
        for scene in &self.scenes {
            if scene.files.is_empty() {
                out.push_str(&format!(
                    "| {} ({}) | {} | {}s | — | — |\n",
                    scene.slug,
                    scene.meta.title,
                    scene.meta.status.label(),
                    scene.meta.duration_secs
                ));
            }
            for file in &scene.files {
                out.push_str(&format!(
                    "| {} ({}) | {} | {}s | {} | {} |\n",
                    scene.slug,
                    scene.meta.title,
                    scene.meta.status.label(),
                    scene.meta.duration_secs,
                    file.name,
                    file.words
                ));
            }
        }
        out.push_str("\n## Shots\n\n");
        for scene in &self.scenes {
            if let Some(shots) = scene.files.iter().find(|f| f.name == "shots.txt") {
                let body = fs::read_to_string(&shots.path).unwrap_or_default();
                out.push_str(&format!("### {}\n\n{}\n\n", scene.slug, body.trim()));
            }
        }
        let path = export_dir.join("paper_edit.md");
        fs::write(&path, out)?;
        Ok(path)
    }

    pub fn export_vibevoice_chunks(&self) -> Result<PathBuf> {
        let export_dir = self.root.join("export").join("vibevoice");
        fs::create_dir_all(&export_dir)?;
        for scene in &self.scenes {
            let mut chunk = String::new();
            for file in scene.files.iter().filter(|f| is_dialogue_name(&f.name)) {
                let body = fs::read_to_string(&file.path).unwrap_or_default();
                let body = body.trim();
                if body.is_empty() {
                    continue;
                }
                chunk.push_str(body);
                chunk.push_str("\n\n");
            }
            if !chunk.trim().is_empty() {
                fs::write(export_dir.join(format!("{}.txt", scene.slug)), chunk)?;
            }
        }
        Ok(export_dir)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewFileKind {
    Dialogue,
    Shots,
    Notes,
    Comfy,
    Blank,
}

impl NewFileKind {
    pub fn all() -> &'static [NewFileKind] {
        &[
            NewFileKind::Dialogue,
            NewFileKind::Shots,
            NewFileKind::Notes,
            NewFileKind::Comfy,
            NewFileKind::Blank,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dialogue => "dialogue / narration",
            Self::Shots => "shots / picture sources",
            Self::Notes => "notes",
            Self::Comfy => "Comfy / AI prompt",
            Self::Blank => "blank .txt",
        }
    }

    pub fn default_name(self) -> &'static str {
        match self {
            Self::Dialogue => "narration.txt",
            Self::Shots => "shots.txt",
            Self::Notes => "notes.txt",
            Self::Comfy => "comfy_prompt.txt",
            Self::Blank => "untitled.txt",
        }
    }
}

fn load_scene(dir: PathBuf) -> Result<Option<Scene>> {
    let slug = match dir.file_name().and_then(|s| s.to_str()) {
        Some(s) => s.to_string(),
        None => return Ok(None),
    };
    let meta_path = dir.join(SCENE_FILE);
    let meta = if meta_path.exists() {
        SceneMeta::from_ini(&fs::read_to_string(&meta_path)?)?
    } else {
        let title = slug
            .split_once('_')
            .map(|(_, rest)| rest.replace('_', " "))
            .unwrap_or_else(|| slug.clone());
        let meta = SceneMeta::new(title);
        fs::write(&meta_path, meta.to_ini())?;
        meta
    };
    let mut files: Vec<DialogueFile> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("txt"))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_string();
            let words = fs::read_to_string(&path)
                .map(|s| word_count(&s))
                .unwrap_or(0);
            Some(DialogueFile {
                name,
                path,
                words,
            })
        })
        .collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Some(Scene {
        slug,
        dir,
        meta,
        files,
    }))
}

pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('_');
            prev_dash = true;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "scene".into()
    } else {
        out
    }
}

fn ensure_txt(name: &str) -> String {
    let name = name.trim();
    if name.to_lowercase().ends_with(".txt") {
        name.to_string()
    } else {
        format!("{name}.txt")
    }
}

pub fn is_dialogue_name(name: &str) -> bool {
    !matches!(
        name,
        "shots.txt" | "notes.txt" | "comfy_prompt.txt" | "assets.txt"
    )
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

fn default_narration(speaker: &str, title: &str) -> String {
    format!(
        "[1]: {speaker} — {title}.\n\nWrite the voiceover for this scene here.\nUse [1]: / [2]: labels if VibeVoice multi-speaker.\nPunctuation controls pacing. Use [pause] where needed.\n"
    )
}

fn default_shots() -> String {
    "format: duration | type | source_or_prompt | filename | status\n\n00:08 | clip | SOURCE: describe existing clip | clip_.mp4 | need\n00:05 | ai   | GENERATE: documentary wide shot, 16:9, film grain | ai_.mp4 | need\n00:03 | still| chart / photo / title card | still_.png | need\n".into()
}

fn default_notes(title: &str) -> String {
    format!("Scene: {title}\n\nIntent:\nMusic:\nRights / source notes:\nEdit notes:\n")
}

fn default_comfy(title: &str) -> String {
    format!(
        "scene: {title}\nmodel: (wan / ltx / other)\naspect: 16:9\nlook: documentary, natural light, subtle film grain, muted palette\nnegative: glossy commercial, text, watermark, extra fingers\n\nprompt:\n"
    )
}

fn today() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn parse_ini(raw: &str) -> Vec<(String, String)> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            Some((k.trim().to_string(), unescape_ini(v.trim())))
        })
        .collect()
}

fn map_get(map: &[(String, String)], key: &str) -> Option<String> {
    map.iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

fn escape_ini(s: &str) -> String {
    s.replace('\n', "\\n")
}

fn unescape_ini(s: &str) -> String {
    let s = s.trim_matches('"');
    s.replace("\\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn slugify_titles() {
        assert_eq!(slugify("Hook"), "hook");
        assert_eq!(slugify("The River, 1972"), "the_river_1972");
    }

    #[test]
    fn create_project_layout() {
        let root = env::temp_dir().join(format!("cutboard-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut p = Project::create(&root, "Test Doc").unwrap();
        assert!(root.join(PROJECT_FILE).exists());
        let idx = p.add_scene("Hook").unwrap();
        assert_eq!(idx, 0);
        assert!(root.join("01_hook").is_dir());
        assert!(root.join("01_hook").join("narration.txt").exists());
        assert!(root.join("01_hook").join("shots.txt").exists());
        p.add_file(0, "extra_vo", NewFileKind::Dialogue).unwrap();
        assert!(root.join("01_hook").join("extra_vo.txt").exists());
        let script = p.export_full_script().unwrap();
        assert!(script.exists());
        let _ = fs::remove_dir_all(&root);
    }
}
