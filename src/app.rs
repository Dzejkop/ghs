use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, prelude::*};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::api::{CodeResultsWithPagination, PaginationInfo};
use crate::history::SearchHistory;
use crate::results::{CodeResults, ItemResult, TextMatch};
use crate::widgets::{SearchResults, SearchResultsState, TextInput, TextInputState};

#[derive(Debug, Clone)]
pub enum SearchState {
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

impl Default for SearchState {
    fn default() -> Self {
        SearchState::Idle
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            should_exit: false,
            current_screen: Screen::SearchPrompt,
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
                        match event {
                            Event::Key(key) => app.handle_key(key, &mut app_state),
                            _ => {}
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
            Screen::SearchPrompt => match key.code {
                KeyCode::Esc => {
                    state.should_exit = true;
                }
                KeyCode::Enter => {
                    // Spawn async task to fetch search results
                    let query = self.input_state.input.trim().to_string();
                    if !query.is_empty() {
                        let tx = self.message_tx.clone();
                        tokio::spawn(async move {
                            match crate::api::fetch_code_results(&query, None).await {
                                Ok(data) => {
                                    let _ = tx.send(AppMessage::SearchComplete {
                                        results: data,
                                        query,
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
                        self.search_state = SearchState::Loading {
                            query: query.clone(),
                        };

                        // Switch to results screen
                        state.current_screen = Screen::SearchResults;
                    }
                }
                _ => {
                    self.input_state.handle_key(key);
                }
            },
            Screen::SearchResults => match key.code {
                KeyCode::Esc => {
                    state.current_screen = Screen::SearchPrompt;
                }
                _ => {
                    if let Some(results) = self.get_current_results() {
                        let total_items = self.iter_text_matches().count();
                        self.search_results_state
                            .handle_key(key, total_items, results);
                    }
                }
            },
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
                self.render_search_results_screen(area, buf);
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
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(inner_area);

        TextInput { is_focused: true }.render(prompt_area, buf, &mut self.input_state);

        // TODO: Render search history
        Paragraph::new("Search history will go here").render(history_area, buf);

        let footer_lines = vec![Line::from("Enter to search, Esc to quit")];
        Paragraph::new(footer_lines)
            .centered()
            .render(footer_area, buf);
    }

    fn render_search_results_screen(&mut self, area: Rect, buf: &mut Buffer) {
        let [inner_area] = Layout::horizontal([Constraint::Fill(1)])
            .margin(2)
            .areas(area);

        let [matches_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(inner_area);

        // Render based on search state
        match &self.search_state {
            SearchState::Idle => {
                Paragraph::new("No search results yet. Press Esc to go back.")
                    .centered()
                    .render(matches_area, buf);
            }
            SearchState::Loading { query } => {
                Paragraph::new(format!("Loading results for: {}", query))
                    .centered()
                    .render(matches_area, buf);
            }
            SearchState::Loaded { results, .. } | SearchState::LoadingMore { results, .. } => {
                SearchResults {
                    code: results,
                    is_focused: true,
                }
                .render(matches_area, buf, &mut self.search_results_state);

                // Show loading more indicator
                if matches!(self.search_state, SearchState::LoadingMore { .. }) {
                    // This will be shown at the bottom of results
                    // For now, just a simple indicator
                }
            }
        }

        let footer_lines = vec![
            Line::from("Use ↓↑/jk to navigate, Enter/l to open the search result in the browser"),
            Line::from("Esc to go back to search"),
        ];
        Paragraph::new(footer_lines)
            .centered()
            .render(footer_area, buf);
    }

    fn get_current_results(&self) -> Option<&CodeResults> {
        match &self.search_state {
            SearchState::Loaded { results, .. } => Some(results),
            SearchState::LoadingMore { results, .. } => Some(results),
            _ => None,
        }
    }

    fn iter_text_matches(&self) -> impl Iterator<Item = (&ItemResult, &TextMatch)> {
        self.get_current_results()
            .map(|code| {
                code.items
                    .iter()
                    .flat_map(|item| {
                        item.text_matches
                            .iter()
                            .map(move |text_match| (item, text_match))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
    }
}
