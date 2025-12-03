use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, prelude::*};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::api::{CodeResultsWithPagination, PaginationInfo};
use crate::results::{CodeResults, ItemResult, TextMatch};
use crate::widgets::{SearchResults, SearchResultsState, TextInput, TextInputState};

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
    pub code: CodeResults,
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
            code: crate::results::mock(),
            input_state: TextInputState::default(),
            search_results_state: SearchResultsState::default(),
            message_tx,
        }
    }

    pub async fn run(mut terminal: DefaultTerminal) -> eyre::Result<()> {
        let (message_tx, mut message_rx) = mpsc::unbounded_channel();
        let mut app = App::new(message_tx);
        let mut app_state = AppState::default();

        while !app_state.should_exit {
            terminal.draw(|frame| {
                frame.render_stateful_widget(&mut app, frame.area(), &mut app_state)
            })?;

            let event = event::read()?;

            #[allow(clippy::single_match)]
            match event {
                Event::Key(key) => app.handle_key(key, &mut app_state),
                _ => {}
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
                    // TODO: Trigger search and switch to results screen
                    state.current_screen = Screen::SearchResults;
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
                    let total_items = self.iter_text_matches().count();
                    self.search_results_state
                        .handle_key(key, total_items, &self.code);
                }
            },
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

        SearchResults {
            code: &self.code,
            is_focused: true,
        }
        .render(matches_area, buf, &mut self.search_results_state);

        let footer_lines = vec![
            Line::from("Use ↓↑/jk to navigate, Enter/l to open the search result in the browser"),
            Line::from("Esc to go back to search"),
        ];
        Paragraph::new(footer_lines)
            .centered()
            .render(footer_area, buf);
    }

    fn iter_text_matches(&self) -> impl Iterator<Item = (&ItemResult, &TextMatch)> {
        self.code.items.iter().flat_map(|item| {
            item.text_matches
                .iter()
                .map(move |text_match| (item, text_match))
        })
    }
}
