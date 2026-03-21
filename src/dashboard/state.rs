use std::collections::{HashMap, HashSet};

use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use zinc_core::{Inscription, Network};

use ratatui::layout::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncType {
    Balance,
    Inscriptions,
}

#[derive(Debug)]
pub enum DashboardEvent {
    Input(Event),
    Tick,
    BalanceUpdated {
        confirmed: u64,
        pending: u64,
    },
    InscriptionsUpdated(Vec<Inscription>),
    SyncStarted(SyncType),
    SyncFinished(SyncType),
    AddressesUpdated {
        ordinals: String,
        payment: Option<String>,
    },
}

pub struct DashboardLayout {
    pub header: Rect,
    pub hero: Rect,
    pub main: Rect,
    pub footer: Rect,
}

impl DashboardLayout {
    pub fn new(area: Rect) -> Self {
        let hero_height = match area.width {
            0..=79 => 8,
            80..=109 => 10,
            _ => 12,
        };
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(3),           // Header
                ratatui::layout::Constraint::Length(1),           // Gap
                ratatui::layout::Constraint::Length(hero_height), // Hero balance
                ratatui::layout::Constraint::Length(1),           // Gap
                ratatui::layout::Constraint::Min(0),              // Inscriptions
                ratatui::layout::Constraint::Length(1),           // Gap
                ratatui::layout::Constraint::Length(3),           // Footer
            ])
            .split(area);

        Self {
            header: chunks[0],
            hero: chunks[2],
            main: chunks[4],
            footer: chunks[6],
        }
    }
}

pub struct DashboardState {
    pub confirmed_balance: u64,
    pub pending_balance: u64,
    pub inscriptions: Vec<Inscription>,
    pub inscription_index: usize,
    pub sync_tx: tokio::sync::broadcast::Sender<()>,
    pub is_syncing_balance: bool,
    pub is_syncing_inscriptions: bool,
    pub account_index: u32,
    pub tick_count: u64,
    pub image_cache: HashMap<String, Protocol>,
    pub ascii_cache: HashMap<String, Vec<ratatui::text::Line<'static>>>,
    pub failed_images: HashSet<String>,
    #[allow(dead_code)]
    pub picker: Picker,
    pub is_locked: bool,
    pub password_input: String,
    pub auth_error: Option<String>,
    pub is_quitting: bool,
    pub ascii_mode: bool,
    pub network: Option<Network>,
    pub profile_name: Option<String>,
    pub mouse_pos: Option<(u16, u16)>,
    pub hover_inscription_index: Option<usize>,
    pub hover_balance: bool,
    pub gallery_cols: usize,
    pub ordinals_address: Option<String>,
    pub payment_address: Option<String>,
}

impl DashboardState {
    pub fn new(
        sync_tx: tokio::sync::broadcast::Sender<()>,
        picker: Picker,
        ascii_mode: bool,
    ) -> Self {
        Self {
            confirmed_balance: 0,
            pending_balance: 0,
            inscriptions: Vec::new(),
            inscription_index: 0,
            sync_tx,
            is_syncing_balance: false,
            is_syncing_inscriptions: false,
            account_index: 0,
            tick_count: 0,
            image_cache: HashMap::new(),
            failed_images: HashSet::new(),
            picker,
            is_locked: true,
            password_input: String::new(),
            auth_error: None,
            is_quitting: false,
            ascii_mode,
            ascii_cache: HashMap::new(),
            network: None,
            profile_name: None,
            mouse_pos: None,
            hover_inscription_index: None,
            hover_balance: false,
            gallery_cols: 3,
            ordinals_address: None,
            payment_address: None,
        }
    }
}
