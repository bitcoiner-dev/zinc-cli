use crate::wizard::SetupValues;

#[derive(Debug, Clone, PartialEq)]
pub enum SetupStep {
    Welcome,
    CreateShowSeed,
    CreateVerifySeed,
    RestoreInputSeed,
    SetPassword,
    ConfirmPassword,
    Done,
}

pub struct SetupState {
    pub stack: Vec<SetupStep>,
    pub current: SetupStep,
    pub values: SetupValues,
    pub temp_mnemonic: Option<String>,
    pub verify_indices: [usize; 3],
    pub password_temp: Option<String>,
}

impl SetupState {
    pub fn new(seed: SetupValues) -> Self {
        Self {
            stack: Vec::new(),
            current: SetupStep::Welcome,
            values: seed,
            temp_mnemonic: None,
            verify_indices: [0; 3],
            password_temp: None,
        }
    }

    pub fn next_step(&mut self, choice: Option<String>) {
        let next = match self.current {
            SetupStep::Welcome => {
                if choice.as_deref() == Some("create") {
                    SetupStep::CreateShowSeed
                } else {
                    SetupStep::RestoreInputSeed
                }
            }
            SetupStep::CreateShowSeed => SetupStep::CreateVerifySeed,
            SetupStep::CreateVerifySeed => SetupStep::SetPassword,
            SetupStep::RestoreInputSeed => SetupStep::SetPassword,
            SetupStep::SetPassword => SetupStep::ConfirmPassword,
            SetupStep::ConfirmPassword => SetupStep::Done,
            SetupStep::Done => SetupStep::Done,
        };
        self.stack.push(self.current.clone());
        self.current = next;
    }

    pub fn back(&mut self) -> bool {
        if let Some(prev) = self.stack.pop() {
            self.current = prev;
            true
        } else {
            false
        }
    }
}
