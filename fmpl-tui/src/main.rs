use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use fmpl_core::{StreamEvent, Value, Vm, eval};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::collections::HashMap;
use std::io;
use std::time::Duration;

/// Unique identifier for a conversation node
type NodeId = usize;

/// Metadata about a conversation node
#[derive(Clone, Debug)]
#[allow(dead_code)] // compacted field for Phase 4 (context compaction)
struct NodeMetadata {
    branch_name: Option<String>, // "main", "fix-1", etc.
    edited: bool,                // True if message was edited
    compacted: bool,             // True if elided by compaction (Phase 4)
}

/// A node in the conversation DAG (Directed Acyclic Graph)
#[derive(Clone, Debug)]
#[allow(dead_code)] // id and timestamp fields for future phases (compaction, export)
struct ConversationNode {
    id: NodeId,                // Unique identifier
    parent_id: Option<NodeId>, // Parent in DAG
    message: ChatMessage,      // The actual message
    timestamp: String,         // ISO timestamp (for future compaction/export)
    metadata: NodeMetadata,    // Branch info, edited flag
}

impl ConversationNode {
    fn new(id: NodeId, parent_id: Option<NodeId>, message: ChatMessage) -> Self {
        ConversationNode {
            id,
            parent_id,
            message,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: NodeMetadata {
                branch_name: None,
                edited: false,
                compacted: false,
            },
        }
    }
}

/// Block and wait for an async stream to complete.
/// Returns the final result value or an error.
fn wait_for_async(value: Value) -> Result<Value, String> {
    match value {
        Value::AsyncStream(handle) => {
            let mut handle = handle.lock().map_err(|e| format!("Lock error: {}", e))?;

            // Collect all events from the stream
            let mut final_value = Value::Null;

            loop {
                match handle.recv_blocking() {
                    Some(StreamEvent::Data(v)) => {
                        // Intermediate data - keep last value
                        final_value = v;
                    }
                    Some(StreamEvent::Ok(v)) => {
                        // Terminal success - return result
                        return Ok(v);
                    }
                    Some(StreamEvent::Err(e)) => {
                        // Terminal error - return error
                        return Err(format!("Async error: {}", e));
                    }
                    None => {
                        // Channel closed without Ok/Err
                        if final_value != Value::Null {
                            return Ok(final_value);
                        }
                        return Err("Async stream completed without result".to_string());
                    }
                }
            }
        }
        _ => Ok(value),
    }
}

#[derive(Clone, Copy)]
enum LlmProvider {
    Ollama,
    Anthropic,
}

/// A single message in the conversation history
#[derive(Clone, Debug)]
struct ChatMessage {
    role: String, // "user" or "assistant"
    content: String,
}

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
    llm_mode: bool,     // When true, sends code to LLM instead of executing
    llm_provider: LlmProvider,
    vm: Vm,
    // Layer 2: Conversation DAG for backtracking and branching
    conversation_nodes: HashMap<NodeId, ConversationNode>,
    current_head: NodeId,            // Current branch tip
    node_counter: NodeId,            // For generating IDs
    edit_mode: bool,                 // When true, editing last message in history
    editing_node_id: Option<NodeId>, // Node being edited (None = new message)
}

impl App {
    fn new() -> Self {
        let mut vm = Vm::new();

        // Bootstrap LLM libraries
        let bootstrap_result = Self::bootstrap_llm(&mut vm);

        App {
            research_content: String::from("Research view - Problem space analysis"),
            planning_content: String::from("Planning view - Collaborative scope definition"),
            code_lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            output: format!(
                "FMPL TUI - Agentic Development Environment\n\
                 Esc+Enter to execute, Ctrl+L for LLM chat, q to quit\n\
                 Provider: Ollama (Ctrl+P to switch)\n\
                 {}",
                bootstrap_result
            ),
            should_quit: false,
            execute_mode: false,
            llm_mode: false,
            llm_provider: LlmProvider::Ollama,
            vm,
            // Layer 2: Initialize empty conversation DAG
            conversation_nodes: HashMap::new(),
            current_head: 0,
            node_counter: 0,
            edit_mode: false,
            editing_node_id: None,
        }
    }

    fn bootstrap_llm(vm: &mut Vm) -> String {
        // Try to load LLM libraries
        let libraries = vec![
            "lib/llm-common.fmpl",
            "lib/ollama.fmpl",
            "lib/anthropic.fmpl",
        ];

        let mut results = Vec::new();
        for lib in libraries {
            match std::fs::read_to_string(lib) {
                Ok(content) => match eval(vm, &content) {
                    Ok(_) => results.push(format!("✓ Loaded {}", lib)),
                    Err(e) => results.push(format!("✗ Failed to eval {}: {}", lib, e)),
                },
                Err(_) => results.push(format!("✗ Could not read {}", lib)),
            }
        }

        if results.is_empty() {
            String::from("No LLM libraries found")
        } else {
            results.join("\n")
        }
    }

    // Layer 2: DAG helper methods

    /// Get conversation history as a vector (traverse from root to current_head)
    fn get_history(&self) -> Vec<ChatMessage> {
        let mut history = Vec::new();
        let mut current_id = self.current_head;

        // Traverse backwards from current head to root
        let mut path = Vec::new();
        while let Some(node) = self.conversation_nodes.get(&current_id) {
            path.push((current_id, node.clone()));
            match node.parent_id {
                Some(parent) => current_id = parent,
                None => break,
            }
        }

        // Reverse to get root → current_head order
        path.reverse();

        // Extract messages in order
        for (_, node) in path {
            history.push(node.message);
        }

        history
    }

    /// Get conversation history with metadata (for display)
    fn get_history_with_metadata(&self) -> Vec<(ChatMessage, bool, Option<String>)> {
        let mut history = Vec::new();
        let mut current_id = self.current_head;

        // Traverse backwards from current head to root
        let mut path = Vec::new();
        while let Some(node) = self.conversation_nodes.get(&current_id) {
            path.push((current_id, node.clone()));
            match node.parent_id {
                Some(parent) => current_id = parent,
                None => break,
            }
        }

        // Reverse to get root → current_head order
        path.reverse();

        // Extract messages with edited flag and branch name in order
        for (_, node) in path {
            history.push((
                node.message.clone(),
                node.metadata.edited,
                node.metadata.branch_name.clone(),
            ));
        }

        history
    }

    /// Add a new message to the conversation DAG
    fn add_message(&mut self, message: ChatMessage) {
        let new_id = self.node_counter;
        self.node_counter += 1;

        let parent_id = if self.conversation_nodes.is_empty() {
            None
        } else {
            Some(self.current_head)
        };

        let node = ConversationNode::new(new_id, parent_id, message);
        self.conversation_nodes.insert(new_id, node);
        self.current_head = new_id;
    }

    /// Undo: move to parent node
    fn undo(&mut self) -> Result<(), String> {
        let current_node = self
            .conversation_nodes
            .get(&self.current_head)
            .ok_or("No current node")?;

        match current_node.parent_id {
            Some(parent) => {
                self.current_head = parent;
                Ok(())
            }
            None => Err("Already at root".to_string()),
        }
    }

    /// Redo: move to a child node (simple version: picks first child)
    fn redo(&mut self) -> Result<(), String> {
        // Find children of current node
        let mut child_id = None;
        for (&id, node) in &self.conversation_nodes {
            if node.parent_id == Some(self.current_head) {
                child_id = Some(id);
                break;
            }
        }

        match child_id {
            Some(id) => {
                self.current_head = id;
                Ok(())
            }
            None => Err("No child to redo to".to_string()),
        }
    }

    /// Edit the last message in the conversation history
    fn enter_edit_mode(&mut self) -> Result<(), String> {
        if self.conversation_nodes.is_empty() {
            return Err("No messages to edit".to_string());
        }

        // Get the current node (last message)
        let current_node = self
            .conversation_nodes
            .get(&self.current_head)
            .ok_or("No current node")?;

        // Store the node being edited and load its content into the editor
        self.editing_node_id = Some(self.current_head);
        self.code_lines = vec![current_node.message.content.clone()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.edit_mode = true;

        Ok(())
    }

    /// Save edited message as a new node in the DAG
    fn save_edited_message(&mut self) -> Result<(), String> {
        let edited_content = self.code_lines.join("\n");

        // Get the original node
        let original_node_id = self.editing_node_id.ok_or("Not editing any node")?;

        let original_node = self
            .conversation_nodes
            .get(&original_node_id)
            .ok_or("Original node not found")?;

        // Create a new node with the edited message
        let new_id = self.node_counter;
        self.node_counter += 1;

        let parent_id = original_node.parent_id;

        let edited_message = ChatMessage {
            role: original_node.message.role.clone(),
            content: edited_content,
        };

        let metadata = NodeMetadata {
            branch_name: original_node.metadata.branch_name.clone(),
            edited: true, // Mark as edited
            compacted: false,
        };

        let node = ConversationNode {
            id: new_id,
            parent_id,
            message: edited_message,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata,
        };

        // Add the new node and make it the current head
        self.conversation_nodes.insert(new_id, node);
        self.current_head = new_id;

        // Exit edit mode
        self.edit_mode = false;
        self.editing_node_id = None;
        self.code_lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;

        Ok(())
    }

    /// Create a branch at the current head with the given name
    fn create_branch(&mut self, name: String) -> Result<(), String> {
        if self.conversation_nodes.is_empty() {
            return Err("No conversation to branch".to_string());
        }

        // Get current node and update its branch name
        let current_node = self
            .conversation_nodes
            .get_mut(&self.current_head)
            .ok_or("No current node")?;

        current_node.metadata.branch_name = Some(name);

        Ok(())
    }

    /// List all branches in the conversation DAG
    fn list_branches(&self) -> String {
        let mut branches = std::collections::HashMap::new();

        // Collect unique branch names and their node counts
        for node in self.conversation_nodes.values() {
            if let Some(ref name) = node.metadata.branch_name {
                *branches.entry(name.clone()).or_insert(0) += 1;
            }
        }

        if branches.is_empty() {
            return "No branches created yet.\n\nUse Ctrl+N to create a branch at the current point.".to_string();
        }

        let mut result = String::from("Branches:\n");
        result.push_str(&"=".repeat(40));
        result.push('\n');

        for (name, count) in branches.iter() {
            result.push_str(&format!("\n🌿 {} ({} nodes)\n", name, count));
        }

        result.push_str(&"\n=".repeat(40));
        result.push('\n');

        result
    }

    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Toggle LLM mode
                self.llm_mode = !self.llm_mode;
                self.update_mode_indicator();
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Switch LLM provider
                self.llm_provider = match self.llm_provider {
                    LlmProvider::Ollama => LlmProvider::Anthropic,
                    LlmProvider::Anthropic => LlmProvider::Ollama,
                };
                self.update_mode_indicator();
            }
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Undo: move to parent node in conversation DAG
                match self.undo() {
                    Ok(()) => {
                        self.output = format!("Undo: Moved to node {}", self.current_head);
                    }
                    Err(e) => {
                        self.output = format!("Undo failed: {}", e);
                    }
                }
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Redo: move to child node in conversation DAG
                match self.redo() {
                    Ok(()) => {
                        self.output = format!("Redo: Moved to node {}", self.current_head);
                    }
                    Err(e) => {
                        self.output = format!("Redo failed: {}", e);
                    }
                }
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Enter edit mode to edit last message
                match self.enter_edit_mode() {
                    Ok(()) => {
                        self.update_mode_indicator();
                    }
                    Err(e) => {
                        self.output = format!("Edit mode failed: {}", e);
                    }
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Create a new branch at current point
                let branch_name = format!("branch-{}", self.node_counter);
                match self.create_branch(branch_name.clone()) {
                    Ok(()) => {
                        self.output = format!("Created branch: {}", branch_name);
                    }
                    Err(e) => {
                        self.output = format!("Create branch failed: {}", e);
                    }
                }
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // List all branches
                self.output = self.list_branches();
            }
            KeyCode::Esc => {
                // If in edit mode, cancel and return to normal mode
                if self.edit_mode {
                    self.edit_mode = false;
                    self.editing_node_id = None;
                    self.code_lines = vec![String::new()];
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                    self.output = String::from("Edit mode cancelled");
                } else {
                    self.execute_mode = !self.execute_mode;
                }
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
                // Check for Ctrl+Enter to save edited message
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if self.edit_mode {
                        match self.save_edited_message() {
                            Ok(()) => {
                                self.update_mode_indicator();
                            }
                            Err(e) => {
                                self.output = format!("Save failed: {}", e);
                            }
                        }
                    }
                } else if self.execute_mode {
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

    fn update_mode_indicator(&mut self) {
        let provider_name = match self.llm_provider {
            LlmProvider::Ollama => "Ollama",
            LlmProvider::Anthropic => "Anthropic",
        };

        let mode = if self.edit_mode {
            "EDIT (last message)".to_string()
        } else if self.llm_mode {
            format!("LLM ({})", provider_name)
        } else {
            "Execute".to_string()
        };

        let action = if self.edit_mode {
            "Ctrl+Enter to save, Esc to cancel"
        } else {
            "Press Enter to run"
        };

        self.output = format!("Mode: {} - {}", mode, action);
    }

    fn execute_code(&mut self) {
        let code = self.get_code();
        if code.trim().is_empty() {
            return;
        }

        if self.llm_mode {
            // Send to LLM
            self.send_to_llm(&code);
        } else {
            // Execute as FMPL code
            match eval(&mut self.vm, &code) {
                Ok(result) => {
                    self.output = format!(">>> {}\nResult: {:?}", code, result);
                }
                Err(e) => {
                    self.output = format!(">>> {}\nError: {}", code, e);
                }
            }
        }

        // Clear input after execution
        self.code_lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
    }

    fn send_to_llm(&mut self, prompt: &str) {
        let provider_name = match self.llm_provider {
            LlmProvider::Ollama => "ollama",
            LlmProvider::Anthropic => "anthropic",
        };

        // Add user message to DAG
        self.add_message(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        // Convert conversation history to FMPL messages array format
        // Format: [%{role: "user", content: "..."}, %{role: "assistant", content: "..."}]
        let messages_array = self.format_history_as_fmpl();

        // Use chat_with_history() for multi-turn context
        let fmpl_code = format!("{}.chat_with_history({})", provider_name, messages_array);

        match eval(&mut self.vm, &fmpl_code) {
            Ok(result) => {
                // Wait for async response if needed
                match wait_for_async(result) {
                    Ok(Value::String(response)) => {
                        // Add assistant response to DAG
                        self.add_message(ChatMessage {
                            role: "assistant".to_string(),
                            content: response.to_string(),
                        });

                        self.output = format!(
                            ">>> LLM ({})\n{}\n\nResponse:\n{}",
                            provider_name, prompt, response
                        );
                    }
                    Ok(other) => {
                        self.output = format!(
                            ">>> LLM ({})\n{}\n\nUnexpected response type: {:?}",
                            provider_name, prompt, other
                        );
                    }
                    Err(e) => {
                        self.output =
                            format!(">>> LLM ({})\n{}\n\nError: {}", provider_name, prompt, e);
                    }
                }
            }
            Err(e) => {
                self.output = format!(">>> LLM ({})\n{}\n\nError: {}", provider_name, prompt, e);
            }
        }
    }

    /// Format conversation history as FMPL messages array
    /// Converts Rust ChatMessage structs to FMPL array literal
    /// Output format: [%{role: "user", content: "..."}, %{role: "assistant", content: "..."}]
    fn format_history_as_fmpl(&self) -> String {
        let history = self.get_history();
        if history.is_empty() {
            return "[]".to_string();
        }

        let messages: Vec<String> = history
            .iter()
            .map(|msg| format!("%{{role: \"{}\", content: {:?}}}", msg.role, msg.content))
            .collect();

        format!("[{}]", messages.join(", "))
    }

    /// Format conversation history for display
    fn format_history(&self) -> String {
        let history = self.get_history_with_metadata();
        if history.is_empty() {
            return "No conversation history yet.\n\nUse Ctrl+L to enter LLM chat mode and start a conversation.".to_string();
        }

        let mut text = String::from("Conversation History:\n");
        text.push_str(&"=".repeat(40));
        text.push('\n');

        for (i, item) in history.iter().enumerate() {
            let (msg, edited, branch_name) = item;
            let role_label = if msg.role == "user" {
                "👤 User"
            } else {
                "🤖 Assistant"
            };
            let edited_marker = if *edited { " ✏️ (edited)" } else { "" };
            let branch_marker = if let Some(name) = branch_name {
                format!(" 🌿 [{}]", name)
            } else {
                String::new()
            };

            text.push_str(&format!(
                "\n[{}] {}{}{}\n",
                i + 1,
                role_label,
                edited_marker,
                branch_marker
            ));
            text.push_str(&format!("{}\n", msg.content));
            text.push_str(&"-".repeat(40));
            text.push('\n');
        }

        text
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

    // Research panel - show conversation history in LLM mode
    let research_content = if app.llm_mode {
        app.format_history()
    } else {
        app.research_content.clone()
    };

    let research_panel = Paragraph::new(research_content.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if app.llm_mode {
                    "Conversation History"
                } else {
                    "Research View"
                })
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
    let provider_name = match app.llm_provider {
        LlmProvider::Ollama => "Ollama",
        LlmProvider::Anthropic => "Anthropic",
    };

    let mode_indicator = if app.llm_mode {
        format!(
            " [LLM CHAT ({}) - Node: {} - Press Enter to send]",
            provider_name, app.current_head
        )
    } else if app.execute_mode {
        format!(
            " [EXECUTE MODE - Node: {} - Press Enter to run]",
            app.current_head
        )
    } else {
        format!(
            " [EDIT MODE - Node: {} - Press Esc then Enter to run]",
            app.current_head
        )
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
