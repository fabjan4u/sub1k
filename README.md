# Sub-1K Radio (`sub1k`)

[![Crates.io](https://img.shields.io/crates/v/sub1k.svg)](https://crates.io/crates/sub1k)
[![GitHub release (latest by date)](https://img.shields.io/github/v/release/fabjan4u/sub1k?label=release)](https://github.com/fabjan4u/sub1k/releases/latest)
[![Spotify Dark Electronic](https://img.shields.io/badge/Spotify-Dark%20Electronic%20%3C1K-1DB954?style=flat-square&logo=spotify&logoColor=white)](https://open.spotify.com/playlist/1kmZVpw99RovDgeLYfHoPa)
[![Spotify Heavy Deathcore](https://img.shields.io/badge/Spotify-Heavy%20Deathcore%20%3C1K-1DB954?style=flat-square&logo=spotify&logoColor=white)](https://open.spotify.com/playlist/66cUj6IKJvO6UG12bhHxFb)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build & Test](https://github.com/fabjan4u/sub1k/actions/workflows/ci.yml/badge.svg)](https://github.com/fabjan4u/sub1k/actions)

> **A terminal player and crate digger designed to rescue indie tracks from Spotify's <1,000 stream annual demonetization threshold.**

---

### 🎧 Live Spotify Underdog Radios (Updated Weekly)
Stream the latest weekly batch of obscure underground tracks (<1,000 streams) curated by `sub1k-sync`:
* 🎛️ [**Dark Electronic & Industrial Underdogs (<1K)**](https://open.spotify.com/playlist/1kmZVpw99RovDgeLYfHoPa) — *Darksynth, Cyberpunk, Industrial & EBM for deep coding and night focus.*
* 🔨 [**Heavy Deathcore & Beatdown Underdogs (<1K)**](https://open.spotify.com/playlist/66cUj6IKJvO6UG12bhHxFb) — *Brutal underground deathcore, beatdown & slam for heavy gym PRs and lifting.*

---

## ⚡ Quick Install

Pick the method that fits your workflow best:

### Option 1: Direct Binary Download (No Rust or compiler required)

Download the latest pre-compiled binary directly from [**GitHub Releases**](https://github.com/fabjan4u/sub1k/releases/latest):

| Operating System | Architecture | Package | Download |
| :--- | :--- | :--- | :--- |
| 🐧 **Linux** | `x86_64` | `.tar.gz` | [**Download for Linux**](https://github.com/fabjan4u/sub1k/releases/latest/download/sub1k-linux-x86_64.tar.gz) |
| 🪟 **Windows** | `x86_64` | `.zip` (includes `sub1k.exe`) | [**Download for Windows**](https://github.com/fabjan4u/sub1k/releases/latest/download/sub1k-windows-x86_64.zip) |
| 🍏 **macOS** | Apple Silicon (`arm64` / M1/M2/M3/M4) | `.tar.gz` | [**Download for macOS (Apple Silicon)**](https://github.com/fabjan4u/sub1k/releases/latest/download/sub1k-macos-arm64.tar.gz) |

> **Quick one-liner on Linux:**
> ```bash
> curl -sL https://github.com/fabjan4u/sub1k/releases/latest/download/sub1k-linux-x86_64.tar.gz | tar -xz && sudo mv sub1k /usr/local/bin/
> ```

---

### Option 2: Via Cargo / Crates.io (If you have Rust installed)

```bash
cargo install sub1k
```

---

### Option 3: Build from source

```bash
git clone https://github.com/fabjan4u/sub1k.git
cd sub1k
cargo build --release
./target/release/sub1k
```

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

---

## 🤖 `sub1k-sync`: Zero-Maintenance Spotify Auto-Curator

Want to run your own automated radio station on Spotify that refreshes weekly and runs **100% free on GitHub Actions**?

`sub1k-sync` is a headless daemon that scans Spotify for tracks under 1,000 streams across your favourite genres, updates your playlists every Friday morning, and anchors your own artist in the top slots.

### Quick Setup for GitHub Actions:

1. **Fork or clone** this repository.
2. Configure your playlists in `playlists.toml`:
   ```toml
   [settings]
   max_popularity = 3
   playlist_size = 50
   anchor_artist = "THE SILICON RITE"
   anchor_max_position = 4

   [[playlists]]
   name = "Underground Industrial & Dark Synth [<1K Streams]"
   spotify_playlist_id = "YOUR_PLAYLIST_ID"
   genres = ["darksynth", "industrial", "ebm"]
   anchor_track_query = "artist:\"THE SILICON RITE\" RAGNARÖK"
   ```
3. Run the one-time authentication locally to get your `SPOTIFY_REFRESH_TOKEN`:
   ```bash
   cargo run --bin sub1k-sync -- auth --client-id <ID> --client-secret <SECRET>
   ```
4. Add the 3 secrets to your GitHub Repository (**Settings $\rightarrow$ Secrets and variables $\rightarrow$ Actions**):
   * `RSPOTIFY_CLIENT_ID`
   * `RSPOTIFY_CLIENT_SECRET`
   * `SPOTIFY_REFRESH_TOKEN`
5. Enable GitHub Actions (`.github/workflows/weekly_sync.yml`). Your playlists will automatically refresh weekly every Friday!

