use super::state::SetupStep;
use crate::error::AppError;
use crate::ui::widgets::{BrandedHeader, InfoCard};
use crate::ui::ZincTheme;
use crate::wallet_service::{validate_mnemonic_internal, ZincMnemonic};
use crate::wizard::state::SetupState;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use owo_colors::OwoColorize;
use ratatui::prelude::*;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::time::Duration;
use tokio::sync::mpsc;

pub enum TuiEvent {
    Input(Event),
    Tick,
}

pub struct TuiWizard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    state: SetupState,
    input_buffer: String,
    error_message: Option<String>,
}

impl TuiWizard {
    pub fn new(state: SetupState) -> Result<Self, AppError> {
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            state,
            input_buffer: String::new(),
            error_message: None,
        })
    }

    pub async fn run(mut self) -> Result<SetupState, AppError> {
        let (tx, mut rx) = mpsc::channel(32);

        // Input handling thread
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap() {
                    if let Ok(ev) = event::read() {
                        let _ = tx.send(TuiEvent::Input(ev)).await;
                    }
                }
                let _ = tx.send(TuiEvent::Tick).await;
            }
        });

        let theme = ZincTheme::dark();
        loop {
            self.terminal.draw(|f: &mut Frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // Header
                        Constraint::Min(0),    // Main
                        Constraint::Length(3), // Footer
                    ])
                    .split(f.area());
                let network = match self.state.values.default_network.as_deref() {
                    Some("mainnet") | Some("bitcoin") => zinc_core::Network::Bitcoin,
                    Some("testnet") => zinc_core::Network::Testnet,
                    Some("regtest") => zinc_core::Network::Regtest,
                    Some("signet") => zinc_core::Network::Signet,
                    _ => zinc_core::Network::Bitcoin,
                };

                f.render_widget(BrandedHeader {
                    title: "SETUP WIZARD",
                    profile_name: self.state.values.profile.as_str(),
                    theme: &theme,
                    network,
                    account_index: 0,
                    is_loading: false,
                    tick: 0,
                    _ascii_mode: false,
                }, chunks[0]);


                let (step_title, step_content) = match self.state.current {
                    SetupStep::Welcome => (
                        "WELCOME",
                        "Welcome to Zinc.\n\n[C] Create New Wallet\n[R] Restore Existing Wallet\n\nChoose an option to continue."
                    ),
                    SetupStep::CreateShowSeed => (
                        "GENERATE SEED",
                        "YOUR RECOVERY PHRASE (WRITE THIS DOWN!):\n\n"
                    ),
                    SetupStep::CreateVerifySeed => (
                        "VERIFY SEED",
                        "Please verify your recovery phrase to continue."
                    ),
                    SetupStep::RestoreInputSeed => (
                        "RESTORE WALLET",
                        "Enter your 12- or 24-word recovery phrase."
                    ),
                    SetupStep::SetPassword => (
                        "SET PASSWORD",
                        "Enter a password to encrypt your wallet."
                    ),
                    SetupStep::ConfirmPassword => (
                        "CONFIRM PASSWORD",
                        "Please re-enter your password to confirm."
                    ),
                    SetupStep::Done => (
                        "SETUP COMPLETE",
                        "Your Zinc wallet is ready.\n\nPress Enter to launch the Dashboard."
                    ),
                };

                let mut display_content = step_content.to_string();

                match self.state.current {
                    SetupStep::CreateShowSeed => {
                        if let Some(m) = &self.state.temp_mnemonic {
                            display_content.push_str(&format!("\n{}\n\nPress Enter when you have securely backed this up.", m));
                        }
                    }
                    SetupStep::CreateVerifySeed => {
                        let i = self.state.verify_indices;
                        display_content.push_str(&format!("\n\nEnter word #{} and #{} and #{}", i[0]+1, i[1]+1, i[2]+1));
                        display_content.push_str(&format!("\n\n> {}", self.input_buffer));
                    }
                    SetupStep::RestoreInputSeed => {
                        display_content.push_str(&format!("\n\n> {}", self.input_buffer));

                        let words: Vec<&str> = self.input_buffer.split_whitespace().collect();
                        if words.len() == 12 || words.len() == 24 {
                            match validate_mnemonic_internal(&self.input_buffer) {
                                true => display_content.push_str("\n\n✅ Mnemonic is VALID. Press Enter to continue."),
                                false => display_content.push_str("\n\n❌ Mnemonic is INVALID. Please check the words and order."),
                            }
                        } else if !words.is_empty() {
                            display_content.push_str(&format!("\n\n({} words entered...)", words.len()));

                            if self.input_buffer.ends_with(' ') {
                                if let Some(last) = words.last() {
                                    let wordlist = bip39::Language::English.word_list();
                                    if wordlist.binary_search(&last.to_lowercase().as_str()).is_err() {
                                        display_content.push_str(&format!("\n\n⚠️ '{}' is NOT a valid BIP-39 word.", last));
                                    }
                                }
                            }
                        }
                    }
                    SetupStep::SetPassword | SetupStep::ConfirmPassword => {
                        let masked = "*".repeat(self.input_buffer.len());
                        display_content.push_str(&format!("\n\nPassword: {}", masked));
                    }
                    _ => {}
                }

                if let Some(msg) = &self.error_message {
                    display_content.push_str(&format!("\n\n{}", msg.red()));
                }

                f.render_widget(InfoCard { title: step_title, content: &display_content, theme: &theme }, chunks[1]);
                f.render_widget(InfoCard { title: "CONTROLS", content: "Enter: Next | Esc: Back | Ctrl+C: Quit", theme: &theme }, chunks[2]);
            })?;

            if let Some(event) = rx.recv().await {
                match event {
                    TuiEvent::Input(Event::Key(key)) => {
                        self.error_message = None; // Clear error message on any key input
                        match key.code {
                            KeyCode::Char('c')
                                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                break
                            }
                            KeyCode::Esc => {
                                if self.state.current == SetupStep::ConfirmPassword {
                                    self.state.password_temp = None;
                                }
                                if !self.state.back() {
                                    break;
                                }
                                self.input_buffer.clear();
                            }
                            KeyCode::Enter => {
                                match self.state.current {
                                    SetupStep::Welcome => {
                                        // Support both Enter and immediate keys
                                        let choice = self.input_buffer.to_lowercase();
                                        if choice == "c" {
                                            self.start_create_flow();
                                        } else if choice == "r" {
                                            self.state.next_step(Some("restore".to_string()));
                                        }
                                    }
                                    SetupStep::CreateShowSeed => {
                                        self.state.next_step(None);
                                    }
                                    SetupStep::CreateVerifySeed => {
                                        if let Some(m) = &self.state.temp_mnemonic {
                                            let m_words: Vec<&str> = m.split_whitespace().collect();
                                            let input_words: Vec<&str> =
                                                self.input_buffer.split_whitespace().collect();

                                            if input_words.len() == 3 {
                                                let mut all_match = true;
                                                for (idx, &v_idx) in
                                                    self.state.verify_indices.iter().enumerate()
                                                {
                                                    if input_words[idx].to_lowercase()
                                                        != m_words[v_idx].to_lowercase()
                                                    {
                                                        all_match = false;
                                                        break;
                                                    }
                                                }

                                                if all_match {
                                                    self.error_message = None;
                                                    self.state.next_step(None);
                                                } else {
                                                    self.error_message = Some("❌ Verification failed. Please check the words and try again.".to_string());
                                                    self.input_buffer.clear();
                                                }
                                            } else {
                                                self.error_message = Some(
                                                    "❌ Please enter exactly 3 words.".to_string(),
                                                );
                                            }
                                        }
                                    }
                                    SetupStep::RestoreInputSeed => {
                                        let phrase = self.input_buffer.trim();
                                        let words: Vec<&str> = phrase.split_whitespace().collect();
                                        if !(words.len() == 12 || words.len() == 24) {
                                            self.error_message = Some(
                                                "❌ Recovery phrase must be 12 or 24 words."
                                                    .to_string(),
                                            );
                                        } else if !validate_mnemonic_internal(phrase) {
                                            self.error_message = Some(
                                                "❌ Recovery phrase is invalid. Check spelling and order."
                                                    .to_string(),
                                            );
                                        } else {
                                            self.state.values.restore_mnemonic =
                                                Some(phrase.to_string());
                                            self.state.next_step(None);
                                        }
                                    }
                                    SetupStep::SetPassword => {
                                        if self.input_buffer.is_empty() {
                                            self.error_message =
                                                Some("❌ Password cannot be empty.".to_string());
                                        } else {
                                            self.state.password_temp =
                                                Some(self.input_buffer.clone());
                                            self.state.next_step(None);
                                        }
                                    }
                                    SetupStep::ConfirmPassword => {
                                        if Some(&self.input_buffer)
                                            == self.state.password_temp.as_ref()
                                        {
                                            self.state.values.password =
                                                Some(self.input_buffer.clone());
                                            self.state.values.initialize_wallet = true;
                                            self.state.next_step(None);
                                        } else {
                                            self.error_message = Some(
                                                "❌ Passwords do not match. Please try again."
                                                    .to_string(),
                                            );
                                            self.input_buffer.clear();
                                            // Stay in ConfirmPassword
                                        }
                                    }
                                    SetupStep::Done => break,
                                }
                                self.input_buffer.clear();
                            }
                            KeyCode::Char('c') | KeyCode::Char('C')
                                if self.state.current == SetupStep::Welcome =>
                            {
                                self.start_create_flow();
                                self.input_buffer.clear();
                            }
                            KeyCode::Char('r') | KeyCode::Char('R')
                                if self.state.current == SetupStep::Welcome =>
                            {
                                self.state.next_step(Some("restore".to_string()));
                                self.input_buffer.clear();
                            }
                            KeyCode::Char(c) => {
                                self.input_buffer.push(c);
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        self.cleanup()?;
        Ok(self.state)
    }

    fn start_create_flow(&mut self) {
        // Generate mnemonic using zinc_core
        let mnemonic = match ZincMnemonic::generate(12) {
            Ok(mnemonic) => mnemonic.phrase().to_string(),
            Err(err) => {
                self.error_message = Some(format!("❌ failed to generate seed phrase: {err}"));
                return;
            }
        };

        self.state.temp_mnemonic = Some(mnemonic);
        // Pick 3 random indices from 12 words
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.state.verify_indices = [
            rng.gen_range(0..4),
            rng.gen_range(4..8),
            rng.gen_range(8..12),
        ];
        self.state.next_step(Some("create".to_string()));
    }

    fn cleanup(&mut self) -> Result<(), AppError> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}
