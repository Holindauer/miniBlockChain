use std::process::Command;

// Helper function for clearing the terminal
pub fn clear_terminal() {
    #[cfg(unix)]
    Command::new("clear").status().unwrap();

    #[cfg(windows)]
    Command::new("cmd").args(&["/C", "cls"]).status().unwrap();
}
