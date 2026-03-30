mod app;
mod decode;
mod decoder_lua;
mod decoder_wasm;
mod file_buffer;
mod hex_view;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, DecoderSource};
use decoder_lua::LuaDecoderManager;
use decoder_wasm::WasmDecoderManager;
use file_buffer::FileBuffer;

#[derive(Parser)]
#[command(name = "turbohex")]
#[command(about = "Interactive TUI hex viewer with decode panel")]
#[command(after_help = "Use --skills to print the decoder plugin development guide")]
struct Cli {
    /// File to view
    file: Option<PathBuf>,

    /// Print decoder plugin development guide (markdown)
    #[arg(long)]
    skills: bool,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if cli.skills {
        print!("{}", include_str!("skills.md"));
        return Ok(());
    }

    let file = cli.file.unwrap_or_else(|| {
        eprintln!("error: a file argument is required\n\nUsage: turbohex <FILE>\n\nFor more information, try '--help'.");
        std::process::exit(1);
    });

    let buffer = FileBuffer::open(&file)?;
    let filename = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut app = App::new(buffer, filename);
    let mut lua_mgr = LuaDecoderManager::new();
    lua_mgr.load_decoders();
    let mut wasm_mgr = WasmDecoderManager::new();
    wasm_mgr.load_decoders();

    // Register all decoders in app for settings UI
    app.register_decoder("Built-in".to_string(), DecoderSource::Builtin);
    for name in lua_mgr.decoder_names() {
        app.register_decoder(name, DecoderSource::Lua);
    }
    for name in wasm_mgr.decoder_names() {
        app.register_decoder(name, DecoderSource::Wasm);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_loop(&mut terminal, &mut app, &mut lua_mgr, &mut wasm_mgr);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    lua_mgr: &mut LuaDecoderManager,
    wasm_mgr: &mut WasmDecoderManager,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| {
            ui::draw(frame, app, lua_mgr, wasm_mgr);
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        if app.quit {
            break;
        }
    }

    Ok(())
}
