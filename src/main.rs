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
    event::{self, Event, EnableMouseCapture, DisableMouseCapture, MouseEventKind},
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

    // Register all decoders in app for settings UI (with params)
    app.register_decoder("Built-in".to_string(), DecoderSource::Builtin, Vec::new());
    for name in lua_mgr.decoder_names() {
        let params = lua_mgr.query_params(&name);
        app.register_decoder(name, DecoderSource::Lua, params);
    }
    for name in wasm_mgr.decoder_names() {
        let params = wasm_mgr.query_params(&name);
        app.register_decoder(name, DecoderSource::Wasm, params);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_loop(&mut terminal, &mut app, &mut lua_mgr, &mut wasm_mgr);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
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
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => {
                    let col = mouse.column;
                    let row = mouse.row;
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            if let Some(area) = app.hex_area {
                                if col >= area.x && col < area.x + area.width
                                    && row >= area.y && row < area.y + area.height
                                {
                                    app.scroll_offset = app.scroll_offset.saturating_sub(3);
                                }
                            }
                            if let Some(area) = app.decode_area {
                                if col >= area.x && col < area.x + area.width
                                    && row >= area.y && row < area.y + area.height
                                {
                                    app.decode_scroll_offset = app.decode_scroll_offset.saturating_sub(3);
                                }
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if let Some(area) = app.hex_area {
                                if col >= area.x && col < area.x + area.width
                                    && row >= area.y && row < area.y + area.height
                                {
                                    let max_scroll = app.total_rows().saturating_sub(app.visible_rows);
                                    app.scroll_offset = (app.scroll_offset + 3).min(max_scroll);
                                }
                            }
                            if let Some(area) = app.decode_area {
                                if col >= area.x && col < area.x + area.width
                                    && row >= area.y && row < area.y + area.height
                                {
                                    app.decode_scroll_offset += 3;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if app.quit {
            break;
        }
    }

    Ok(())
}
