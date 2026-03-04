use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use fmpl_core::builtins::human::{APPROVAL_QUEUE, ApprovalRequest};
use fmpl_core::{StreamEvent, Value, Vm, eval};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
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
                    Some(StreamEvent::Done) => {
                        // Stream completed without value - return final data or null
                        if final_value != Value::Null {
                            return Ok(final_value);
                        }
                        return Ok(Value::Null);
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

/// Panel types for focus management (Phase 6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelType {
    Research,
    Planning,
    CodeEditor,
    Output,
    Tools, // Phase 7: Tool management panel
}

/// Task status for planning panel (Phase 6 Task 6.3)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskStatus {
    Pending,
    InProgress,
    Complete,
}

/// Task priority for planning panel (Phase 6 Task 6.3)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Priority {
    Low,
    Medium,
    High,
}

/// A planning task (Phase 6 Task 6.3)
#[derive(Clone, Debug)]
struct PlanningTask {
    id: usize,
    description: String,
    status: TaskStatus,
    priority: Priority,
}

/// A tool for LLM agent operations (Phase 7 Task 7.1)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Tool {
    id: String,
    name: String,
    description: String,
    enabled: bool,
    timeout_ms: u64,
    requires_confirmation: bool,
    usage_count: usize,
}

/// A tool execution request (Phase 9 Task 9.1)
#[derive(Clone, Debug)]
struct ToolRequest {
    tool_id: String,
    args: Vec<String>,
}

/// A tool execution result (Phase 9 Task 9.2)
#[derive(Clone, Debug)]
struct ToolResult {
    success: bool,
    output: String,
    error: Option<String>,
    duration_ms: u64,
}

// ============================================================================
// Command Stream - Code panel as command stream viewer/editor
// ============================================================================

/// Unique identifier for a command
type CommandId = usize;

/// Unique identifier for a stream branch
type BranchId = String;

/// State of a command in the stream
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
enum CommandState {
    Pending,  // Awaiting user decision
    Approved, // Approved for execution
    Denied,   // Denied/skipped by user
    Executed, // Successfully executed
    #[allow(dead_code)]
    Failed, // Execution failed
}

/// A tool invocation within a command
#[derive(Clone, Debug)]
struct ToolInvocation {
    tool_name: String,
    args: Vec<String>,
}

/// A grammar/policy rule match for a command
#[derive(Clone, Debug)]
struct RuleMatch {
    rule_name: String,
    matched: bool,
    message: String,
}

/// A command in the command stream
#[derive(Clone, Debug)]
struct CodeCommand {
    id: CommandId,
    parent_id: Option<CommandId>, // Parent in command DAG
    #[allow(dead_code)]
    linked_task_id: Option<usize>, // Links to planning task
    description: String,          // Human-readable description
    tool_call: ToolInvocation,    // The tool to invoke
    grammar_checks: Vec<RuleMatch>, // Policy/grammar validation
    state: CommandState,          // Current state
    #[allow(dead_code)]
    timestamp: String, // ISO timestamp
    execution_result: Option<String>, // Result if executed
}

struct App {
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
    current_head: NodeId,               // Current branch tip
    node_counter: NodeId,               // For generating IDs
    edit_mode: bool,                    // When true, editing last message in history
    editing_node_id: Option<NodeId>,    // Node being edited (None = new message)
    compaction_warning: Option<String>, // Warning message when off-track/circular detected
    // Phase 2: Backtracking UI
    selected_node_id: Option<NodeId>, // Currently selected node in history (for replay)
    history_selection_mode: bool,     // When true, arrow keys navigate history
    compare_branch_id: Option<NodeId>, // Branch to compare with (for diff view)
    diff_view_mode: bool,             // When true, show diff between branches
    // Phase 6: Panel interactivity
    focused_panel: PanelType,    // Currently focused panel
    research_lines: Vec<String>, // Research panel content (editable)
    research_cursor_row: usize,  // Research panel cursor row
    research_cursor_col: usize,  // Research panel cursor column
    // Phase 6 Task 6.3: Planning panel with task list
    planning_tasks: Vec<PlanningTask>, // Task list
    selected_task_index: usize,        // Currently selected task
    task_counter: usize,               // For generating task IDs
    // Phase 7 Task 7.1: Tool management
    tools: Vec<Tool>,           // Available tools
    selected_tool_index: usize, // Currently selected tool in tools panel
    // Phase 8 Task 8.4: LLM generation status
    llm_generation_status: Option<String>, // Current LLM operation ("Generating research summary...", etc.)
    // Command Stream: Code panel as command stream viewer/editor
    command_stream: Option<Value>, // The active async stream handle
    arrived_commands: Vec<CodeCommand>, // Commands that have arrived on current branch
    #[allow(dead_code)]
    command_cursor: usize, // Current position in command stream
    selected_command_index: usize, // Currently selected command (for UI)
    command_counter: CommandId,    // For generating command IDs
    stream_complete: bool,         // Has the stream terminated?
    stream_branches: HashMap<BranchId, Vec<CodeCommand>>, // All stream branches
    current_branch: BranchId,      // Which branch we're viewing
    branch_point: Option<usize>,   // Where current branch forked from parent
    command_edit_mode: bool,       // When true, editing selected command
    expanded_command: Option<CommandId>, // Command to show details for
    command_stream_mode: bool,     // When true, show command stream instead of code editor
    // Human-in-the-loop approval
    pending_approval: Option<ApprovalRequest>, // Current approval request awaiting user input
    denial_reason: String,                     // Buffer for denial reason text
    approval_mode: bool,                       // When true, showing approval prompt
}

impl App {
    fn new() -> Self {
        let mut vm = Vm::new();

        // Bootstrap LLM libraries
        let bootstrap_result = Self::bootstrap_llm(&mut vm);

        App {
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
            compaction_warning: None,
            // Phase 2: Initialize backtracking UI fields
            selected_node_id: None,
            history_selection_mode: false,
            compare_branch_id: None,
            diff_view_mode: false,
            // Phase 6: Initialize focused panel (default to code editor)
            focused_panel: PanelType::CodeEditor,
            // Phase 6 Task 6.2: Initialize research panel (try to load from file)
            research_lines: Self::load_research_notes(),
            research_cursor_row: 0,
            research_cursor_col: 0,
            // Phase 6 Task 6.3: Initialize planning panel (try to load from file)
            planning_tasks: Self::load_planning_tasks(),
            selected_task_index: 0,
            task_counter: 0,
            // Phase 7 Task 7.1: Initialize tools (load from file or use defaults)
            tools: Self::load_tools(),
            selected_tool_index: 0,
            // Phase 8 Task 8.4: Initialize LLM generation status
            llm_generation_status: None,
            // Command Stream: Initialize empty command stream
            command_stream: None,
            arrived_commands: Vec::new(),
            command_cursor: 0,
            selected_command_index: 0,
            command_counter: 0,
            stream_complete: false,
            stream_branches: {
                let mut map = HashMap::new();
                map.insert("main".to_string(), Vec::new());
                map
            },
            current_branch: "main".to_string(),
            branch_point: None,
            command_edit_mode: false,
            expanded_command: None,
            command_stream_mode: false,
            // Human-in-the-loop approval
            pending_approval: None,
            denial_reason: String::new(),
            approval_mode: false,
        }
    }

    fn bootstrap_llm(vm: &mut Vm) -> String {
        // Try to load LLM libraries
        let libraries = vec![
            "lib/llm-common.fmpl",
            "lib/ollama.fmpl",
            "lib/anthropic.fmpl",
            "lib/compaction.fmpl", // Phase 5: Auto-detection
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

    // Populate sample commands for testing
    fn populate_sample_commands(&mut self) {
        let now = chrono::Utc::now().to_rfc3339();

        self.arrived_commands = vec![
            CodeCommand {
                id: 0,
                parent_id: None,
                linked_task_id: Some(1),
                description: "Fetch user profile data".to_string(),
                tool_call: ToolInvocation {
                    tool_name: "curl.get".to_string(),
                    args: vec!["https://api.example.com/users/123".to_string()],
                },
                grammar_checks: vec![
                    RuleMatch {
                        rule_name: "url_validation".to_string(),
                        matched: true,
                        message: "URL format is valid".to_string(),
                    },
                    RuleMatch {
                        rule_name: "rate_limit_check".to_string(),
                        matched: true,
                        message: "Rate limit OK".to_string(),
                    },
                ],
                state: CommandState::Approved,
                timestamp: now.clone(),
                execution_result: Some("Success: User data retrieved".to_string()),
            },
            CodeCommand {
                id: 1,
                parent_id: Some(0),
                linked_task_id: Some(1),
                description: "Parse user JSON response".to_string(),
                tool_call: ToolInvocation {
                    tool_name: "json.parse".to_string(),
                    args: vec!["{...response data...}".to_string()],
                },
                grammar_checks: vec![],
                state: CommandState::Executed,
                timestamp: now.clone(),
                execution_result: Some("Parsed: User{name: \"Alice\", age: 30}".to_string()),
            },
            CodeCommand {
                id: 2,
                parent_id: Some(1),
                linked_task_id: Some(2),
                description: "Update user record in database".to_string(),
                tool_call: ToolInvocation {
                    tool_name: "db.update".to_string(),
                    args: vec![
                        "users".to_string(),
                        "123".to_string(),
                        "{age: 31}".to_string(),
                    ],
                },
                grammar_checks: vec![RuleMatch {
                    rule_name: "write_permission".to_string(),
                    matched: true,
                    message: "User has write access".to_string(),
                }],
                state: CommandState::Pending,
                timestamp: now.clone(),
                execution_result: None,
            },
            CodeCommand {
                id: 3,
                parent_id: Some(1),
                linked_task_id: Some(3),
                description: "Send confirmation email".to_string(),
                tool_call: ToolInvocation {
                    tool_name: "email.send".to_string(),
                    args: vec![
                        "alice@example.com".to_string(),
                        "Your profile was updated".to_string(),
                    ],
                },
                grammar_checks: vec![
                    RuleMatch {
                        rule_name: "email_validation".to_string(),
                        matched: true,
                        message: "Email format valid".to_string(),
                    },
                    RuleMatch {
                        rule_name: "content_policy".to_string(),
                        matched: false,
                        message: "Warning: Email contains sensitive keyword".to_string(),
                    },
                ],
                state: CommandState::Denied,
                timestamp: now,
                execution_result: None,
            },
        ];

        self.command_counter = 4;
        self.selected_command_index = 0;
        self.stream_complete = true;
    }

    // Phase 6 Task 6.2: Research panel persistence

    /// Load research notes from .agent/research.md
    fn load_research_notes() -> Vec<String> {
        match std::fs::read_to_string(".agent/research.md") {
            Ok(content) => {
                if content.trim().is_empty() {
                    vec![String::from("# Research Notes")]
                } else {
                    content.lines().map(String::from).collect()
                }
            }
            Err(_) => vec![String::from("# Research Notes")],
        }
    }

    /// Save research notes to .agent/research.md
    fn save_research_notes(&self) {
        let content = self.research_lines.join("\n");
        match std::fs::write(".agent/research.md", content) {
            Ok(_) => {
                // Success - silent save
            }
            Err(e) => {
                eprintln!("Warning: Failed to save research notes: {}", e);
            }
        }
    }

    // Phase 6 Task 6.3: Planning panel persistence

    /// Parse a task line from .agent/tasks.md
    /// Format: "- [ ] Task description [P]" where P is L/M/H
    fn parse_task_line(line: &str, max_id: &mut usize) -> Option<PlanningTask> {
        let trimmed = line.trim();

        // Check if it's a task line (starts with "- [")
        if !trimmed.starts_with("- [") {
            return None;
        }

        // Extract status marker (character after "- [")
        let status_char = trimmed.chars().nth(3)?;
        let status = match status_char {
            ' ' => TaskStatus::Pending,
            '>' => TaskStatus::InProgress,
            'x' | 'X' => TaskStatus::Complete,
            _ => return None,
        };

        // Extract description (between "] " and " [")
        let desc_start = trimmed.find("] ")? + 2;
        let desc_end = trimmed.rfind(" [")?;

        if desc_end <= desc_start {
            return None;
        }

        let description = trimmed[desc_start..desc_end].trim().to_string();

        // Extract priority (character between "[" and "]" at end)
        let priority_char = trimmed.chars().nth(trimmed.len() - 2)?;
        let priority = match priority_char {
            'L' | 'l' => Priority::Low,
            'M' | 'm' => Priority::Medium,
            'H' | 'h' => Priority::High,
            _ => return None,
        };

        *max_id += 1;
        Some(PlanningTask {
            id: *max_id,
            description,
            status,
            priority,
        })
    }

    /// Load planning tasks from .agent/tasks.md
    fn load_planning_tasks() -> Vec<PlanningTask> {
        match std::fs::read_to_string(".agent/tasks.md") {
            Ok(content) => {
                let mut tasks = Vec::new();
                let mut max_id = 0;

                for line in content.lines() {
                    if line.trim().is_empty() || line.starts_with("#") {
                        continue;
                    }

                    // Parse format: "- [ ] Task description [P]" or "- [x] Task description [P]"
                    // where P is L/M/H for priority
                    let task = Self::parse_task_line(line, &mut max_id);
                    if let Some(t) = task {
                        tasks.push(t);
                    }
                }

                tasks
            }
            Err(_) => Vec::new(),
        }
    }

    /// Save planning tasks to .agent/tasks.md
    fn save_planning_tasks(&self) {
        let mut content = String::from("# Planning Tasks\n\n");

        for task in &self.planning_tasks {
            let status_marker = match task.status {
                TaskStatus::Pending => " ",
                TaskStatus::InProgress => ">",
                TaskStatus::Complete => "x",
            };

            let priority_tag = match task.priority {
                Priority::Low => "[L]",
                Priority::Medium => "[M]",
                Priority::High => "[H]",
            };

            content.push_str(&format!(
                "- [{}] {} {}\n",
                status_marker, task.description, priority_tag
            ));
        }

        match std::fs::write(".agent/tasks.md", content) {
            Ok(_) => {
                // Success - silent save
            }
            Err(e) => {
                eprintln!("Warning: Failed to save planning tasks: {}", e);
            }
        }
    }

    // Phase 7 Task 7.1 & 7.4: Tool management persistence

    /// Load tools from .agent/tools.json, or return default tools
    fn load_tools() -> Vec<Tool> {
        // Try to load from JSON file
        match std::fs::read_to_string(".agent/tools.json") {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => {
                    if let Some(tools_array) = json.get("tools").and_then(|t| t.as_array()) {
                        let mut tools = Vec::new();
                        for tool_json in tools_array {
                            if let Ok(tool) = serde_json::from_value(tool_json.clone()) {
                                tools.push(tool);
                            }
                        }
                        if !tools.is_empty() {
                            return tools;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse .agent/tools.json: {}", e);
                }
            },
            Err(_) => {
                // File doesn't exist, will create with defaults
            }
        }

        // Return default tools
        vec![
            Tool {
                id: "grep".to_string(),
                name: "grep".to_string(),
                description: "Search codebase".to_string(),
                enabled: true,
                timeout_ms: 30000,
                requires_confirmation: false,
                usage_count: 0,
            },
            Tool {
                id: "file_read".to_string(),
                name: "file_read".to_string(),
                description: "Read file contents".to_string(),
                enabled: true,
                timeout_ms: 10000,
                requires_confirmation: false,
                usage_count: 0,
            },
            Tool {
                id: "bash_execute".to_string(),
                name: "bash_execute".to_string(),
                description: "Execute shell commands".to_string(),
                enabled: true,
                timeout_ms: 60000,
                requires_confirmation: true,
                usage_count: 0,
            },
            Tool {
                id: "llm_query".to_string(),
                name: "llm_query".to_string(),
                description: "Query LLM for assistance".to_string(),
                enabled: true,
                timeout_ms: 120000,
                requires_confirmation: false,
                usage_count: 0,
            },
        ]
    }

    /// Save tools to .agent/tools.json
    fn save_tools(&self) {
        use serde_json::json;

        let tools_json = json!({
            "tools": self.tools
        });

        match std::fs::write(
            ".agent/tools.json",
            serde_json::to_string_pretty(&tools_json).unwrap(),
        ) {
            Ok(_) => {
                // Success - silent save
            }
            Err(e) => {
                eprintln!("Warning: Failed to save tools: {}", e);
            }
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
    /// Returns: Vec<(NodeId, ChatMessage, bool, Option<String>)>
    fn get_history_with_metadata(&self) -> Vec<(NodeId, ChatMessage, bool, Option<String>)> {
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
        for (id, node) in path {
            history.push((
                id,
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

    // Phase 2: Backtracking UI functions

    /// Enter history selection mode (navigate through conversation with arrow keys)
    fn enter_history_selection(&mut self) -> Result<(), String> {
        if self.conversation_nodes.is_empty() {
            return Err("No conversation history to select from".to_string());
        }

        // Start by selecting the current head
        self.selected_node_id = Some(self.current_head);
        self.history_selection_mode = true;

        Ok(())
    }

    /// Exit history selection mode
    fn exit_history_selection(&mut self) {
        self.selected_node_id = None;
        self.history_selection_mode = false;
    }

    /// Move selection to next (newer) message
    fn select_next_message(&mut self) -> Result<(), String> {
        let selected = self.selected_node_id.ok_or("No node selected")?;

        // Find children of the selected node
        let children: Vec<NodeId> = self
            .conversation_nodes
            .values()
            .filter(|node| node.parent_id == Some(selected))
            .map(|node| node.id)
            .collect();

        if children.is_empty() {
            return Err("Already at the newest message".to_string());
        }

        // Select the first child (simple version)
        self.selected_node_id = Some(children[0]);
        Ok(())
    }

    /// Move selection to previous (older) message
    fn select_prev_message(&mut self) -> Result<(), String> {
        let selected = self.selected_node_id.ok_or("No node selected")?;

        let node = self
            .conversation_nodes
            .get(&selected)
            .ok_or("Selected node not found")?;

        match node.parent_id {
            Some(parent) => {
                self.selected_node_id = Some(parent);
                Ok(())
            }
            None => Err("Already at the oldest message".to_string()),
        }
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

    /// Phase 2 Task 2.2: Replay conversation from selected node
    /// Creates new branch and regenerates LLM responses from selected point
    fn replay_from_node(&mut self, node_id: NodeId) -> Result<(), String> {
        // Verify node exists
        if !self.conversation_nodes.contains_key(&node_id) {
            return Err(format!("Node {} not found", node_id));
        }

        // Store original branch head for comparison (Phase 2 Task 2.3 - diff view)
        let original_head = self.current_head;
        self.compare_branch_id = Some(original_head);

        // Generate branch name with timestamp
        let branch_name = format!("replay-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));

        // Get history up to and including the selected node
        let mut current_id = node_id;

        // Traverse backwards from selected node to root
        let mut path = Vec::new();
        while let Some(node) = self.conversation_nodes.get(&current_id) {
            path.push((current_id, node.clone()));
            match node.parent_id {
                Some(parent) => current_id = parent,
                None => break,
            }
        }

        // Reverse to get root → selected_node order
        path.reverse();

        // Extract messages up to selected node
        let mut node_chain: Vec<NodeId> = Vec::new();
        for (id, _node) in &path {
            node_chain.push(*id);
        }

        // Find all nodes after the selected point in original conversation
        // These are the assistant messages we need to regenerate
        let mut nodes_to_regenerate: Vec<NodeId> = Vec::new();
        let mut temp_id = original_head;

        // Walk back from original head until we hit the selected node
        while let Some(node) = self.conversation_nodes.get(&temp_id) {
            if temp_id == node_id {
                break;
            }
            if node.message.role == "assistant" {
                nodes_to_regenerate.push(temp_id);
            }
            match node.parent_id {
                Some(parent) => temp_id = parent,
                None => break,
            }
        }

        // Reverse to regenerate in chronological order
        nodes_to_regenerate.reverse();

        // If no assistant messages to regenerate, just create a branch point
        if nodes_to_regenerate.is_empty() {
            // Set current head to selected node
            self.current_head = node_id;

            // Mark branch on the node
            if let Some(node) = self.conversation_nodes.get_mut(&node_id) {
                node.metadata.branch_name = Some(branch_name.clone());
            }

            self.output = format!(
                "🔄 Created branch '{}' from node {}\n\nNo assistant messages to regenerate.\nUse Ctrl+L to chat and extend this branch.",
                branch_name, node_id
            );
            return Ok(());
        }

        // Start replay: set head to selected node
        self.current_head = node_id;

        // Mark selected node as branch point
        if let Some(node) = self.conversation_nodes.get_mut(&node_id) {
            node.metadata.branch_name = Some(branch_name.clone());
        }

        // Regenerate each assistant message
        let mut regenerated_count = 0;
        let provider_name = match self.llm_provider {
            LlmProvider::Ollama => "ollama",
            LlmProvider::Anthropic => "anthropic",
        };

        for original_node_id in nodes_to_regenerate {
            // Get the user prompt that led to this assistant response
            let user_prompt_node_id = self
                .conversation_nodes
                .get(&original_node_id)
                .and_then(|node| node.parent_id)
                .ok_or("Cannot find user prompt for assistant message")?;

            let user_prompt_node = self
                .conversation_nodes
                .get(&user_prompt_node_id)
                .ok_or("User prompt node not found")?;

            if user_prompt_node.message.role != "user" {
                continue;
            }

            let user_prompt = &user_prompt_node.message.content;

            // Add user message to new branch
            self.add_message(ChatMessage {
                role: "user".to_string(),
                content: user_prompt.clone(),
            });

            // Get current history for LLM context
            let messages_array = self.format_history_as_fmpl();
            let fmpl_code = format!("{}.chat_with_history({})", provider_name, messages_array);

            // Call LLM
            match eval(&mut self.vm, &fmpl_code) {
                Ok(result) => {
                    match wait_for_async(result) {
                        Ok(Value::String(response)) => {
                            // Add assistant response to new branch
                            self.add_message(ChatMessage {
                                role: "assistant".to_string(),
                                content: response.to_string(),
                            });

                            // Mark new nodes with branch name
                            if let Some(node) = self.conversation_nodes.get_mut(&self.current_head)
                            {
                                node.metadata.branch_name = Some(branch_name.clone());
                            }

                            regenerated_count += 1;
                        }
                        Ok(other) => {
                            return Err(format!("Unexpected response type: {:?}", other));
                        }
                        Err(e) => {
                            return Err(format!("LLM error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    return Err(format!("FMPL eval error: {}", e));
                }
            }
        }

        self.output = format!(
            "🔄 Replayed conversation from node {}\n\nBranch: '{}'\nRegenerated {} assistant responses\n\nYou are now on the new branch. Use Ctrl+Z to move back to original branch.",
            node_id, branch_name, regenerated_count
        );

        Ok(())
    }

    /// Check APPROVAL_QUEUE for pending requests and promote to active
    fn check_approval_queue(&mut self) {
        if self.pending_approval.is_some() {
            return; // Already handling one
        }
        APPROVAL_QUEUE.with(|q| {
            let mut queue = q.lock().unwrap();
            if !queue.is_empty() {
                let request = queue.remove(0);
                self.output = format!(
                    "🔔 APPROVAL REQUIRED\n\nAction: {}\n\n[y] Approve  [n] Deny  [r] Deny with reason",
                    request.action
                );
                self.pending_approval = Some(request);
                self.approval_mode = true;
                self.denial_reason.clear();
            }
        });
    }

    /// Send approval response through the request's tx channel
    fn send_approval_response(&mut self, approved: bool, reason: Option<String>) {
        if let Some(request) = self.pending_approval.take() {
            let tx = request.tx.clone();
            let response = if approved {
                let mut map = HashMap::new();
                map.insert(SmolStr::new("approved"), Value::Bool(true));
                Value::Map(Arc::new(map))
            } else {
                let mut map = HashMap::new();
                map.insert(
                    SmolStr::new("denied"),
                    Value::String(SmolStr::new(reason.as_deref().unwrap_or("User denied"))),
                );
                Value::Map(Arc::new(map))
            };

            // Send response through the channel (try_lock since we're sync)
            let response_clone = response.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut guard = tx.lock().await;
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(StreamEvent::Ok(response_clone)).await;
                    }
                });
            });

            self.approval_mode = false;
            self.denial_reason.clear();
            self.output = if approved {
                "✅ Approved".to_string()
            } else {
                format!(
                    "❌ Denied: {}",
                    reason.unwrap_or_else(|| "User denied".to_string())
                )
            };
        }
    }

    /// Handle key input during approval mode
    fn handle_approval_input(&mut self, key: KeyEvent) {
        if self.denial_reason.is_empty() {
            // Waiting for initial y/n/r decision
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.send_approval_response(true, None);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.send_approval_response(false, None);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.output = "🔔 DENIAL REASON\n\nType reason and press Enter:".to_string();
                    self.denial_reason = " ".to_string(); // Non-empty to enter reason mode
                }
                KeyCode::Esc => {
                    self.send_approval_response(false, Some("Cancelled".to_string()));
                }
                _ => {}
            }
        } else {
            // Typing denial reason
            match key.code {
                KeyCode::Enter => {
                    let reason = self.denial_reason.trim().to_string();
                    self.send_approval_response(
                        false,
                        Some(if reason.is_empty() {
                            "User denied".to_string()
                        } else {
                            reason
                        }),
                    );
                }
                KeyCode::Char(c) => {
                    if self.denial_reason == " " {
                        self.denial_reason = String::new();
                    }
                    self.denial_reason.push(c);
                    self.output = format!(
                        "🔔 DENIAL REASON\n\nType reason and press Enter:\n> {}",
                        self.denial_reason
                    );
                }
                KeyCode::Backspace => {
                    if self.denial_reason != " " {
                        self.denial_reason.pop();
                    }
                    self.output = format!(
                        "🔔 DENIAL REASON\n\nType reason and press Enter:\n> {}",
                        self.denial_reason
                    );
                }
                KeyCode::Esc => {
                    self.send_approval_response(false, Some("Cancelled".to_string()));
                }
                _ => {}
            }
        }
    }

    fn handle_input(&mut self, key: KeyEvent) {
        // Intercept input during approval mode
        if self.approval_mode {
            self.handle_approval_input(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Toggle LLM mode
                self.llm_mode = !self.llm_mode;
                self.update_mode_indicator();
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Toggle command stream mode
                self.command_stream_mode = !self.command_stream_mode;
                self.output = if self.command_stream_mode {
                    "Command Stream Mode: ON".to_string()
                } else {
                    "Command Stream Mode: OFF".to_string()
                };
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
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Rewind upstream in command stream (navigate to parent command)
                if self.command_stream_mode && !self.arrived_commands.is_empty() {
                    let idx = self
                        .selected_command_index
                        .min(self.arrived_commands.len() - 1);
                    if let Some(parent_id) = self.arrived_commands[idx].parent_id {
                        // Find parent command
                        if let Some(parent_idx) =
                            self.arrived_commands.iter().position(|c| c.id == parent_id)
                        {
                            self.selected_command_index = parent_idx;
                            self.output = format!("Rewound to parent command {}", parent_id);
                        } else {
                            self.output =
                                format!("Parent command {} not found in stream", parent_id);
                        }
                    } else {
                        self.output = "Selected command has no parent".to_string();
                    }
                } else {
                    self.output = "Rewind only available in command stream mode".to_string();
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
                // Command stream mode: create branch at current position
                // Otherwise: list all branches
                if self.command_stream_mode && !self.arrived_commands.is_empty() {
                    let new_branch = format!("cmd-branch-{}", self.stream_branches.len());
                    // Clone current commands to new branch
                    let current_commands = self.arrived_commands.clone();
                    self.stream_branches
                        .insert(new_branch.clone(), current_commands);
                    self.branch_point = Some(self.selected_command_index);
                    self.output = format!(
                        "Created command branch '{}' at position {}",
                        new_branch, self.selected_command_index
                    );
                } else {
                    self.output = self.list_branches();
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Phase 5: Compact conversation if warning is present
                if self.compaction_warning.is_some() {
                    // Actually perform compaction
                    self.output = self.perform_compaction();
                } else {
                    // No warning, but user can still manually compact if they want
                    let history = self.get_history();
                    if history.len() > 5 {
                        // Offer manual compaction for long conversations
                        self.output = format!(
                            "No compaction warning active, but conversation has {} messages.\n\n\
                             Press Ctrl+C again to force compaction, or continue chatting.\n\n\
                             Compaction suggestions appear automatically when:\n\
                             - Agent goes off-track (groveling, apologizing)\n\
                             - Circular conversation detected (repeated responses)",
                            history.len()
                        );
                        // Set a temporary warning to allow force-compact on next Ctrl+C
                        self.compaction_warning = Some("Manual compaction requested".to_string());
                    } else {
                        self.output = String::from(
                            "Conversation too short to compact (< 5 messages).\n\nCompaction suggestions appear when the agent goes off-track or enters a circular conversation.",
                        );
                    }
                }
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Phase 2: Enter history selection mode
                if self.llm_mode {
                    match self.enter_history_selection() {
                        Ok(()) => {
                            self.output = String::from(
                                "History selection mode.\n\nUse ↑/↓ to navigate, Enter to replay from here, Esc to exit.",
                            );
                        }
                        Err(e) => {
                            self.output = format!("Failed to enter history selection: {}", e);
                        }
                    }
                } else {
                    self.output = String::from(
                        "History selection requires LLM mode.\n\nUse Ctrl+L to enter LLM chat mode first.",
                    );
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Phase 2 Task 2.3: Toggle diff view mode
                if self.llm_mode {
                    if self.compare_branch_id.is_some() {
                        self.diff_view_mode = !self.diff_view_mode;
                        if self.diff_view_mode {
                            self.output = String::from(
                                "Diff view enabled.\n\nShowing comparison between current branch and saved comparison branch.\nPress Ctrl+D to exit diff view.",
                            );
                        } else {
                            self.output = String::from(
                                "Diff view disabled.\n\nShowing normal conversation history.",
                            );
                        }
                    } else {
                        self.output = String::from(
                            "No comparison branch available.\n\nUse Ctrl+H to select a node, then Enter to replay from there.\nThis creates a new branch and saves the original for diff viewing.",
                        );
                    }
                } else {
                    self.output = String::from(
                        "Diff view requires LLM mode.\n\nUse Ctrl+L to enter LLM chat mode first.",
                    );
                }
            }
            // Phase 6: Panel focus switching
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.focused_panel = PanelType::Research;
                self.output = String::from(
                    "Research panel focused.\n\nArrow keys navigate content when editable (Phase 6.2).",
                );
            }
            KeyCode::Char('p')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.llm_mode =>
            {
                // Only switch to planning panel if not in LLM mode (Ctrl+P switches provider in LLM mode)
                self.focused_panel = PanelType::Planning;
                self.output = String::from(
                    "Planning panel focused.\n\nArrow keys navigate tasks when editable (Phase 6.3).",
                );
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Phase 7 Task 7.2: Focus tools panel
                self.focused_panel = PanelType::Tools;
                self.output =
                    String::from("Tools panel focused.\n\nArrow keys navigate tools list.");
            }
            KeyCode::Char('e')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !self.edit_mode =>
            {
                // Only switch to code editor if not in edit mode (Ctrl+E enters edit mode)
                self.focused_panel = PanelType::CodeEditor;
                self.output =
                    String::from("Code editor focused.\n\nType to edit, Esc+Enter to execute.");
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.focused_panel = PanelType::Output;
                self.output = String::from(
                    "Output panel focused.\n\nView execution results and LLM responses.",
                );
            }
            // Phase 6 Task 6.2: Ctrl+S to save research notes
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focused_panel == PanelType::Research {
                    self.save_research_notes();
                    self.output = String::from(
                        "Research notes saved to .agent/research.md\n\nUse Ctrl+R to focus research panel, then type to edit.",
                    );
                } else if self.focused_panel == PanelType::Planning {
                    // Phase 6 Task 6.3: Save planning tasks
                    self.save_planning_tasks();
                    self.output = String::from(
                        "Planning tasks saved to .agent/tasks.md\n\nUse Ctrl+P to focus planning panel.",
                    );
                }
            }
            // Shift+S: Populate sample commands (for testing)
            KeyCode::Char('S') => {
                self.populate_sample_commands();
                self.output =
                    String::from("Populated sample commands. Press Ctrl+T to view command stream.");
            }
            // Phase 8 Task 8.2 & 8.3: Ctrl+G to generate AI assistance
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focused_panel == PanelType::Research {
                    // Generate research summary from conversation
                    match self.generate_research_summary() {
                        Ok(summary) => {
                            // Append summary to research lines
                            self.research_lines.push(String::new()); // blank line before
                            self.research_lines
                                .push(String::from("# AI-Generated Summary"));
                            self.research_lines.push(String::new());
                            for line in summary.lines() {
                                self.research_lines.push(line.to_string());
                            }
                            self.save_research_notes();
                            self.output = String::from(
                                "Research summary generated and appended to research panel.\n\nUse Ctrl+R to view.",
                            );
                        }
                        Err(e) => {
                            self.output = format!(
                                "Failed to generate research summary: {}\n\nMake sure you have a conversation history first (use Ctrl+L to chat with LLM).",
                                e
                            );
                        }
                    }
                } else if self.focused_panel == PanelType::Planning {
                    // Generate planning tasks from conversation
                    match self.generate_planning_tasks() {
                        Ok(tasks) => {
                            let task_count = tasks.len();
                            // Add tasks to planning panel
                            for task_description in tasks {
                                self.task_counter += 1;
                                self.planning_tasks.push(PlanningTask {
                                    id: self.task_counter,
                                    description: task_description,
                                    status: TaskStatus::Pending,
                                    priority: Priority::Medium,
                                });
                            }
                            self.save_planning_tasks();
                            self.selected_task_index =
                                self.planning_tasks.len().saturating_sub(task_count);
                            self.output = format!(
                                "Generated {} tasks from conversation.\n\nUse Ctrl+P to view planning panel.",
                                task_count
                            );
                        }
                        Err(e) => {
                            self.output = format!(
                                "Failed to generate tasks: {}\n\nMake sure you have a conversation history first (use Ctrl+L to chat with LLM).",
                                e
                            );
                        }
                    }
                }
            }
            // Phase 6 Task 6.3: Task management keybindings (when planning panel focused)
            KeyCode::Char('a') if self.focused_panel == PanelType::Planning => {
                self.task_counter += 1;
                self.planning_tasks.push(PlanningTask {
                    id: self.task_counter,
                    description: String::from("New task"),
                    status: TaskStatus::Pending,
                    priority: Priority::Medium,
                });
                self.selected_task_index = self.planning_tasks.len().saturating_sub(1);
                self.save_planning_tasks();
                self.output = format!(
                    "Task {} added.\n\nPress 'e' to edit description.\nPress Enter to toggle status.\nPress +/- to change priority.",
                    self.task_counter
                );
            }
            KeyCode::Char('d')
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                let removed = self.planning_tasks.remove(self.selected_task_index);
                if self.selected_task_index >= self.planning_tasks.len()
                    && !self.planning_tasks.is_empty()
                {
                    self.selected_task_index = self.planning_tasks.len() - 1;
                }
                self.save_planning_tasks();
                self.output = format!(
                    "Task deleted: {}\n\n{} tasks remaining.",
                    removed.description,
                    self.planning_tasks.len()
                );
            }
            KeyCode::Char('e')
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                // Toggle through descriptions: "Task 1" -> "Task 2" -> etc.
                // In a real implementation, this would enter edit mode for the description
                let task_id = self.planning_tasks[self.selected_task_index].id;
                self.planning_tasks[self.selected_task_index].description =
                    format!("Edited task {}", task_id);
                self.save_planning_tasks();
                self.output = format!(
                    "Task {} description updated.\n\n(Full edit mode coming soon)",
                    task_id
                );
            }
            KeyCode::Enter
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                // Toggle status: Pending -> InProgress -> Complete -> Pending
                let task_id = self.planning_tasks[self.selected_task_index].id;
                let new_status = match self.planning_tasks[self.selected_task_index].status {
                    TaskStatus::Pending => TaskStatus::InProgress,
                    TaskStatus::InProgress => TaskStatus::Complete,
                    TaskStatus::Complete => TaskStatus::Pending,
                };
                self.planning_tasks[self.selected_task_index].status = new_status;
                self.save_planning_tasks();
                let status_str = match new_status {
                    TaskStatus::Pending => "Pending",
                    TaskStatus::InProgress => "In Progress",
                    TaskStatus::Complete => "Complete",
                };
                self.output = format!(
                    "Task {} status: {}\n\nPress Enter to cycle again.",
                    task_id, status_str
                );
            }
            KeyCode::Char('+')
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                // Increase priority: Low -> Medium -> High
                let idx = self.selected_task_index;
                let task_id = self.planning_tasks[idx].id;
                let new_priority = match self.planning_tasks[idx].priority {
                    Priority::Low => Priority::Medium,
                    Priority::Medium => Priority::High,
                    Priority::High => Priority::High, // Already at max
                };
                self.planning_tasks[idx].priority = new_priority;
                self.save_planning_tasks();
                let priority_str = match new_priority {
                    Priority::Low => "Low",
                    Priority::Medium => "Medium",
                    Priority::High => "High",
                };
                self.output = format!(
                    "Task {} priority: {}\n\nPress - to decrease.",
                    task_id, priority_str
                );
            }
            KeyCode::Char('-')
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                // Decrease priority: High -> Medium -> Low
                let idx = self.selected_task_index;
                let task_id = self.planning_tasks[idx].id;
                let new_priority = match self.planning_tasks[idx].priority {
                    Priority::Low => Priority::Low, // Already at min
                    Priority::Medium => Priority::Low,
                    Priority::High => Priority::Medium,
                };
                self.planning_tasks[idx].priority = new_priority;
                self.save_planning_tasks();
                let priority_str = match new_priority {
                    Priority::Low => "Low",
                    Priority::Medium => "Medium",
                    Priority::High => "High",
                };
                self.output = format!(
                    "Task {} priority: {}\n\nPress + to increase.",
                    task_id, priority_str
                );
            }
            // Arrow keys for task selection when planning panel focused
            KeyCode::Up
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                if self.selected_task_index > 0 {
                    self.selected_task_index -= 1;
                }
            }
            KeyCode::Down
                if self.focused_panel == PanelType::Planning && !self.planning_tasks.is_empty() =>
            {
                if self.selected_task_index < self.planning_tasks.len() - 1 {
                    self.selected_task_index += 1;
                }
            }
            // Phase 7 Task 7.2: Arrow keys for tool selection when tools panel focused
            KeyCode::Up if self.focused_panel == PanelType::Tools && !self.tools.is_empty() => {
                if self.selected_tool_index > 0 {
                    self.selected_tool_index -= 1;
                }
            }
            KeyCode::Down if self.focused_panel == PanelType::Tools && !self.tools.is_empty() => {
                if self.selected_tool_index < self.tools.len() - 1 {
                    self.selected_tool_index += 1;
                }
            }
            // Phase 7 Task 7.5: Tool management keybindings
            KeyCode::Char('u') if self.focused_panel == PanelType::Tools => {
                // Reset usage statistics for all tools
                for tool in &mut self.tools {
                    tool.usage_count = 0;
                }
                self.output = format!(
                    "Tool usage statistics reset for {} tools.\n\nPress Ctrl+S to save changes.",
                    self.tools.len()
                );
                self.save_tools(); // Auto-save on reset
            }
            KeyCode::Enter if self.focused_panel == PanelType::Tools && !self.tools.is_empty() => {
                // Toggle tool enabled/disabled
                let tool = &mut self.tools[self.selected_tool_index];
                tool.enabled = !tool.enabled;
                self.output = format!(
                    "Tool '{}' {}.\n\nPress Ctrl+S to save changes.",
                    tool.name,
                    if tool.enabled { "enabled" } else { "disabled" }
                );
                self.save_tools(); // Auto-save on toggle
            }
            KeyCode::Esc => {
                // Phase 2: Exit history selection mode
                if self.history_selection_mode {
                    self.exit_history_selection();
                    self.output = String::from("History selection mode exited.");
                }
                // If in edit mode, cancel and return to normal mode
                else if self.edit_mode {
                    self.edit_mode = false;
                    self.editing_node_id = None;
                    self.code_lines = vec![String::new()];
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                    self.output = String::from("Edit mode cancelled");
                }
                // Command stream mode: deny selected command
                else if self.command_stream_mode && self.focused_panel == PanelType::CodeEditor {
                    if !self.arrived_commands.is_empty() {
                        let idx = self
                            .selected_command_index
                            .min(self.arrived_commands.len() - 1);
                        self.arrived_commands[idx].state = CommandState::Denied;
                        self.output = format!(
                            "Denied command {}: {}",
                            self.arrived_commands[idx].id, self.arrived_commands[idx].description
                        );
                    }
                } else {
                    self.execute_mode = !self.execute_mode;
                }
            }
            // Phase 2: History selection navigation
            KeyCode::Up if self.history_selection_mode => {
                match self.select_prev_message() {
                    Ok(()) => {
                        // Navigation successful, display will update automatically
                    }
                    Err(e) => {
                        self.output = format!("Navigation: {}", e);
                    }
                }
            }
            KeyCode::Down if self.history_selection_mode => {
                match self.select_next_message() {
                    Ok(()) => {
                        // Navigation successful, display will update automatically
                    }
                    Err(e) => {
                        self.output = format!("Navigation: {}", e);
                    }
                }
            }
            KeyCode::Enter if self.history_selection_mode => {
                // Phase 2 Task 2.2: "Replay from here" - regenerate LLM responses from selected point
                if let Some(selected_id) = self.selected_node_id {
                    match self.replay_from_node(selected_id) {
                        Ok(()) => {
                            // Exit history selection mode after successful replay
                            self.exit_history_selection();
                        }
                        Err(e) => {
                            self.output = format!("Replay failed: {}", e);
                            self.exit_history_selection();
                        }
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Space bar: expand/collapse command details in command stream mode
                if self.command_stream_mode && self.focused_panel == PanelType::CodeEditor {
                    if !self.arrived_commands.is_empty() {
                        let idx = self
                            .selected_command_index
                            .min(self.arrived_commands.len() - 1);
                        let cmd_id = self.arrived_commands[idx].id;
                        // Toggle expansion
                        if self.expanded_command == Some(cmd_id) {
                            self.expanded_command = None;
                        } else {
                            self.expanded_command = Some(cmd_id);
                        }
                    }
                } else {
                    // Regular space handling in code editor
                    match self.focused_panel {
                        PanelType::Research => self.research_insert_char(' '),
                        PanelType::CodeEditor => self.insert_char(' '),
                        _ => {}
                    }
                }
            }
            KeyCode::Char(c) => {
                // Command stream mode: check for special keys first
                if self.command_stream_mode && self.focused_panel == PanelType::CodeEditor {
                    match c {
                        'e' | 'E' => {
                            // Edit selected command description
                            if !self.arrived_commands.is_empty() {
                                let idx = self
                                    .selected_command_index
                                    .min(self.arrived_commands.len() - 1);
                                self.command_edit_mode = true;
                                self.output = format!(
                                    "Editing command {}: {}",
                                    self.arrived_commands[idx].id,
                                    self.arrived_commands[idx].description
                                );
                                // TODO: Enter actual edit mode for description
                            }
                        }
                        'i' | 'I' => {
                            // Insert new command at current position
                            let new_id = self.command_counter;
                            self.command_counter += 1;
                            let new_cmd = CodeCommand {
                                id: new_id,
                                parent_id: if self.arrived_commands.is_empty() {
                                    None
                                } else {
                                    Some(
                                        self.arrived_commands[self
                                            .selected_command_index
                                            .min(self.arrived_commands.len() - 1)]
                                        .id,
                                    )
                                },
                                linked_task_id: None,
                                description: "New command".to_string(),
                                tool_call: ToolInvocation {
                                    tool_name: "placeholder".to_string(),
                                    args: vec![],
                                },
                                grammar_checks: vec![],
                                state: CommandState::Pending,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                execution_result: None,
                            };
                            self.arrived_commands
                                .insert(self.selected_command_index, new_cmd);
                            self.output = format!("Inserted new command {}", new_id);
                        }
                        'd' | 'D' => {
                            // Delete selected command
                            if !self.arrived_commands.is_empty() {
                                let idx = self
                                    .selected_command_index
                                    .min(self.arrived_commands.len() - 1);
                                let cmd_id = self.arrived_commands[idx].id;
                                self.arrived_commands.remove(idx);
                                if !self.arrived_commands.is_empty()
                                    && self.selected_command_index >= self.arrived_commands.len()
                                {
                                    self.selected_command_index = self.arrived_commands.len() - 1;
                                }
                                self.output = format!("Deleted command {}", cmd_id);
                            }
                        }
                        _ => {
                            // Other chars - ignore in command stream mode
                        }
                    }
                } else {
                    // Phase 6 Task 6.2: Route to focused panel
                    match self.focused_panel {
                        PanelType::Research => self.research_insert_char(c),
                        PanelType::CodeEditor => self.insert_char(c),
                        _ => {}
                    }
                }
            }
            KeyCode::Backspace => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => self.research_backspace(),
                    PanelType::CodeEditor => self.backspace(),
                    _ => {}
                }
            }
            KeyCode::Delete => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => self.research_delete(),
                    PanelType::CodeEditor => self.delete(),
                    _ => {}
                }
            }
            KeyCode::Enter => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => {
                        self.research_insert_newline();
                    }
                    PanelType::CodeEditor => {
                        if self.command_stream_mode {
                            // Approve selected command
                            if !self.arrived_commands.is_empty() {
                                let idx = self
                                    .selected_command_index
                                    .min(self.arrived_commands.len() - 1);
                                self.arrived_commands[idx].state = CommandState::Approved;
                                self.output = format!(
                                    "Approved command {}: {}",
                                    self.arrived_commands[idx].id,
                                    self.arrived_commands[idx].description
                                );
                                // TODO: Actually execute the command
                            }
                        } else {
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
                    }
                    _ => {}
                }
            }
            KeyCode::Left => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => self.research_cursor_left(),
                    PanelType::CodeEditor => {
                        if self.cursor_col > 0 {
                            self.cursor_col -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Right => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => self.research_cursor_right(),
                    PanelType::CodeEditor => {
                        let line_len = self.code_lines[self.cursor_row].len();
                        if self.cursor_col < line_len {
                            self.cursor_col += 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Up => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => {
                        self.research_cursor_up();
                    }
                    PanelType::CodeEditor => {
                        if self.command_stream_mode {
                            // Navigate command stream
                            if self.selected_command_index > 0 {
                                self.selected_command_index -= 1;
                            }
                        } else {
                            // Original code editor navigation
                            if self.cursor_row > 0 {
                                self.cursor_row -= 1;
                                let line_len = self.code_lines[self.cursor_row].len();
                                self.cursor_col = self.cursor_col.min(line_len);
                                self.adjust_scroll();
                            }
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => {
                        self.research_cursor_down();
                    }
                    PanelType::CodeEditor => {
                        if self.command_stream_mode {
                            // Navigate command stream
                            if !self.arrived_commands.is_empty()
                                && self.selected_command_index < self.arrived_commands.len() - 1
                            {
                                self.selected_command_index += 1;
                            }
                        } else {
                            // Original code editor navigation
                            if self.cursor_row < self.code_lines.len() - 1 {
                                self.cursor_row += 1;
                                let line_len = self.code_lines[self.cursor_row].len();
                                self.cursor_col = self.cursor_col.min(line_len);
                                self.adjust_scroll();
                            }
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Home => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => self.research_cursor_home(),
                    PanelType::CodeEditor => {
                        self.cursor_col = 0;
                    }
                    _ => {}
                }
            }
            KeyCode::End => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => self.research_cursor_end(),
                    PanelType::CodeEditor => {
                        self.cursor_col = self.code_lines[self.cursor_row].len();
                    }
                    _ => {}
                }
            }
            KeyCode::Tab => {
                // Phase 6 Task 6.2: Route to focused panel
                match self.focused_panel {
                    PanelType::Research => {
                        // Insert 4 spaces in research panel
                        for _ in 0..4 {
                            self.research_insert_char(' ');
                        }
                    }
                    PanelType::CodeEditor => {
                        self.insert_str("    ");
                    }
                    _ => {}
                }
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

    // Phase 6 Task 6.2: Research panel editing methods

    fn research_insert_char(&mut self, c: char) {
        // Ensure we have at least one line
        if self.research_lines.is_empty() {
            self.research_lines.push(String::new());
        }

        let row = self.research_cursor_row.min(self.research_lines.len() - 1);
        let col = self.research_cursor_col.min(self.research_lines[row].len());

        self.research_lines[row].insert(col, c);
        self.research_cursor_col += 1;
    }

    fn research_backspace(&mut self) {
        if self.research_lines.is_empty() {
            return;
        }

        let row = self.research_cursor_row.min(self.research_lines.len() - 1);
        let col = self.research_cursor_col.min(self.research_lines[row].len());

        if col > 0 {
            self.research_lines[row].remove(col - 1);
            self.research_cursor_col -= 1;
        } else if row > 0 {
            // Merge with previous line
            let prev_line_len = self.research_lines[row - 1].len();
            let current_line = self.research_lines.remove(row);
            self.research_lines[row - 1].push_str(&current_line);
            self.research_cursor_row -= 1;
            self.research_cursor_col = prev_line_len;
        }
    }

    fn research_delete(&mut self) {
        if self.research_lines.is_empty() {
            return;
        }

        let row = self.research_cursor_row.min(self.research_lines.len() - 1);
        let col = self.research_cursor_col.min(self.research_lines[row].len());

        let line_len = self.research_lines[row].len();
        if col < line_len {
            self.research_lines[row].remove(col);
        } else if row < self.research_lines.len() - 1 {
            // Merge with next line
            let next_line = self.research_lines.remove(row + 1);
            self.research_lines[row].push_str(&next_line);
        }
    }

    fn research_insert_newline(&mut self) {
        if self.research_lines.is_empty() {
            self.research_lines.push(String::new());
            return;
        }

        let row = self.research_cursor_row.min(self.research_lines.len() - 1);
        let col = self.research_cursor_col.min(self.research_lines[row].len());

        let new_line = self.research_lines[row].split_off(col);
        self.research_lines.insert(row + 1, new_line);
        self.research_cursor_row += 1;
        self.research_cursor_col = 0;
    }

    fn research_cursor_left(&mut self) {
        if self.research_cursor_col > 0 {
            self.research_cursor_col -= 1;
        }
    }

    fn research_cursor_right(&mut self) {
        if self.research_lines.is_empty() {
            return;
        }

        let row = self.research_cursor_row.min(self.research_lines.len() - 1);
        let line_len = self.research_lines[row].len();
        if self.research_cursor_col < line_len {
            self.research_cursor_col += 1;
        }
    }

    fn research_cursor_up(&mut self) {
        if self.research_cursor_row > 0 {
            self.research_cursor_row -= 1;
            let line_len = self.research_lines[self.research_cursor_row].len();
            self.research_cursor_col = self.research_cursor_col.min(line_len);
        }
    }

    fn research_cursor_down(&mut self) {
        if !self.research_lines.is_empty()
            && self.research_cursor_row < self.research_lines.len() - 1
        {
            self.research_cursor_row += 1;
            let line_len = self.research_lines[self.research_cursor_row].len();
            self.research_cursor_col = self.research_cursor_col.min(line_len);
        }
    }

    fn research_cursor_home(&mut self) {
        self.research_cursor_col = 0;
    }

    fn research_cursor_end(&mut self) {
        if !self.research_lines.is_empty() {
            let row = self.research_cursor_row.min(self.research_lines.len() - 1);
            self.research_cursor_col = self.research_lines[row].len();
        }
    }

    fn update_mode_indicator(&mut self) {
        let provider_name = match self.llm_provider {
            LlmProvider::Ollama => "Ollama",
            LlmProvider::Anthropic => "Anthropic",
        };

        let mode = if self.history_selection_mode {
            "History Selection".to_string()
        } else if self.edit_mode {
            "EDIT (last message)".to_string()
        } else if self.llm_mode {
            format!("LLM ({})", provider_name)
        } else {
            "Execute".to_string()
        };

        let action = if self.history_selection_mode {
            "↑/↓ navigate, Enter to replay, Esc to exit"
        } else if self.edit_mode {
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

    /// Phase 5: Auto-detection - Check if conversation needs compaction
    /// Detects off-track patterns (groveling/apologizing) and circular conversations
    fn check_compaction_needed(&mut self, last_response: &str) {
        let lower = last_response.to_lowercase();

        // Detect off-track patterns
        let (detected, pattern, confidence) =
            if lower.contains("absolutely right") || lower.contains("you are absolutely right") {
                (true, "groveling", 0.9)
            } else if lower.contains("i apologize")
                || lower.contains("i'm sorry")
                || lower.contains("sorry for")
            {
                (true, "apologizing", 0.8)
            } else if lower.contains("let me be clear") || lower.contains("to be clear") {
                (true, "condescending", 0.7)
            } else {
                (false, "", 0.0)
            };

        if detected {
            let message = match pattern {
                "groveling" => {
                    "Agent appears to be groveling/agreeing excessively (lost original goal)"
                }
                "apologizing" => "Agent is using defensive/apologetic language",
                "condescending" => "Agent is using condescending language",
                _ => "Unknown pattern detected",
            };

            self.compaction_warning = Some(format!(
                "⚠️ Agent Issue Detected (confidence: {:.0}%)\nReason: {}\n\nPress Ctrl+C to compact conversation",
                confidence * 100.0,
                message
            ));
            return;
        }

        // Detect circular conversation (repeated short responses)
        let history = self.get_history();
        let assistant_msgs: Vec<_> = history.iter().filter(|m| m.role == "assistant").collect();

        if assistant_msgs.len() >= 2 {
            let last_two: Vec<_> = assistant_msgs.iter().rev().take(2).collect();
            if last_two.len() == 2
                && last_two[0].content.len() < 20
                && last_two[1].content.len() < 20
            {
                self.compaction_warning = Some(
                    "⚠️ Circular Conversation Detected (confidence: 70%)\n\
                     Reason: Repeated short responses\n\n\
                     Press Ctrl+C to compact conversation"
                        .to_string(),
                );
            }
        }
    }

    /// Helper: Extract string value from FMPL Map
    #[allow(dead_code)]
    fn get_map_string(&self, map: &Arc<HashMap<SmolStr, Value>>, key: &str) -> Option<String> {
        map.get(&SmolStr::new(key)).and_then(|v| match v {
            Value::String(s) => Some(s.to_string()),
            _ => None,
        })
    }

    /// Helper: Extract bool value from FMPL Map
    #[allow(dead_code)]
    fn get_map_bool(&self, map: &Arc<HashMap<SmolStr, Value>>, key: &str) -> Option<bool> {
        map.get(&SmolStr::new(key)).and_then(|v| match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        })
    }

    /// Helper: Extract float value from FMPL Map
    #[allow(dead_code)]
    fn get_map_float(&self, map: &Arc<HashMap<SmolStr, Value>>, key: &str) -> Option<f64> {
        map.get(&SmolStr::new(key)).and_then(|v| match v {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        })
    }

    /// Helper: Extract int value from FMPL Map
    #[allow(dead_code)]
    fn get_map_int(&self, map: &Arc<HashMap<SmolStr, Value>>, key: &str) -> Option<i64> {
        map.get(&SmolStr::new(key)).and_then(|v| match v {
            Value::Int(i) => Some(*i),
            _ => None,
        })
    }

    /// Perform actual context compaction on the conversation history
    /// Creates a new branch with compacted history, preserving original for undo
    fn perform_compaction(&mut self) -> String {
        let history = self.get_history();
        let keep_recent = 5;
        let max_chars_per_message = 200;

        // Calculate original stats
        let original_count = history.len();
        let original_tokens: usize = history.iter().map(|m| m.content.len() / 4 + 1).sum();

        if original_count <= keep_recent {
            self.compaction_warning = None;
            return format!(
                "Conversation too short to compact ({} messages, keeping last {}).",
                original_count, keep_recent
            );
        }

        // Save current head as compare branch for potential undo
        self.compare_branch_id = Some(self.current_head);
        let _old_nodes = self.conversation_nodes.clone();

        // Clear current conversation
        self.conversation_nodes.clear();
        self.current_head = 0;
        self.node_counter = 0;

        let mut messages_summarized = 0;
        let split_point = original_count.saturating_sub(keep_recent);

        // Process messages
        for (i, msg) in history.iter().enumerate() {
            let is_old = i < split_point;
            let (content, compacted) = if is_old && msg.content.len() > max_chars_per_message {
                // Truncate old long messages
                let truncated = format!(
                    "{}... [truncated]",
                    &msg.content[..max_chars_per_message.min(msg.content.len())]
                );
                messages_summarized += 1;
                (truncated, true)
            } else {
                (msg.content.clone(), false)
            };

            let mut node = ConversationNode::new(
                self.node_counter,
                if self.node_counter == 0 {
                    None
                } else {
                    Some(self.node_counter - 1)
                },
                ChatMessage {
                    role: msg.role.clone(),
                    content,
                },
            );

            if compacted {
                node.metadata.compacted = true;
            }

            self.current_head = self.node_counter;
            self.conversation_nodes.insert(self.node_counter, node);
            self.node_counter += 1;
        }

        // Calculate compacted stats
        let compacted_history = self.get_history();
        let compacted_tokens: usize = compacted_history
            .iter()
            .map(|m| m.content.len() / 4 + 1)
            .sum();
        let savings_percent = if original_tokens > 0 {
            ((original_tokens - compacted_tokens) * 100) / original_tokens
        } else {
            0
        };

        self.compaction_warning = None;

        format!(
            "✅ Context compacted successfully!\n\n\
             Original: ~{} tokens ({} messages)\n\
             Compacted: ~{} tokens\n\
             Savings: {}%\n\
             Messages summarized: {}\n\n\
             The compacted conversation is now active.\n\
             Your original conversation is preserved - use Ctrl+H to access history.",
            original_tokens, original_count, compacted_tokens, savings_percent, messages_summarized
        )
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
                        // Phase 9 Task 9.1: Check for tool requests in LLM response
                        let tool_requests = parse_tool_request(&response);

                        if let Some(requests) = tool_requests {
                            // Phase 9 Task 9.4: Execute tools and display results
                            let mut output_parts = Vec::new();
                            output_parts.push(format!(
                                ">>> LLM ({})\n{}\n\nResponse:\n{}",
                                provider_name, prompt, response
                            ));
                            output_parts.push("\n--- Executing Tools ---\n".to_string());

                            let mut success_count = 0;
                            let mut total_count = 0;

                            for request in &requests {
                                total_count += 1;

                                // Validate request
                                if let Err(e) = validate_tool_request(request, &self.tools) {
                                    output_parts.push(format!(
                                        "[Tool {}] Validation failed: {}\n",
                                        request.tool_id, e
                                    ));
                                    continue;
                                }

                                // Execute tool
                                let tool_result = execute_tool(request, &mut self.tools);

                                // Format result
                                let formatted = format_tool_result(
                                    &request.tool_id,
                                    &request.args,
                                    &tool_result,
                                );
                                output_parts.push(formatted);
                                output_parts.push("\n".to_string());

                                if tool_result.success {
                                    success_count += 1;
                                }
                            }

                            // Add completion summary
                            output_parts.push(format!(
                                "--- Tool Execution Complete: {}/{} succeeded ---\n",
                                success_count, total_count
                            ));

                            // Add assistant response to DAG
                            self.add_message(ChatMessage {
                                role: "assistant".to_string(),
                                content: response.to_string(),
                            });

                            self.output = output_parts.join("\n");
                        } else {
                            // No tool requests, proceed normally
                            self.add_message(ChatMessage {
                                role: "assistant".to_string(),
                                content: response.to_string(),
                            });

                            self.output = format!(
                                ">>> LLM ({})\n{}\n\nResponse:\n{}",
                                provider_name, prompt, response
                            );

                            // Phase 5: Auto-detection - Check for off-track/circular patterns
                            self.check_compaction_needed(&response);
                        }
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
        if self.history_selection_mode {
            text.push_str("\n(Selection mode: ↑/↓ to move, Enter to replay, Esc to exit)");
        }
        text.push('\n');

        for (i, item) in history.iter().enumerate() {
            let (node_id, msg, edited, branch_name) = item;
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
            let selected_marker =
                if self.history_selection_mode && self.selected_node_id == Some(*node_id) {
                    "► "
                } else {
                    "   "
                };

            text.push_str(&format!(
                "\n{}[{}] {}{}{}\n",
                selected_marker,
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

    fn format_diff_view(&self) -> String {
        let compare_node_id = match self.compare_branch_id {
            Some(id) => id,
            None => return "No comparison branch selected.\n\nUse Ctrl+H to select a node, then Enter to replay.\nAfter replay, use Ctrl+D to view diff.".to_string(),
        };

        // Get history from both branches
        let current_history = self.get_history();
        let compare_history = self.get_history_from_node(compare_node_id);

        if current_history.is_empty() && compare_history.is_empty() {
            return "Both branches are empty.\n".to_string();
        }

        let mut text = String::from("Branch Diff View:\n");
        text.push_str(&"=".repeat(50));
        text.push_str(&format!(
            "\nCurrent Branch (Node {}) vs Comparison Branch (Node {})\n",
            self.current_head, compare_node_id
        ));
        text.push_str(&"=".repeat(50));
        text.push('\n');

        // Find common ancestor and display divergent messages
        let max_len = current_history.len().max(compare_history.len());

        for i in 0..max_len {
            let current_msg = current_history.get(i);
            let compare_msg = compare_history.get(i);

            match (current_msg, compare_msg) {
                (Some(curr), Some(comp)) => {
                    // Both branches have message at this position
                    if curr.content == comp.content && curr.role == comp.role {
                        // Same message - show as unchanged
                        let role_label = if curr.role == "user" {
                            "👤 User"
                        } else {
                            "🤖 Assistant"
                        };
                        text.push_str(&format!("\n  [{}] {} (unchanged)\n", i + 1, role_label));
                        text.push_str(&format!(
                            "  {}\n",
                            curr.content.lines().next().unwrap_or("")
                        ));
                    } else {
                        // Different message - show as modified
                        let role_label = if curr.role == "user" {
                            "👤 User"
                        } else {
                            "🤖 Assistant"
                        };
                        text.push_str(&format!("\n🔄 [{}] {} (MODIFIED)\n", i + 1, role_label));
                        text.push_str("  ── Comparison branch:\n");
                        text.push_str(&format!(
                            "  {}\n",
                            comp.content.lines().next().unwrap_or("")
                        ));
                        text.push_str("  ── Current branch:\n");
                        text.push_str(&format!(
                            "  {}\n",
                            curr.content.lines().next().unwrap_or("")
                        ));
                    }
                }
                (Some(curr), None) => {
                    // Only current branch has message - added
                    let role_label = if curr.role == "user" {
                        "👤 User"
                    } else {
                        "🤖 Assistant"
                    };
                    text.push_str(&format!(
                        "\n➕ [{}] {} (ADDED in current)\n",
                        i + 1,
                        role_label
                    ));
                    text.push_str(&format!(
                        "  {}\n",
                        curr.content.lines().next().unwrap_or("")
                    ));
                }
                (None, Some(comp)) => {
                    // Only compare branch has message - removed
                    let role_label = if comp.role == "user" {
                        "👤 User"
                    } else {
                        "🤖 Assistant"
                    };
                    text.push_str(&format!(
                        "\n➖ [{}] {} (REMOVED from current)\n",
                        i + 1,
                        role_label
                    ));
                    text.push_str(&format!(
                        "  {}\n",
                        comp.content.lines().next().unwrap_or("")
                    ));
                }
                (None, None) => unreachable!(),
            }
            text.push_str(&"-".repeat(40));
            text.push('\n');
        }

        text.push_str("\nPress Ctrl+D to exit diff view.\n");
        text
    }

    fn get_history_from_node(&self, node_id: NodeId) -> Vec<ChatMessage> {
        let mut history = Vec::new();
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            if let Some(node) = self.conversation_nodes.get(&id) {
                history.push(node.message.clone());
                current_id = node.parent_id;
            } else {
                break;
            }
        }

        history.reverse();
        history
    }

    fn get_code(&self) -> String {
        self.code_lines.join("\n")
    }

    // Phase 8 Task 8.1: LLM Integration Helper Functions

    /// Format current conversation context as a readable string for LLM prompting
    fn format_conversation_for_llm(&self) -> String {
        let history = self.get_history();
        if history.is_empty() {
            return "No conversation history yet.".to_string();
        }

        let mut text = String::new();
        for msg in &history {
            let role = if msg.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            text.push_str(&format!("{}: {}\n", role, msg.content));
        }
        text
    }

    /// Generate research summary from conversation context
    fn generate_research_summary(&mut self) -> Result<String, String> {
        let conversation_context = self.format_conversation_for_llm();

        if conversation_context == "No conversation history yet." {
            return Err("No conversation to analyze".to_string());
        }

        let prompt = format!(
            "Analyze this conversation and extract the key points, insights, and important information for research notes. Format as clear, organized bullet points:\n\n{}",
            conversation_context
        );

        // Set generation status
        self.llm_generation_status = Some("Generating research summary...".to_string());

        // Call LLM using existing infrastructure
        let provider_name = match self.llm_provider {
            LlmProvider::Ollama => "ollama",
            LlmProvider::Anthropic => "anthropic",
        };

        // Create a single-turn request (not adding to conversation DAG)
        let fmpl_code = format!("{}.chat({:?})", provider_name, &prompt);

        match eval(&mut self.vm, &fmpl_code) {
            Ok(result) => match wait_for_async(result) {
                Ok(Value::String(summary)) => {
                    self.llm_generation_status = None;
                    Ok(summary.to_string())
                }
                Ok(other) => {
                    self.llm_generation_status = None;
                    Err(format!("Unexpected response type: {:?}", other))
                }
                Err(e) => {
                    self.llm_generation_status = None;
                    Err(format!("LLM error: {}", e))
                }
            },
            Err(e) => {
                self.llm_generation_status = None;
                Err(format!("Evaluation error: {}", e))
            }
        }
    }

    /// Generate planning tasks from conversation context
    fn generate_planning_tasks(&mut self) -> Result<Vec<String>, String> {
        let conversation_context = self.format_conversation_for_llm();

        if conversation_context == "No conversation history yet." {
            return Err("No conversation to analyze".to_string());
        }

        let prompt = format!(
            "Based on this conversation, generate a list of actionable tasks. Return one task per line, starting with '- '. Be specific and concise:\n\n{}",
            conversation_context
        );

        // Set generation status
        self.llm_generation_status = Some("Generating planning tasks...".to_string());

        // Call LLM using existing infrastructure
        let provider_name = match self.llm_provider {
            LlmProvider::Ollama => "ollama",
            LlmProvider::Anthropic => "anthropic",
        };

        // Create a single-turn request
        let fmpl_code = format!("{}.chat({:?})", provider_name, &prompt);

        match eval(&mut self.vm, &fmpl_code) {
            Ok(result) => {
                match wait_for_async(result) {
                    Ok(Value::String(response)) => {
                        self.llm_generation_status = None;
                        // Parse response into task descriptions (lines starting with '-')
                        let tasks: Vec<String> = response
                            .lines()
                            .filter(|line| line.trim().starts_with('-'))
                            .map(|line| line.trim().trim_start_matches('-').trim().to_string())
                            .filter(|line| !line.is_empty())
                            .collect();

                        if tasks.is_empty() {
                            Err("No tasks generated".to_string())
                        } else {
                            Ok(tasks)
                        }
                    }
                    Ok(other) => {
                        self.llm_generation_status = None;
                        Err(format!("Unexpected response type: {:?}", other))
                    }
                    Err(e) => {
                        self.llm_generation_status = None;
                        Err(format!("LLM error: {}", e))
                    }
                }
            }
            Err(e) => {
                self.llm_generation_status = None;
                Err(format!("Evaluation error: {}", e))
            }
        }
    }
}

// Phase 9 Task 9.1: Tool Execution Request Parsing

/// Parse a tool request from LLM response text.
///
/// Supports formats:
/// - Simple: TOOL:grep:pattern:src/
/// - JSON: TOOL:{"tool": "grep", "args": {"pattern": "..."}}
///
/// Returns None if no tool request found.
fn parse_tool_request(text: &str) -> Option<Vec<ToolRequest>> {
    let mut requests = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Check for TOOL: prefix
        if !trimmed.starts_with("TOOL:") {
            continue;
        }

        let request_str = &trimmed[5..]; // Skip "TOOL:" prefix

        // Try JSON format first
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(request_str)
            && let Some(obj) = json_val.as_object()
            && let Some(tool_id) = obj.get("tool").and_then(|v| v.as_str())
        {
            let args = if let Some(args_obj) = obj.get("args").and_then(|v| v.as_object()) {
                // Convert JSON object args to string pairs
                let mut arg_vec = Vec::new();
                for (key, value) in args_obj {
                    if let Some(str_val) = value.as_str() {
                        arg_vec.push(format!("{}:{}", key, str_val));
                    }
                }
                arg_vec
            } else if let Some(args_array) = obj.get("args").and_then(|v| v.as_array()) {
                // Convert JSON array args
                args_array
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            };

            requests.push(ToolRequest {
                tool_id: tool_id.to_string(),
                args,
            });
            continue;
        }

        // Try simple format: TOOL:tool_id:arg1:arg2:...
        let parts: Vec<&str> = request_str.split(':').collect();
        if !parts.is_empty() {
            let tool_id = parts[0].to_string();
            let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

            requests.push(ToolRequest { tool_id, args });
        }
    }

    if requests.is_empty() {
        None
    } else {
        Some(requests)
    }
}

/// Validate a tool request against available tools
/// Returns Ok(()) if valid, Err(message) if invalid
fn validate_tool_request(request: &ToolRequest, tools: &[Tool]) -> Result<(), String> {
    // Find tool by ID
    let tool = tools
        .iter()
        .find(|t| t.id == request.tool_id)
        .ok_or_else(|| format!("Tool '{}' not found", request.tool_id))?;

    // Check if tool is enabled
    if !tool.enabled {
        return Err(format!("Tool '{}' is disabled", tool.name));
    }

    // Validate arguments based on tool type
    match request.tool_id.as_str() {
        "grep" => {
            if request.args.len() < 2 {
                return Err("grep requires pattern and path".to_string());
            }
        }
        "file_read" => {
            if request.args.is_empty() {
                return Err("file_read requires a file path".to_string());
            }
        }
        "bash_execute" => {
            if request.args.is_empty() {
                return Err("bash_execute requires a command".to_string());
            }
        }
        "llm_query" => {
            // llm_query can have empty args (uses conversation context)
        }
        _ => {
            // Unknown tool ID - this is OK, might be a custom tool
        }
    }

    Ok(())
}

/// Execute a tool synchronously (Phase 9 Task 9.2)
/// Handles different tool types: grep, file_read, bash_execute, llm_query
/// Returns ToolResult with success status, output, error, and duration
fn execute_tool(request: &ToolRequest, tools: &mut [Tool]) -> ToolResult {
    let start = std::time::Instant::now();

    // Find tool
    let tool_idx = tools.iter().position(|t| t.id == request.tool_id);
    let tool = match tool_idx {
        Some(idx) => &mut tools[idx],
        None => {
            return ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Tool '{}' not found", request.tool_id)),
                duration_ms: 0,
            };
        }
    };

    // Check if enabled
    if !tool.enabled {
        return ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("Tool '{}' is disabled", tool.name)),
            duration_ms: 0,
        };
    }

    // Execute based on tool type
    let result = match request.tool_id.as_str() {
        "grep" => {
            // grep expects: ["pattern", "path"]
            if request.args.len() < 2 {
                ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("grep requires pattern and path".to_string()),
                    duration_ms: 0,
                }
            } else {
                let pattern = &request.args[0];
                let path = &request.args[1];
                match std::process::Command::new("grep")
                    .arg("-r")
                    .arg(pattern)
                    .arg(path)
                    .output()
                {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if output.status.success() {
                            ToolResult {
                                success: true,
                                output: stdout,
                                error: None,
                                duration_ms: start.elapsed().as_millis() as u64,
                            }
                        } else {
                            ToolResult {
                                success: false,
                                output: stdout,
                                error: Some(stderr),
                                duration_ms: start.elapsed().as_millis() as u64,
                            }
                        }
                    }
                    Err(e) => ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to execute grep: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                    },
                }
            }
        }
        "file_read" => {
            // file_read expects: ["path"]
            if request.args.is_empty() {
                ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("file_read requires a file path".to_string()),
                    duration_ms: 0,
                }
            } else {
                let path = &request.args[0];
                match std::fs::read_to_string(path) {
                    Ok(content) => ToolResult {
                        success: true,
                        output: content,
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                    },
                    Err(e) => ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to read file: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                    },
                }
            }
        }
        "bash_execute" => {
            // bash_execute expects: ["command"]
            if request.args.is_empty() {
                ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("bash_execute requires a command".to_string()),
                    duration_ms: 0,
                }
            } else {
                let command = &request.args.join(" ");
                match std::process::Command::new("bash")
                    .arg("-c")
                    .arg(command)
                    .output()
                {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if output.status.success() {
                            ToolResult {
                                success: true,
                                output: stdout,
                                error: None,
                                duration_ms: start.elapsed().as_millis() as u64,
                            }
                        } else {
                            ToolResult {
                                success: false,
                                output: stdout,
                                error: Some(stderr),
                                duration_ms: start.elapsed().as_millis() as u64,
                            }
                        }
                    }
                    Err(e) => ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to execute bash: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                    },
                }
            }
        }
        "llm_query" => {
            // llm_query is recursive - defer to future phase
            ToolResult {
                success: false,
                output: String::new(),
                error: Some("llm_query not yet implemented".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
        _ => {
            // Unknown tool type
            ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unknown tool type: {}", request.tool_id)),
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
    };

    // Increment usage count on successful execution
    if result.success {
        tool.usage_count += 1;
    }

    result
}

/// Format tool result for display (Phase 9 Task 9.3)
/// Returns formatted string with tool name, arguments, and output/error
fn format_tool_result(tool_id: &str, args: &[String], result: &ToolResult) -> String {
    let args_str = args.join(" ");
    if result.success {
        format!(
            "Tool: {} {}\n{}\n[Completed in {}ms]",
            tool_id, args_str, result.output, result.duration_ms
        )
    } else {
        let error_msg: String = result
            .error
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "Unknown error".to_string());
        if !result.output.is_empty() {
            format!(
                "Tool: {} {} [ERROR]\nOutput:\n{}\nError: {}\n[Failed in {}ms]",
                tool_id, args_str, result.output, error_msg, result.duration_ms
            )
        } else {
            format!(
                "Tool: {} {} [ERROR]\nError: {}\n[Failed in {}ms]",
                tool_id, args_str, error_msg, result.duration_ms
            )
        }
    }
}

// Phase 6: Helper function to get panel title with focus indicator
fn get_panel_title(base_title: &str, is_focused: bool) -> String {
    if is_focused {
        format!("{} [FOCUSED]", base_title)
    } else {
        base_title.to_string()
    }
}

// Phase 6 Task 6.4: Get panel-specific help text
fn get_panel_help(panel: PanelType, app: &App) -> String {
    match panel {
        PanelType::Research => {
            if app.llm_mode {
                if app.diff_view_mode {
                    "Ctrl+D: exit diff | Ctrl+H: history | Esc: exit selection".to_string()
                } else {
                    "Ctrl+H: select history | Ctrl+E: edit msg | Ctrl+D: diff view | Ctrl+Z: undo"
                        .to_string()
                }
            } else {
                "Type to edit | Ctrl+S: save | Ctrl+G: generate summary | Arrows: navigate"
                    .to_string()
            }
        }
        PanelType::Planning => {
            // Phase 8: Add Ctrl+G help for generating tasks
            "a:add d:del e:edit Enter:toggle +/−:priority Ctrl+S:save Ctrl+G:generate".to_string()
        }
        PanelType::CodeEditor => {
            if app.llm_mode {
                "Type: input | Enter: send to LLM | Ctrl+L: exit chat".to_string()
            } else if app.execute_mode {
                "Enter: run code | Esc: cancel".to_string()
            } else {
                "Type: edit code | Esc+Enter: run | Ctrl+L: LLM chat".to_string()
            }
        }
        PanelType::Output => "Scroll: ↑↓ | Ctrl+C: copy (planned)".to_string(),
        PanelType::Tools => "Enter: toggle | u: reset stats".to_string(),
    }
}

// ============================================================================
// Command Stream Rendering
// ============================================================================

impl App {
    /// Format command state as a single character indicator
    fn format_command_state(state: CommandState) -> &'static str {
        match state {
            CommandState::Pending => "○",  // Awaiting decision
            CommandState::Approved => "✓", // Approved
            CommandState::Denied => "✗",   // Denied
            CommandState::Executed => "✔", // Executed successfully
            CommandState::Failed => "✖",   // Failed
        }
    }

    /// Get color for command state
    fn command_state_color(state: CommandState) -> Color {
        match state {
            CommandState::Pending => Color::Yellow,
            CommandState::Approved => Color::Green,
            CommandState::Denied => Color::Red,
            CommandState::Executed => Color::Cyan,
            CommandState::Failed => Color::Magenta,
        }
    }

    /// Format the command stream as text lines for rendering
    fn format_command_stream(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        // Header
        lines.push(Line::from(vec![
            Span::styled("ID", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("State", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("Description", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("Tool", Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::from(""));

        if self.arrived_commands.is_empty() {
            lines.push(Line::from(
                "No commands yet. Generate commands from planning panel.",
            ));
            return lines;
        }

        // Render each command
        for (idx, cmd) in self.arrived_commands.iter().enumerate() {
            let is_selected = idx == self.selected_command_index;
            let is_expanded = self.expanded_command == Some(cmd.id);

            // State indicator with color
            let state_str = Self::format_command_state(cmd.state);
            let state_color = Self::command_state_color(cmd.state);

            // Selection indicator
            let cursor = if is_selected { "►" } else { " " };

            // Build line
            lines.push(Line::from(vec![
                Span::styled(
                    cursor,
                    Style::default().fg(if is_selected {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(format!("{:2} ", cmd.id)),
                Span::styled(state_str, Style::default().fg(state_color)),
                Span::raw("  "),
                Span::raw(&cmd.description),
                Span::raw(" "),
                Span::styled(&cmd.tool_call.tool_name, Style::default().fg(Color::Cyan)),
            ]));

            // Show expanded details if selected
            if is_expanded {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("  Tool: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&cmd.tool_call.tool_name, Style::default().fg(Color::Cyan)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("  Args: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("{:?}", cmd.tool_call.args)),
                ]));

                // Show grammar checks if any
                if !cmd.grammar_checks.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        "  Grammar Checks:",
                        Style::default().fg(Color::DarkGray),
                    )]));
                    for check in &cmd.grammar_checks {
                        let check_color = if check.matched {
                            Color::Green
                        } else {
                            Color::Red
                        };
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(&check.rule_name, Style::default().fg(check_color)),
                            Span::raw(": "),
                            Span::raw(&check.message),
                        ]));
                    }
                }

                // Show execution result if available
                if let Some(ref result) = cmd.execution_result {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  Result: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(result),
                    ]));
                }

                lines.push(Line::from(""));
            }
        }

        // Show stream status at bottom
        if self.stream_complete {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Stream complete. ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("Branch: {}", self.current_branch),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else if self.command_stream.is_some() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Receiving commands...",
                Style::default().fg(Color::Yellow),
            )]));
        }

        lines
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

    // Research panel - show conversation history in LLM mode, editable notes otherwise
    let research_focused = app.focused_panel == PanelType::Research;
    let research_help = get_panel_help(PanelType::Research, app);

    let research_content = if app.llm_mode {
        let base_content = if app.diff_view_mode {
            app.format_diff_view()
        } else {
            app.format_history()
        };
        // Phase 6 Task 6.4: Append help text when focused
        if research_focused && !research_help.is_empty() {
            format!("{}\n\n─\n{}\n─", base_content, research_help)
        } else {
            base_content
        }
    } else {
        // Phase 6 Task 6.2: Show editable research lines with cursor indicator
        let base_content = app.research_lines.join("\n");
        // Phase 6 Task 6.4: Append help text when focused
        if research_focused && !research_help.is_empty() && !app.research_lines.is_empty() {
            format!("{}\n\n─\n{}\n─", base_content, research_help)
        } else {
            base_content
        }
    };

    let base_panel_title = if app.llm_mode {
        if app.diff_view_mode {
            "Branch Diff View"
        } else {
            "Conversation History"
        }
    } else {
        "Research View"
    };

    let research_title = get_panel_title(base_panel_title, research_focused);

    let research_panel = if app.llm_mode {
        // In LLM mode, just show the conversation history
        Paragraph::new(research_content.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(research_title)
                    .title_alignment(Alignment::Center),
            )
            .wrap(Wrap { trim: true })
    } else {
        // Phase 6 Task 6.2: In non-LLM mode, show research with cursor when focused
        if research_focused && !app.research_lines.is_empty() {
            let cursor_row = app.research_cursor_row.min(app.research_lines.len() - 1);
            let cursor_col = app
                .research_cursor_col
                .min(app.research_lines[cursor_row].len());

            // Build Text with cursor indicator
            let mut spans = Vec::new();
            for (i, line) in app.research_lines.iter().enumerate() {
                if i == cursor_row {
                    // Show cursor position
                    let before = &line[..cursor_col];
                    let after = &line[cursor_col..];
                    spans.push(Line::from(vec![
                        Span::raw(before.to_string()),
                        Span::styled("█", Style::default().fg(Color::Yellow)),
                        Span::raw(after.to_string()),
                    ]));
                } else {
                    spans.push(Line::from(line.as_str()));
                }
            }
            Paragraph::new(spans)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(research_title)
                        .title_alignment(Alignment::Center),
                )
                .wrap(Wrap { trim: false })
        } else {
            // Not focused or empty, show plain text
            Paragraph::new(research_content.as_str())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(research_title)
                        .title_alignment(Alignment::Center),
                )
                .wrap(Wrap { trim: true })
        }
    };

    // Planning panel (or Tools panel when focused)
    let planning_focused = app.focused_panel == PanelType::Planning;
    let tools_focused = app.focused_panel == PanelType::Tools;
    let panel_title = if tools_focused {
        get_panel_title("Tools View", true)
    } else {
        get_panel_title("Planning View", planning_focused)
    };

    // Phase 7 Task 7.2: Render tools list when tools panel is focused, otherwise render planning tasks
    let middle_panel_content = if tools_focused {
        // Show tools list
        if app.tools.is_empty() {
            vec![
                Line::from("No tools configured."),
                Line::from(""),
                Line::from("Press 'a' to add a tool."),
            ]
        } else {
            let mut lines = Vec::new();

            // Header
            lines.push(Line::from(vec![
                Span::styled("ID  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Name           ", Style::default().fg(Color::DarkGray)),
                Span::styled("Enabled  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Timeout  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Confirm  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Use", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(""));

            // Render each tool
            for (idx, tool) in app.tools.iter().enumerate() {
                let is_selected = idx == app.selected_tool_index;

                let enabled_str = if tool.enabled { "✓" } else { "✗" };
                let enabled_color = if tool.enabled {
                    Color::Green
                } else {
                    Color::Red
                };

                let confirm_str = if tool.requires_confirmation {
                    "✓"
                } else {
                    "✗"
                };
                let confirm_color = if tool.requires_confirmation {
                    Color::Yellow
                } else {
                    Color::DarkGray
                };

                if is_selected {
                    // Selected tool gets highlighted
                    lines.push(Line::from(vec![
                        Span::styled("► ", Style::default().fg(Color::Yellow)),
                        Span::styled(&tool.name, Style::default().fg(Color::Yellow)),
                        Span::raw(" ".repeat(14 - tool.name.len().min(14))),
                        Span::styled(enabled_str, Style::default().fg(Color::Yellow)),
                        Span::raw(" ".repeat(9)),
                        Span::styled(
                            format!("{}s", tool.timeout_ms / 1000),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::raw(" ".repeat(9)),
                        Span::styled(confirm_str, Style::default().fg(Color::Yellow)),
                        Span::raw(" ".repeat(9)),
                        Span::styled(
                            format!("{}", tool.usage_count),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::raw(&tool.name),
                        Span::raw(" ".repeat(14 - tool.name.len().min(14))),
                        Span::styled(enabled_str, Style::default().fg(enabled_color)),
                        Span::raw(" ".repeat(9)),
                        Span::raw(format!("{}s", tool.timeout_ms / 1000)),
                        Span::raw(" ".repeat(9)),
                        Span::styled(confirm_str, Style::default().fg(confirm_color)),
                        Span::raw(" ".repeat(9)),
                        Span::raw(format!("{}", tool.usage_count)),
                    ]));
                }
            }

            // Add help text
            lines.push(Line::from(""));
            lines.push(Line::from("Enter: toggle | e: edit | a: add | d: delete"));

            lines
        }
    } else {
        // Show planning tasks
        if app.planning_tasks.is_empty() {
            if planning_focused {
                vec![
                    Line::from("No tasks yet."),
                    Line::from(""),
                    Line::from("Press 'a' to add a task."),
                ]
            } else {
                vec![Line::from("Planning view - Collaborative scope definition")]
            }
        } else {
            let mut lines = Vec::new();

            // Add help text when focused
            if planning_focused {
                lines.push(Line::from("a:add e:edit Enter:toggle d:del +/-:priority"));
                lines.push(Line::from(""));
            }

            // Render each task
            for (idx, task) in app.planning_tasks.iter().enumerate() {
                let is_selected = planning_focused && idx == app.selected_task_index;

                // Status indicator
                let status_marker = match task.status {
                    TaskStatus::Pending => "[ ]",
                    TaskStatus::InProgress => "[>]",
                    TaskStatus::Complete => "[x]",
                };

                // Priority color
                let priority_color = match task.priority {
                    Priority::Low => Color::Blue,
                    Priority::Medium => Color::Yellow,
                    Priority::High => Color::Red,
                };

                let priority_tag = match task.priority {
                    Priority::Low => "[L]",
                    Priority::Medium => "[M]",
                    Priority::High => "[H]",
                };

                // Build line with task info
                if is_selected {
                    // Selected task gets highlighted
                    lines.push(Line::from(vec![
                        Span::styled("► ", Style::default().fg(Color::Yellow)),
                        Span::styled(status_marker, Style::default().fg(Color::Yellow)),
                        Span::raw(" "),
                        Span::styled(&task.description, Style::default().fg(Color::Yellow)),
                        Span::raw(" "),
                        Span::styled(priority_tag, Style::default().fg(priority_color)),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::raw(status_marker),
                        Span::raw(" "),
                        Span::raw(&task.description),
                        Span::raw(" "),
                        Span::styled(priority_tag, Style::default().fg(priority_color)),
                    ]));
                }
            }

            lines
        }
    };

    let planning_panel = Paragraph::new(Text::from(middle_panel_content))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(panel_title)
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: false }); // Don't wrap to preserve formatting

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

    // Phase 5: Display compaction warning if detected
    let warning_text = if let Some(ref warning) = app.compaction_warning {
        format!("\n\n{}", warning)
    } else {
        String::new()
    };

    let code_focused = app.focused_panel == PanelType::CodeEditor;

    // Render either command stream or code editor
    let (code_text, code_title) = if app.command_stream_mode {
        // Command stream mode
        let mut command_lines = app.format_command_stream();

        // Add help text when focused
        if code_focused {
            command_lines.push(Line::from(""));
            command_lines.push(Line::from(vec![
                Span::styled("─ ", Style::default().fg(Color::DarkGray)),
                Span::styled("↑↓:nav Enter:approve Esc:deny Space:expand E:edit I:ins D:del Ctrl+U:rewind Ctrl+T:toggle", Style::default().fg(Color::Cyan)),
                Span::styled(" ─", Style::default().fg(Color::DarkGray)),
            ]));
        }

        let title = format!("Command Stream [Branch: {}]", app.current_branch);
        (
            Text::from(command_lines),
            get_panel_title(&title, code_focused),
        )
    } else {
        // Original code editor mode
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
                let cursor_col = app.cursor_col.min(line.len());
                let before = line[..cursor_col].to_string();
                let cursor_char = if cursor_col < line.len() {
                    line[cursor_col..cursor_col + 1].to_string()
                } else {
                    " ".to_string()
                };
                let after = line[cursor_col..].to_string();

                code_spans.push(Line::from(vec![
                    Span::raw(format!("{:2} ", actual_row + 1)),
                    Span::raw(before),
                    Span::styled(
                        cursor_char,
                        Style::default().fg(Color::Yellow).bg(Color::DarkGray),
                    ),
                    Span::raw(after),
                ]));
            } else {
                code_spans.push(Line::from(vec![
                    Span::raw(format!("{:2} ", actual_row + 1)),
                    Span::raw(line.clone()),
                ]));
            }
        }

        // Phase 6 Task 6.4: Add help text when focused
        if code_focused {
            let code_help = get_panel_help(PanelType::CodeEditor, app);
            if !code_help.is_empty() {
                code_spans.push(Line::from(""));
                code_spans.push(Line::from(vec![
                    Span::styled("─ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(code_help, Style::default().fg(Color::Cyan)),
                    Span::styled(" ─", Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        (
            Text::from(code_spans),
            get_panel_title("Code Editor", code_focused),
        )
    };

    let code_panel = Paragraph::new(code_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(code_title)
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: false }); // Don't wrap - show horizontal scroll

    // Output panel
    let output_focused = app.focused_panel == PanelType::Output;
    let output_help = get_panel_help(PanelType::Output, app);
    let output_content = if output_focused && !output_help.is_empty() {
        format!("{}\n\n─\n{}\n─", app.output, output_help)
    } else {
        format!("{}{}", app.output, warning_text)
    };

    let output_title = get_panel_title("Execution Output", output_focused);

    let output_panel = Paragraph::new(output_content.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(output_title)
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

        // Check for pending human approval requests
        app.check_approval_queue();

        // Handle input
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_input(key);
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
