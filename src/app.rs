use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::{DefaultTerminal, prelude::*};

use crate::results::{CodeResults, ItemResult, TextMatch};
use crate::widgets::{SearchResults, SearchResultsState, TextInput, TextInputState};

#[derive(Debug, Clone)]
pub struct App {
    pub code: CodeResults,
    pub input_state: TextInputState,
    pub search_results_state: SearchResultsState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedWidget {
    TextInput,
    SearchResults,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub should_exit: bool,
    pub focused_widget: FocusedWidget,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            should_exit: false,
            focused_widget: FocusedWidget::SearchResults,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            code: crate::results::mock(),
            input_state: TextInputState::default(),
            search_results_state: SearchResultsState::default(),
        }
    }
}

impl App {
    pub async fn run(mut terminal: DefaultTerminal) -> eyre::Result<()> {
        let mut app = App::default();
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
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc
                if !matches!(state.focused_widget, FocusedWidget::TextInput) =>
            {
                state.should_exit = true;
            }
            KeyCode::Tab => {
                state.focused_widget = match state.focused_widget {
                    FocusedWidget::TextInput => FocusedWidget::SearchResults,
                    FocusedWidget::SearchResults => FocusedWidget::TextInput,
                };
            }
            _ => match state.focused_widget {
                FocusedWidget::TextInput => {
                    self.input_state.handle_key(key);
                }
                FocusedWidget::SearchResults => {
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

    fn render(self, area: Rect, buf: &mut Buffer, _state: &mut AppState) {
        buf.reset();

        let [prompt_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(area);

        let [matches_area] = Layout::horizontal([Constraint::Fill(4)])
            .margin(2)
            .areas(main_area);

        TextInput {}.render(prompt_area, buf, &mut self.input_state);
        App::render_footer(footer_area, buf);

        SearchResults { code: &self.code }.render(
            matches_area,
            buf,
            &mut self.search_results_state,
        );
    }
}

impl App {
    fn render_footer(area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from("Use ↓↑/jk to navigate, Enter/l to open the search result in the browser"),
            Line::from("Tab to switch to prompt"),
        ];

        Paragraph::new(lines).centered().render(area, buf);
    }

    fn iter_text_matches(&self) -> impl Iterator<Item = (&ItemResult, &TextMatch)> {
        self.code.items.iter().flat_map(|item| {
            item.text_matches
                .iter()
                .map(move |text_match| (item, text_match))
        })
    }
}
