use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    pub is_focused: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TextInputState {
    pub input: String,
    pub cursor_position: usize,
}

impl TextInputState {
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.input.insert(self.cursor_position, c);
                self.cursor_position += 1;
                true
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input.remove(self.cursor_position);
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input.len() {
                    self.input.remove(self.cursor_position);
                }
                true
            }
            KeyCode::Left => {
                self.cursor_position = self.cursor_position.saturating_sub(1);
                true
            }
            KeyCode::Right => {
                self.cursor_position = (self.cursor_position + 1).min(self.input.len());
                true
            }
            KeyCode::Home => {
                self.cursor_position = 0;
                true
            }
            KeyCode::End => {
                self.cursor_position = self.input.len();
                true
            }
            _ => false,
        }
    }
}

impl StatefulWidget for TextInput {
    type State = TextInputState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let border_style = if self.is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let block = Block::new()
            .borders(Borders::ALL)
            .title("Search")
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        Paragraph::new(state.input.as_str()).render(inner, buf);
    }
}
