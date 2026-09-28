//! A plain Win32 window for process-level tests.
//!
//! It shows one top-level window of the predefined `STATIC` class, far off
//! screen and without taking the focus, and pumps its messages, so that UI
//! Automation lists the window and its requests are answered. Its bitness
//! follows the build target: the Windows acceptance lane builds it for
//! `i686-pc-windows-msvc` as the 32-bit process of the architecture scenario.
//! It is a helper, not a fixture of the blueprint: it has a process and a
//! window, and no controls.
//!
//! ```text
//! platynui-win32-test-window [--title <text>] [--auto-close <seconds>]
//! ```
//!
//! It exits by itself after `--auto-close` seconds (default 60), so that a
//! failed teardown leaves no process behind, and once its window is closed.

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("platynui-win32-test-window runs only on Windows");
    std::process::ExitCode::from(1)
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match options::Options::parse(std::env::args().skip(1)) {
        Ok(options) => window::run(&options),
        Err(message) => {
            eprintln!("{message}\n{}", options::USAGE);
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
mod options {
    use std::time::Duration;

    pub(crate) const USAGE: &str = "usage: platynui-win32-test-window [--title <text>] [--auto-close <seconds>]";

    /// What the command line asks for.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct Options {
        pub(crate) title: String,
        pub(crate) auto_close: Duration,
    }

    impl Options {
        pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
            let mut options =
                Self { title: "PlatynUI Win32 Test Window".to_owned(), auto_close: Duration::from_secs(60) };
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--title" => options.title = args.next().ok_or("--title needs a text")?,
                    "--auto-close" => {
                        let seconds = args.next().ok_or("--auto-close needs a number of seconds")?;
                        let seconds = seconds
                            .parse()
                            .map_err(|_| format!("--auto-close needs a number of seconds, not {seconds}"))?;
                        options.auto_close = Duration::from_secs(seconds);
                    }
                    other => return Err(format!("unknown argument {other}")),
                }
            }
            Ok(options)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::Options;
        use std::time::Duration;

        fn parse(args: &[&str]) -> Result<Options, String> {
            Options::parse(args.iter().map(|arg| (*arg).to_owned()))
        }

        #[test]
        fn the_title_and_the_lifetime_come_from_the_command_line() {
            let options = parse(&["--title", "Win32 Test Window", "--auto-close", "5"]).expect("valid");
            assert_eq!(options.title, "Win32 Test Window");
            assert_eq!(options.auto_close, Duration::from_secs(5));
        }

        #[test]
        fn without_arguments_the_window_closes_itself_after_a_minute() {
            assert_eq!(parse(&[]).expect("valid").auto_close, Duration::from_secs(60));
        }

        #[test]
        fn a_missing_or_malformed_value_is_refused() {
            assert!(parse(&["--title"]).is_err());
            assert!(parse(&["--auto-close", "soon"]).is_err());
            assert!(parse(&["--size", "3"]).is_err());
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code, reason = "the window is created and pumped through Win32 calls")]
mod window {
    use super::options::Options;
    use std::process::ExitCode;
    use std::time::{Duration, Instant};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, IsWindow, MSG, PM_REMOVE, PeekMessageW, SW_SHOWNOACTIVATE, ShowWindow,
        TranslateMessage, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{HSTRING, PCWSTR, w};

    /// Shows the window and answers its messages until the lifetime is over
    /// or the window has been closed.
    pub(crate) fn run(options: &Options) -> ExitCode {
        let title = HSTRING::from(options.title.as_str());
        // A top-level window of the predefined STATIC class, far off screen:
        // visible to UI Automation, invisible to whoever works at the desktop.
        // SAFETY: creating a top-level window of a predefined class; `title`
        // is a null-terminated wide string that outlives the call.
        let created = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW,
                -20_000,
                -20_000,
                400,
                200,
                None,
                None,
                None,
                None,
            )
        };
        let window = match created {
            Ok(window) => window,
            Err(error) => {
                eprintln!("cannot create the window: {error}");
                return ExitCode::FAILURE;
            }
        };
        // SAFETY: showing the window created above, without activating it.
        let _ = unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };

        let deadline = Instant::now() + options.auto_close;
        let mut message = MSG::default();
        // SAFETY: `window` is a handle this thread created; IsWindow only asks
        // whether it still names a window.
        while Instant::now() < deadline && unsafe { IsWindow(Some(window)) }.as_bool() {
            // SAFETY: pumping this thread's messages, so that UI Automation's
            // requests to the window are answered.
            while unsafe { PeekMessageW(&raw mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                // SAFETY: `message` was just filled by PeekMessageW.
                unsafe {
                    let _ = TranslateMessage(&raw const message);
                    DispatchMessageW(&raw const message);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        ExitCode::SUCCESS
    }
}
