# cutboard

Terminal paper-edit board for mini-documentaries.

You plan the film as **a project directory of scene directories**, and each scene holds **plain `.txt` files** (narration, shots, notes, Comfy prompts). The filesystem is the source of truth — no database.

```
project/
  01_hook/
    narration.txt
    shots.txt
    notes.txt
  02_context/
    ...
```

Built for a Linux edit: clip + AI picture, VibeVoice (via ComfyUI) for VO, Ocenaudio for audio cleanup, Kdenlive (or similar) for the timeline.

## Install

Needs a Rust toolchain (`rustc` 1.75+).

```bash
cd cutboard
cargo run --release -- --new --name "River Doc" ~/Films/river-doc
```

Later sessions:

```bash
cargo run --release -- ~/Films/river-doc
```

Or install the binary onto your PATH:

```bash
cargo install --path .
cutboard --new --name "River Doc" ~/Films/river-doc
```

`--name` is the display title stored in `project.toml`. It defaults to the directory name.

## Keys

| Key | Action |
|---|---|
| `Tab` `←` `→` | Scenes list ↔ files list |
| `j` `k` / arrows | Move |
| `n` | New scene directory (`NN_slug/`) |
| `N` | New `.txt` in the current scene |
| `e` / Enter | Edit the selected file in the TUI |
| `Ctrl-S` | Save in-TUI edit |
| `Esc` | Cancel edit / close modal |
| `E` or `o` | Open the file in `$VISUAL` or `$EDITOR` (falls back to `nano`) |
| `s` | Cycle scene status |
| `t` | Set estimated duration (seconds) |
| `x` | Export script + paper edit + VibeVoice chunks |
| `d` | Delete selected scene or file (confirm with `y`) |
| `r` | Reload from disk |
| `?` | Help |
| `q` | Quit |

Status cycle: **idea → draft → VO ready → pix ready → locked**

## Disk layout

```
~/Films/river-doc/
  project.toml              # name, default speaker
  README.md
  01_hook/
    scene.toml              # title, status, duration
    narration.txt           # VibeVoice / dialogue
    shots.txt               # clip | AI | still list
    notes.txt               # intent, rights, music
    comfy_prompt.txt        # optional, added with N
  02_context/
    ...
  export/                   # written by x, not treated as a scene
    full_script.txt
    paper_edit.md
    vibevoice/
      01_hook.txt
      02_context.txt
```

A new scene is numbered from existing `NN_*` folders and slugified from the title you type (`The River, 1972` → `01_the_river_1972`).

Any `.txt` in a scene folder shows up in the files list. Names matter for export:

| File | Kind in UI | Included in VO export? |
|---|---|---|
| `narration.txt`, `quote.txt`, anything else `.txt` | dialogue | yes |
| `shots.txt`, `assets.txt` | shots | no |
| `notes.txt` | notes | no |
| `comfy_prompt.txt` | comfy | no |

## Suggested workflow

1. `n` a scene per beat of the mini-doc (hook, context, turn, close).
2. Write VO in `narration.txt`. Keep VibeVoice labels if you use them:

   ```
   [1]: Narrator — Hook.

   In 1972 the river still ran clear.
   ```

3. Fill `shots.txt` while you write — one line per picture beat:

   ```
   00:08 | clip | SOURCE: newsreel of the dam | clip_dam.mp4 | need
   00:05 | ai   | GENERATE: wide river, 16:9, grain | ai_river.mp4 | need
   ```

4. When a scene’s VO is locked, hit `s` until **VO ready**. Generate that scene in Comfy (VibeVoice Large). Clean in Ocenaudio. Save WAV next to the scene or in a shared `audio/` folder.
5. Cover the shot list with clips / generations. `s` to **pix ready**.
6. `x` when you want a concatenated script or per-scene files for Comfy **Load Text From File**.
7. Assemble in Kdenlive: lock VO on A1, picture on V1/V2 following `shots.txt`.

`E` is the right way to write longer VO — use Helix, Neovim, or Kate, then `r` if you already jumped back to cutboard.

## Exports (`x`)

All written under `export/` inside the project:

- **`full_script.txt`** — every dialogue file, scene order, with headings.
- **`paper_edit.md`** — table of scenes, status, duration, word counts, plus each `shots.txt`.
- **`vibevoice/NN_slug.txt`** — that scene’s dialogue only. Drop on VibeVoice / Comfy. Keep chunks short if VRAM is tight on a 3090.

`export/` is ignored when scanning for scenes, so you can leave it in the project.

## Metadata files

`project.toml` and `scene.toml` are simple `key = value` files. Safe to edit by hand.

```
# project.toml
name = River Doc
default_speaker = Narrator
created = 2026-08-28
notes =

# scene.toml
title = Hook
status = draft
duration_secs = 45
notes =
```

Valid `status` values: `idea`, `draft`, `vo_ready`, `picture_ready`, `locked`.

## Notes

- Reload (`r`) after anything you change outside the TUI.
- Deleting a scene removes the whole directory.
- This is not an NLE and not a drawing storyboard. It only organizes text so Comfy and Kdenlive have a spine.
