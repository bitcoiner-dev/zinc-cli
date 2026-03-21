use std::panic;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use std::io::stdout;

pub fn install_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let mut stdout = stdout();
        let _ = disable_raw_mode();
        let _ = stdout.execute(LeaveAlternateScreen);
        let _ = crossterm::cursor::show();

        original_hook(panic_info);
    }));
}
