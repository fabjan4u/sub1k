#![allow(dead_code)]

use rand::Rng;
use rspotify::model::Device;
use std::time::Instant;

use crate::config::{AppConfig, RescuedStats};
use crate::digger::DiscoveredTrack;

#[derive(Debug, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    EnteringGenre,
    SelectingDevice,
}

#[derive(Debug)]
pub enum Action {
    PlayPause,
    PlayTrack(String),
    NextTrack,
    RescueTrack(String),
    FollowArtist(String),
    DigGenre(String),
    ReplenishQueue(String),
    SelectDevice(String),
    RefreshPlayback,
    RefreshDevices,
    Quit,
}

#[derive(Debug)]
pub enum Update {
    PlaybackChanged(Option<PlaybackInfo>),
    TracksDiscovered(Vec<DiscoveredTrack>),
    DevicesLoaded(Vec<Device>),
    Status(String),
    Error(String),
    TrackRescued(String),
}

#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    pub track_title: String,
    pub artist_names: String,
    pub album_name: String,
    pub track_id: Option<String>,
    pub artist_id: Option<String>,
    pub is_playing: bool,
    pub progress_ms: u32,
    pub duration_ms: u32,
    pub popularity: u32,
    pub is_featured: bool,
    pub device_name: String,
    pub device_volume: Option<u32>,
}

pub struct App {
    pub config: AppConfig,
    pub genre: String,
    pub queue: Vec<DiscoveredTrack>,
    pub queue_index: usize,
    pub playback: Option<PlaybackInfo>,
    pub available_devices: Vec<Device>,
    pub device_index: usize,
    pub boost_mode: bool,
    pub target_boost_skip: u32, // Smart Boost jitter: skips after 35-45s
    pub rescued_count: u32,
    pub status_message: Option<(String, Instant)>,
    pub input_mode: InputMode,
    pub genre_input: String,
    pub is_loading: bool,
    pub is_replenishing: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let genre = config.default_genre.clone();
        let target_boost_skip = rand::thread_rng().gen_range(35..=45);
        let rescued_count = RescuedStats::load().total_rescued;
        Self {
            config,
            genre,
            queue: Vec::new(),
            queue_index: 0,
            playback: None,
            available_devices: Vec::new(),
            device_index: 0,
            boost_mode: false,
            target_boost_skip,
            rescued_count,
            status_message: None,
            input_mode: InputMode::Normal,
            genre_input: String::new(),
            is_loading: false,
            is_replenishing: false,
            should_quit: false,
        }
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }

    pub fn clear_expired_status(&mut self) {
        if let Some((_, timestamp)) = &self.status_message {
            if timestamp.elapsed().as_secs() >= 4 {
                self.status_message = None;
            }
        }
    }

    pub fn reset_boost_jitter(&mut self) {
        self.target_boost_skip = rand::thread_rng().gen_range(35..=45);
    }

    pub fn next_queue_item(&mut self) -> Option<DiscoveredTrack> {
        if self.queue.is_empty() {
            return None;
        }

        if self.queue_index + 1 < self.queue.len() {
            self.queue_index += 1;
        } else {
            self.queue_index = 0;
        }

        self.queue.get(self.queue_index).cloned()
    }

    pub fn select_queue_index(&mut self, index: usize) -> Option<DiscoveredTrack> {
        if index < self.queue.len() {
            self.queue_index = index;
            self.queue.get(index).cloned()
        } else {
            None
        }
    }
}
