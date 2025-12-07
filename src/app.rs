use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, prelude::*};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::api::{CodeResultsWithPagination, PaginationInfo};
use crate::history::SearchHistory;
use crate::results::CodeResults;
use crate::widgets::{
    FilterMode, KeyHandleResult, SearchResults, SearchResultsState, TextInput, TextInputState,
};

#[derive(Default, Debug, Clone)]
pub enum SearchState {
    #[default]
    Idle,
    Loading {
        query: String,
    },
    Loaded {
        query: String,
        results: CodeResults,
        pagination: Option<PaginationInfo>,
        current_page: u32,
    },
    LoadingMore {
        query: String,
        results: CodeResults,
        pagination: Option<PaginationInfo>,
        current_page: u32,
    },
}

impl SearchState {
    pub fn is_loading(&self) -> bool {
        matches!(
            self,
            SearchState::Loading { .. } | SearchState::LoadingMore { .. }
        )
    }

    pub fn num_results(&self) -> usize {
        match self {
            Self::Loaded { results, .. } | Self::LoadingMore { results, .. } => results.count(),
            _ => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppMessage {
    SearchComplete {
        results: CodeResultsWithPagination,
        query: String,
    },
    SearchError {
        error: String,
    },
    PaginationComplete {
        results: CodeResultsWithPagination,
        page: u32,
    },
    PaginationError {
        error: String,
    },
    HistoryLoaded {
        searches: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct App {
    pub search_state: SearchState,
    pub search_history: SearchHistory,
    pub input_state: TextInputState,
    pub search_results_state: SearchResultsState,
    pub message_tx: UnboundedSender<AppMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    SearchPrompt,
    SearchResults,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub should_exit: bool,
    pub current_screen: Screen,
    pub frame_counter: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            should_exit: false,
            current_screen: Screen::SearchPrompt,
            frame_counter: 0,
        }
    }
}

impl App {
    fn new(message_tx: UnboundedSender<AppMessage>) -> Self {
        Self {
            search_state: SearchState::default(),
            search_history: SearchHistory::default(),
            input_state: TextInputState::default(),
            search_results_state: SearchResultsState::default(),
            message_tx,
        }
    }

    pub async fn run(mut terminal: DefaultTerminal) -> eyre::Result<()> {
        let (message_tx, mut message_rx) = mpsc::unbounded_channel();
        let mut app = App::new(message_tx.clone());
        let mut app_state = AppState::default();

        // Load search history on startup
        tokio::spawn(async move {
            match crate::history::load_history().await {
                Ok(history) => {
                    let _ = message_tx.send(AppMessage::HistoryLoaded {
                        searches: history.searches,
                    });
                }
                Err(e) => {
                    eprintln!("Failed to load history: {}", e);
                }
            }
        });

        loop {
            // Render frame
            terminal.draw(|frame| {
                frame.render_stateful_widget(&mut app, frame.area(), &mut app_state)
            })?;

            app_state.frame_counter = app_state.frame_counter.wrapping_add(1);

            if app_state.should_exit {
                break;
            }

            // Use tokio::select! to multiplex event sources
            tokio::select! {
                // Tick for rendering (60 FPS = ~16ms per frame)
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(16)) => {
                    // Check for terminal events without blocking
                    if event::poll(std::time::Duration::ZERO)? {
                        let event = event::read()?;
                        tracing::debug!("Event received: {:?}", event);
                        if let Event::Key(key) = event {
                            app.handle_key(key, &mut app_state);
                        }
                    }
                }
                // Handle messages from background tasks
                Some(msg) = message_rx.recv() => {
                    app.handle_message(msg, &mut app_state);
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent, state: &mut AppState) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match state.current_screen {
            Screen::SearchPrompt => {
                // Check for Ctrl modifier
                let ctrl_pressed = key.modifiers.contains(KeyModifiers::CONTROL);

                match (key.code, ctrl_pressed) {
                    (KeyCode::Esc, _) | (KeyCode::Char('c'), true) => {
                        state.should_exit = true;
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), true) => {
                        self.search_history.select_next();
                        // Update input with selected history item
                        if let Some(query) = self.search_history.get_selected() {
                            self.input_state.input = query.clone();
                            self.input_state.cursor_position = query.len();
                        }
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), true) => {
                        self.search_history.select_prev();
                        // Update input with selected history item
                        if let Some(query) = self.search_history.get_selected() {
                            self.input_state.input = query.clone();
                            self.input_state.cursor_position = query.len();
                        }
                    }
                    (KeyCode::Enter, _) | (KeyCode::Char('l'), true) => {
                        // Spawn async task to fetch search results
                        let query = self.input_state.input.trim().to_string();
                        if !query.is_empty() {
                            let tx = self.message_tx.clone();
                            let query_for_task = query.clone();
                            tokio::spawn(async move {
                                match crate::api::fetch_code_results(&query_for_task, None).await {
                                    Ok(data) => {
                                        let _ = tx.send(AppMessage::SearchComplete {
                                            results: data,
                                            query: query_for_task,
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx.send(AppMessage::SearchError {
                                            error: e.to_string(),
                                        });
                                    }
                                }
                            });

                            // Update state to Loading
                            self.search_state = SearchState::Loading { query };

                            // Clear history selection
                            self.search_history.clear_selection();

                            // Switch to results screen
                            state.current_screen = Screen::SearchResults;
                        }
                    }
                    _ => {
                        // Only clear selection and handle input if no Ctrl modifier
                        if !ctrl_pressed {
                            self.search_history.clear_selection();
                            self.input_state.handle_key(key);
                        }
                    }
                }
            }
            Screen::SearchResults => {
                // Handle Esc specially - check filter mode first
                if key.code == KeyCode::Esc {
                    match self.search_results_state.filter_mode {
                        FilterMode::Inactive => {
                            // No filter active, go back to search prompt
                            state.current_screen = Screen::SearchPrompt;
                            return;
                        }
                        _ => {
                            // Filter is active, let handle_key deal with it
                        }
                    }
                }

                // Need to calculate filtered count
                let needs_pagination = match &self.search_state {
                    SearchState::Loaded { results, .. }
                    | SearchState::LoadingMore { results, .. } => {
                        // Count filtered results
                        let filtered_count = results
                            .items
                            .iter()
                            .flat_map(|item| {
                                item.text_matches.iter().filter(|text_match| {
                                    self.search_results_state
                                        .should_include_match(item, text_match)
                                })
                            })
                            .count();

                        let result =
                            self.search_results_state
                                .handle_key(key, filtered_count, results);
                        matches!(result, KeyHandleResult::NeedsPagination)
                    }
                    _ => false,
                };

                if needs_pagination {
                    self.try_load_next_page();
                }
            }
        }
    }

    fn try_load_next_page(&mut self) {
        // Check if we can load more pages
        if let SearchState::Loaded {
            query,
            pagination: Some(pagination),
            current_page,
            ..
        } = &self.search_state
        {
            // Only load if there's a next page
            if pagination.next.is_some() {
                let query = query.clone();
                let next_page = current_page + 1;
                let tx = self.message_tx.clone();

                // Clone search state data before transitioning
                if let SearchState::Loaded {
                    results,
                    pagination,
                    ..
                } = &self.search_state
                {
                    let current_results = results.clone();
                    let current_pagination = pagination.clone();

                    // Transition to LoadingMore state
                    self.search_state = SearchState::LoadingMore {
                        query: query.clone(),
                        results: current_results,
                        pagination: current_pagination,
                        current_page: *current_page,
                    };

                    // Spawn task to fetch next page
                    tokio::spawn(async move {
                        match crate::api::fetch_code_results(&query, Some(next_page)).await {
                            Ok(data) => {
                                let _ = tx.send(AppMessage::PaginationComplete {
                                    results: data,
                                    page: next_page,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(AppMessage::PaginationError {
                                    error: e.to_string(),
                                });
                            }
                        }
                    });
                }
            }
        }
    }

    fn handle_message(&mut self, msg: AppMessage, _state: &mut AppState) {
        match msg {
            AppMessage::SearchComplete { results, query } => {
                // Transition to Loaded state
                self.search_state = SearchState::Loaded {
                    query: query.clone(),
                    results: results.results,
                    pagination: results.pagination,
                    current_page: 1,
                };

                // Reset filter state for new search
                self.search_results_state.filter_mode = FilterMode::Inactive;
                self.search_results_state.filter_input_state.input.clear();
                self.search_results_state.filter_input_state.cursor_position = 0;

                // Add to search history
                self.search_history.add_search(query.clone());

                // Spawn task to save history (fire-and-forget)
                let history = self.search_history.clone();
                tokio::spawn(async move {
                    let _ = crate::history::save_history(&history).await;
                });
            }
            AppMessage::SearchError { error } => {
                // Let it crash per requirements
                panic!("Search error: {}", error);
            }
            AppMessage::PaginationComplete { results, page } => {
                // Merge results and transition back to Loaded
                if let SearchState::LoadingMore {
                    query,
                    results: current_results,
                    ..
                } = &mut self.search_state
                {
                    // Append new items to existing results
                    let mut merged = current_results.clone();
                    merged.items.extend(results.results.items);

                    self.search_state = SearchState::Loaded {
                        query: query.clone(),
                        results: merged,
                        pagination: results.pagination,
                        current_page: page,
                    };
                }
            }
            AppMessage::PaginationError { error } => {
                // Let it crash per requirements
                panic!("Pagination error: {}", error);
            }
            AppMessage::HistoryLoaded { searches } => {
                self.search_history = crate::history::SearchHistory::new(searches);
            }
        }
    }
}

impl StatefulWidget for &mut App {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        buf.reset();

        match state.current_screen {
            Screen::SearchPrompt => {
                self.render_search_prompt_screen(area, buf);
            }
            Screen::SearchResults => {
                self.render_search_results_screen(area, buf, state);
            }
        }
    }
}

impl App {
    fn render_search_prompt_screen(&mut self, area: Rect, buf: &mut Buffer) {
        let [inner_area] = Layout::horizontal([Constraint::Fill(1)])
            .margin(2)
            .areas(area);

        let [prompt_area, history_area, footer_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(inner_area);

        TextInput { is_focused: true }.render(prompt_area, buf, &mut self.input_state);

        // Render search history
        let history_block = Block::new().borders(Borders::ALL).title("Search History");
        let history_inner = history_block.inner(history_area);
        history_block.render(history_area, buf);

        if self.search_history.searches.is_empty() {
            Paragraph::new("No search history yet")
                .style(Style::default().fg(Color::DarkGray))
                .render(history_inner, buf);
        } else {
            let history_lines: Vec<Line> = self
                .search_history
                .searches
                .iter()
                .enumerate()
                .map(|(idx, search)| {
                    let style = if self.search_history.selected_idx == Some(idx) {
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(search.as_str()).style(style)
                })
                .collect();

            Paragraph::new(history_lines).render(history_inner, buf);
        }

        let footer_lines = vec![Line::from(
            "Enter/Ctrl+L to search, ↓↑ to select history, Esc to quit",
        )];
        Paragraph::new(footer_lines)
            .centered()
            .render(footer_area, buf);
    }

    fn render_search_results_screen(&mut self, area: Rect, buf: &mut Buffer, app_state: &AppState) {
        let [inner_area] = Layout::horizontal([Constraint::Fill(1)])
            .margin(2)
            .areas(area);

        // Adjust footer height based on filter mode
        let footer_height = match self.search_results_state.filter_mode {
            FilterMode::Editing => 5, // Need space for input widget
            _ => 3,                   // Normal height
        };

        let [matches_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(footer_height)])
                .areas(inner_area);

        // Render based on search state
        match &self.search_state {
            SearchState::Idle => {
                Paragraph::new("No search results yet. Press Esc to go back.")
                    .centered()
                    .render(matches_area, buf);
            }
            SearchState::Loading { query } => {
                // Spinner frames: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
                let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame_idx = (app_state.frame_counter / 3) as usize % spinner_frames.len();
                let spinner = spinner_frames[frame_idx];

                Paragraph::new(format!("{} Loading results for: {}", spinner, query))
                    .centered()
                    .render(matches_area, buf);
            }
            SearchState::Loaded { results, .. } | SearchState::LoadingMore { results, .. } => {
                SearchResults {
                    code: results,
                    is_focused: true,
                }
                .render(matches_area, buf, &mut self.search_results_state);
            }
        }

        // Render footer with optional loading indicator and pagination info
        let page_info = match &self.search_state {
            SearchState::Loaded {
                current_page,
                pagination,
                ..
            }
            | SearchState::LoadingMore {
                current_page,
                pagination,
                ..
            } => {
                if let Some(pagination) = pagination {
                    if let Some(last_page) = pagination.get_last_page_number() {
                        format!(" | Page {}/{}", current_page, last_page)
                    } else {
                        format!(" | Page {}", current_page)
                    }
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        let mut footer_lines = vec![Line::from(format!(
            "Use ↓↑/jk to navigate, Enter/l to open result | / to filter{page_info}",
        ))];

        // Handle different filter modes
        match self.search_results_state.filter_mode {
            FilterMode::Editing => {
                // Show editable filter input
                footer_lines.push(Line::from(""));

                // Split footer_area to make room for input widget
                let [help_area, input_area] =
                    Layout::vertical([Constraint::Length(2), Constraint::Length(3)])
                        .areas(footer_area);

                // Render help text
                Paragraph::new(footer_lines)
                    .centered()
                    .render(help_area, buf);

                // Render filter input widget
                TextInput { is_focused: true }.render(
                    input_area,
                    buf,
                    &mut self.search_results_state.filter_input_state,
                );

                return; // Skip normal footer rendering
            }
            FilterMode::Applied => {
                // Show applied filter as read-only
                footer_lines.push(
                    Line::from(format!(
                        "Filter: {} (Esc to clear)",
                        self.search_results_state.filter_input_state.input
                    ))
                    .style(Style::default().fg(Color::Yellow)),
                );
            }
            FilterMode::Inactive => {
                // Show normal help text
                if matches!(self.search_state, SearchState::LoadingMore { .. }) {
                    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let frame_idx = (app_state.frame_counter / 3) as usize % spinner_frames.len();
                    let spinner = spinner_frames[frame_idx];
                    footer_lines.push(Line::from(format!("{} Loading more results...", spinner)));
                } else {
                    footer_lines.push(Line::from("Esc to go back to search"));
                }
            }
        }

        Paragraph::new(footer_lines)
            .centered()
            .render(footer_area, buf);
    }
}
