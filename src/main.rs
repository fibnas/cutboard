mod app;
mod model;
mod ui;

use anyhow::{bail, Context, Result};
use app::{run_external_editor, App};
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

struct Cli {
    path: PathBuf,
    new: bool,
    name: Option<String>,
}

fn parse_args() -> Result<Cli> {
    let mut args = std::env::args().skip(1);
    let mut new = false;
    let mut name = None;
    let mut path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("cutboard 0.1.0");
                std::process::exit(0);
            }
            "--new" => new = true,
            "--name" => {
                name = Some(args.next().context("--name needs a value")?);
            }
            flag if flag.starts_with("--name=") => {
                name = Some(flag.trim_start_matches("--name=").to_string());
            }
            flag if flag.starts_with('-') => bail!("unknown flag: {flag}"),
            other => {
                if path.is_some() {
                    bail!("unexpected extra argument: {other}");
                }
                path = Some(PathBuf::from(other));
            }
        }
    }
    let path = path.context("usage: cutboard [--new] [--name NAME] PROJECT_DIR")?;
    Ok(Cli { path, new, name })
}

fn print_help() {
    eprintln!(
        "cutboard 0.1.0\n\
         Paper-edit TUI: project directory → scene directories → dialogue .txt files\n\n\
         Usage:\n  cutboard [--new] [--name NAME] PROJECT_DIR\n\n\
         Options:\n  --new         create project.toml if missing\n  --name NAME   display name (default: directory name)\n  -h, --help    show this help\n"
    );
}

fn main() -> Result<()> {
    let cli = parse_args()?;
    let mut app = App::open_or_create(&cli.path, cli.new, cli.name)?;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(200)).context("poll")? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key)?;
            }
        }

        if let Some(path) = app.pending_editor.take() {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            let edit_res = run_external_editor(&path);
            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            terminal.clear()?;
            app.project.reload_scenes().ok();
            match edit_res {
                Ok(()) => app.status = format!("edited {}", path.display()),
                Err(err) => app.status = format!("editor error: {err}"),
            }
        }
    }
    Ok(())
}
