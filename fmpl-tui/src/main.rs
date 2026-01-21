use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use fmpl_core::{Vm, eval};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::io;
use std::time::Duration;

struct App {
    research_content: String,
    planning_content: String,
    code_lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    scroll_offset: usize,
    output: String,
    should_quit: bool,
    execute_mode: bool, // When true, Enter executes code
}

impl App {
    fn new() -> Self {
        App {
            research_content: String::from("Research view - Problem space analysis"),
            planning_content: String::from("Planning view - Collaborative scope definition"),
            code_lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            output: String::from(
                "FMPL TUI - Agentic Development Environment\nEsc+Enter to execute, q to quit\nLoad lib/ollama.fmpl or lib/anthropic.fmpl for LLM chat",
            ),
            should_quit: false,
            execute_mode: false,
        }
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                self.execute_mode = !self.execute_mode;
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
            }
            KeyCode::Backspace => {
                self.backspace();
            }
            KeyCode::Delete => {
                self.delete();
            }
            KeyCode::Enter => {
                if self.execute_mode {
                    self.execute_code();
                    self.execute_mode = false;
                } else {
                    self.insert_newline();
                }
            }
            KeyCode::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            KeyCode::Right => {
                let line_len = self.code_lines[self.cursor_row].len();
                if self.cursor_col < line_len {
                    self.cursor_col += 1;
                }
            }
            KeyCode::Up => {
                if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    let line_len = self.code_lines[self.cursor_row].len();
                    self.cursor_col = self.cursor_col.min(line_len);
                    self.adjust_scroll();
                }
            }
            KeyCode::Down => {
                if self.cursor_row < self.code_lines.len() - 1 {
                    self.cursor_row += 1;
                    let line_len = self.code_lines[self.cursor_row].len();
                    self.cursor_col = self.cursor_col.min(line_len);
                    self.adjust_scroll();
                }
            }
            KeyCode::Home => {
                self.cursor_col = 0;
            }
            KeyCode::End => {
                self.cursor_col = self.code_lines[self.cursor_row].len();
            }
            KeyCode::Tab => {
                self.insert_str("    ");
            }
            _ => {}
        }
    }

    fn insert_char(&mut self, c: char) {
        self.code_lines[self.cursor_row].insert(self.cursor_col, c);
        self.cursor_col += 1;
    }

    fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.code_lines[self.cursor_row].insert(self.cursor_col, c);
            self.cursor_col += 1;
        }
    }

    fn backspace(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;

        if col > 0 {
            self.code_lines[row].remove(col - 1);
            self.cursor_col -= 1;
        } else if row > 0 {
            // Merge with previous line
            let prev_line_len = self.code_lines[row - 1].len();
            let current_line = self.code_lines.remove(row);
            self.code_lines[row - 1].push_str(&current_line);
            self.cursor_row -= 1;
            self.cursor_col = prev_line_len;
            self.adjust_scroll();
        }
    }

    fn delete(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;

        let line_len = self.code_lines[row].len();
        if col < line_len {
            self.code_lines[row].remove(col);
        } else if row < self.code_lines.len() - 1 {
            // Merge with next line
            let next_line = self.code_lines.remove(row + 1);
            self.code_lines[row].push_str(&next_line);
        }
    }

    fn insert_newline(&mut self) {
        let row = self.cursor_row;
        let col = self.cursor_col;

        let new_line = self.code_lines[row].split_off(col);
        self.code_lines.insert(row + 1, new_line);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.adjust_scroll();
    }

    fn adjust_scroll(&mut self) {
        const VISIBLE_LINES: usize = 10; // Approximate visible lines
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        } else if self.cursor_row >= self.scroll_offset + VISIBLE_LINES {
            self.scroll_offset = self.cursor_row - VISIBLE_LINES + 1;
        }
    }

    fn execute_code(&mut self) {
        let code = self.get_code();
        if code.trim().is_empty() {
            return;
        }

        // Create VM and execute code
        let mut vm = Vm::new();

        match eval(&mut vm, &code) {
            Ok(result) => {
                self.output = format!(">>> {}\nResult: {:?}", code, result);
            }
            Err(e) => {
                self.output = format!(">>> {}\nError: {}", code, e);
            }
        }

        // Clear input after execution
        self.code_lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
    }

    fn get_code(&self) -> String {
        self.code_lines.join("\n")
    }
}

fn draw_ui(f: &mut Frame, app: &App) {
    // Main layout: split into horizontal sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Percentage(33), // Research
                Constraint::Percentage(33), // Planning
                Constraint::Percentage(34), // Execution (bottom)
            ]
            .as_ref(),
        )
        .split(f.area());

    // Research panel
    let research_panel = Paragraph::new(app.research_content.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Research View")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });

    // Planning panel
    let planning_panel = Paragraph::new(app.planning_content.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Planning View")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });

    // Execution panel - split horizontally into code input and output
    let execution_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(
            [
                Constraint::Percentage(50), // Code input
                Constraint::Percentage(50), // Output
            ]
            .as_ref(),
        )
        .split(chunks[2]);

    // Code input panel
    let mode_indicator = if app.execute_mode {
        " [EXECUTE MODE - Press Enter to run]"
    } else {
        " [EDIT MODE - Press Esc then Enter to run]"
    };

    let visible_lines: Vec<String> = app
        .code_lines
        .iter()
        .skip(app.scroll_offset)
        .take(20) // Show max 20 lines
        .cloned()
        .collect();

    let mut code_spans: Vec<Line> = vec![Line::from(format!(
        "FMPL Code{} (q to quit):",
        mode_indicator
    ))];

    for (i, line) in visible_lines.iter().enumerate() {
        let actual_row = app.scroll_offset + i;
        let is_cursor_row = actual_row == app.cursor_row;

        if is_cursor_row {
            // Show cursor position
            let before = &line[..app.cursor_col.min(line.len())];
            let after = &line[app.cursor_col.min(line.len())..];

            code_spans.push(Line::from(vec![
                Span::raw(format!("{:2} ", actual_row + 1)),
                Span::raw(before),
                Span::styled(
                    if app.cursor_col < line.len() {
                        &line[app.cursor_col..app.cursor_col + 1]
                    } else {
                        " "
                    },
                    Style::default().fg(Color::Yellow).bg(Color::DarkGray),
                ),
                Span::raw(after),
            ]));
        } else {
            code_spans.push(Line::from(vec![
                Span::raw(format!("{:2} ", actual_row + 1)),
                Span::raw(line.as_str()),
            ]));
        }
    }

    let code_text = Text::from(code_spans);

    let code_panel = Paragraph::new(code_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Code Editor")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: false }); // Don't wrap - show horizontal scroll

    // Output panel
    let output_panel = Paragraph::new(app.output.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Execution Output")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });

    // Render all panels
    f.render_widget(research_panel, chunks[0]);
    f.render_widget(planning_panel, chunks[1]);
    f.render_widget(code_panel, execution_chunks[0]);
    f.render_widget(output_panel, execution_chunks[1]);
}

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();

    // Main loop
    loop {
        // Draw UI
        terminal.draw(|f| draw_ui(f, &app))?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_input(key);
            }
        }

        // Check for quit
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
