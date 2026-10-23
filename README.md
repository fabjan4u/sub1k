# Sub-1K Radio (`sub1k`)

[![Crates.io](https://img.shields.io/crates/v/sub1k.svg)](https://crates.io/crates/sub1k)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build & Test](https://github.com/fabjan4u/sub1k/actions/workflows/ci.yml/badge.svg)](https://github.com/fabjan4u/sub1k/actions)

> A terminal player designed to rescue indie tracks from Spotify's <1000 stream demonetization threshold.

---

## ⚡ Quick Install

```bash
cargo install sub1k
```
*(Or download pre-built binaries for Linux, macOS and Windows from [Releases](https://github.com/fabjan4u/sub1k/releases))*

---

## 🎧 Features

* **Terminal UI (Ratatui)**: Clean, Cyberpunk-inspired terminal interface with live playback gauges, underdog queue, and real-time Spotify Connect controller.
* **PKCE OAuth 2.0**: Secure authentication directly with the Spotify Web API. No client secrets required.
* **Intelligent Crate Digger**: Explores obscure tracks using popularity filters (`popularity <= 3`) and hipster discovery tags.
* **Smart Boost Mode**: Automatically skips songs after 35–45 seconds with random jitter. A Spotify stream officially registers at 30 seconds, allowing you to give legitimate streams to dozens of underground artists while scanning new music.
* **Impact Tracking (Rescued Counter)**: Locally tracks every single track you have saved and rescued from demonetization (`~/.config/sub1k/rescued_stats.json`).
* **Featured Seed Support**: Creator's picks (such as *THE SILICON RITE*) are openly supported with full transparency and `--no-featured` opt-out.

---

## ⌨️ TUI Keybindings

| Key | Action | Description |
| :---: | :--- | :--- |
| `Space` | **Play / Pause** | Toggle playback on your active Spotify Connect device |
| `n` | **Rescue Next** | Skip to and start the next underground track in queue |
| `s` | **Save Track (Rescue)** | Adds current track to your *Liked Songs* (boosts Spotify's algorithm) |
| `f` | **Follow Artist** | Follows the active artist on Spotify directly |
| `b` | **Toggle Smart Boost** | Auto-skips tracks after 35–45s with randomized jitter |
| `g` | **Change Genre** | Opens an inline modal to dig a new genre (e.g. `dark ambient`, `post-punk`) |
| `d` | **Device Picker** | Select which Spotify Connect device to stream to |
| `↑` / `↓` | **Navigate Queue** | Browse up and down through the discovered tracklist |
| `Enter` | **Play Selected** | Play the highlighted track in the queue |
| `q` | **Quit** | Exit Sub-1K Radio |

---

## 🚀 Quickstart

### 1. Build and Run
```bash
cargo build --release
./target/release/sub1k
```
*(Or install globally with `cargo install --path .` and run `sub1k` from anywhere!)*

### 2. Configure Client ID (First time only)
```bash
sub1k setup <YOUR_SPOTIFY_CLIENT_ID>
sub1k auth
```

### 3. CLI Commands
```bash
sub1k                 # Launch interactive TUI
sub1k radio           # Launch interactive TUI
sub1k dig --genre "synthwave" --limit 10  # CLI batch search
sub1k status          # Check connection & active playback
sub1k devices         # List all Spotify Connect devices
sub1k play <TRACK_ID> # Play a specific track
sub1k rescue <TRACK_ID> # Save a track to Liked Songs
```
