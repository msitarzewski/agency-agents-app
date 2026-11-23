use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(target_os = "windows")]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Build a standard-library command that must not display a console window.
pub(crate) fn headless_command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut command = Command::new(program);
    configure_headless(&mut command);
    command
}

/// Build a Tokio command that must not display a console window.
pub(crate) fn headless_tokio_command<S: AsRef<OsStr>>(program: S) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    configure_headless(command.as_std_mut());
    command
}

/// Build a command that opens in its own console window. Used for launching
/// interactive terminal tools (e.g. OpenCode TUI) from the GUI app.
pub(crate) fn console_command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut command = Command::new(program);
    configure_console(&mut command);
    command
}

fn configure_headless(command: &mut Command) {
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

fn configure_console(command: &mut Command) {
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NEW_CONSOLE);

    #[cfg(not(target_os = "windows"))]
    let _ = command;
}

/// Spawn an interactive tool in a directory. On Windows this opens a new console;
/// on Unix it spawns the program directly (caller should ensure a terminal context
/// or use a terminal emulator wrapper when needed).
pub(crate) fn spawn_in_dir<S: AsRef<OsStr>>(program: S, dir: &Path) -> std::io::Result<()> {
    let mut cmd = console_command(program);
    cmd.current_dir(dir);
    #[cfg(not(target_os = "windows"))]
    {
        // Detach from the app's process group so closing the app doesn't kill it.
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}
