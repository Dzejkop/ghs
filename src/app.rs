use std::borrow::Cow;
use std::ops::Range;

use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, prelude::*};

use crate::results::{CodeResults, ItemResult, MatchSegment, TextMatch};

#[derive(Debug, Clone)]
pub struct App {
    pub code: CodeResults,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub should_exit: bool,
    pub vertical_scroll: usize,
    pub selected_item_idx: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            code: crate::results::mock(),
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
            KeyCode::Char('q') | KeyCode::Esc => state.should_exit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                state.selected_item_idx = (state.selected_item_idx + 1) % self.code.items.len();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.selected_item_idx = state.selected_item_idx.saturating_sub(1);
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                let Some((item, _text_match)) =
                    self.iter_text_matches().nth(state.selected_item_idx)
                else {
                    return;
                };

                let _ = open::that(&item.html_url);
            }
            _ => {}
        }
    }
}

impl StatefulWidget for &mut App {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        buf.reset();

        let [_prompt_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(area);

        let [matches_area] = Layout::horizontal([Constraint::Fill(4)])
            .margin(2)
            .areas(main_area);

        App::render_footer(footer_area, buf);

        self.render_search_results(
            state,
            &self.code.items[state.selected_item_idx],
            matches_area,
            buf,
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

    fn render_search_results(
        &self,
        state: &mut AppState,
        _item_result: &ItemResult,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let block = Block::new().borders(Borders::ALL);

        let inner_area = block.inner(area);
        block.render(area, buf);

        let mut text_match_heights = vec![];
        let mut total_height = 0;

        for (_item, text_match) in self.iter_text_matches() {
            let h = smart_iter_lines(&text_match.fragment).count();
            text_match_heights.push(h);
            total_height += h;
            total_height += 3; // 2 for borders, 1 for margin
        }

        let mut tbuf = Buffer::empty(Rect::new(0, 0, inner_area.width, total_height as u16));
        let areas = Layout::vertical(
            text_match_heights
                .iter()
                .map(|&h| Constraint::Length(h as u16 + 3)),
        )
        .split(*tbuf.area());

        for (idx, (item, text_match)) in self.iter_text_matches().enumerate() {
            let area = areas[idx];
            self.render_text_match(idx, item, text_match, area, &mut tbuf, state);
        }

        // adjust the offset based on the selected item idx
        // Account for the +3 border lines added to each item
        let calculated_offset_start: usize = text_match_heights
            .iter()
            .take(state.selected_item_idx)
            .map(|&h| h + 3)
            .sum();
        let calculated_offset_end: usize = text_match_heights
            .iter()
            .take(state.selected_item_idx + 1)
            .map(|&h| h + 3)
            .sum();

        let h = inner_area.height as usize;
        let current_window_start = state.vertical_scroll;
        let current_window_end = state.vertical_scroll + h;

        // Scroll down if selected item's bottom is below the visible window
        if calculated_offset_end > current_window_end {
            state.vertical_scroll = calculated_offset_end - h;
        }
        // Scroll up if selected item's top is above the visible window
        if calculated_offset_start < current_window_start {
            state.vertical_scroll = calculated_offset_start;
        }

        // blit the buffer with scrolling
        crate::buffers::blit(buf, &tbuf, inner_area, (0, state.vertical_scroll as u16));
    }

    fn render_text_match(
        &self,
        idx: usize,
        item_result: &ItemResult,
        text_match: &TextMatch,
        area: Rect,
        buf: &mut Buffer,
        state: &AppState,
    ) {
        let repo_name = item_result.repository.full_name.as_str();
        let file_path = item_result.path.as_str();
        let block_title = format!(" {repo_name} {file_path} ");
        let block = Block::new().borders(Borders::TOP).title(
            Span::from(block_title).style(
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
        );

        let mut lines = vec![];

        for line in smart_iter_lines(&text_match.fragment) {
            // Translate tabs to spaces
            let content = line.content.replace("\t", "    ");
            let line_start = line.start;
            let line_end = line_start + content.len();
            let abs_line_range = line_start..line_end;

            let segments = fill_out_segments(abs_line_range.clone(), &text_match.matches);

            let mut vis_line = Line::default();
            for segment_match in segments {
                let local_start = segment_match.range.start - line.start;
                let local_end = segment_match.range.end - line.start;

                let local_range = local_start..local_end;

                let text = &content[local_range];
                let text = Cow::Owned(text.to_owned());

                let mut span = Span::from(text);

                if segment_match.is_match {
                    span = span.style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    );
                }

                vis_line.push_span(span);
            }

            lines.push(vis_line);
        }

        let paragraph_style = if state.selected_item_idx == idx {
            Style::default().reversed()
        } else {
            Style::default()
        };

        Paragraph::new(lines)
            .style(paragraph_style)
            .block(block)
            .render(area, buf);
    }

    fn iter_text_matches(&self) -> impl Iterator<Item = (&ItemResult, &TextMatch)> {
        self.code.items.iter().flat_map(|item| {
            item.text_matches
                .iter()
                .map(move |text_match| (item, text_match))
        })
    }
}

/// Takes in a list of segments and returns a fully allocated list of segments
///
/// e.g. given 11..20, 32..40 in context 0..100 it should return
/// 0..11, 11..20, 20..32, 32..40, 40..100
fn fill_out_segments(context: Range<usize>, segments: &[MatchSegment]) -> Vec<RangeSegment> {
    let ranges = segments.iter().map(|ms| ms.indices.0..ms.indices.1);
    fill_out_range_list(context, ranges)
}

/// Takes in a list of ranges and returns a fully allocated list of ranges
///
/// e.g. given 11..20, 32..40 in context 0..100 it should return
/// 0..11, 11..20, 20..32, 32..40, 40..100
///
/// Note the ranges are assumed to be sorted.
fn fill_out_range_list(
    context: Range<usize>,
    segments: impl IntoIterator<Item = Range<usize>>,
) -> Vec<RangeSegment> {
    let mut items = vec![];

    let mut current = context.start;
    for range in segments.into_iter() {
        if !are_ranges_overlapping(&context, &range) {
            continue;
        }

        if current < range.start {
            items.push(RangeSegment {
                range: current..range.start,
                is_match: false,
            });
        }

        let start = range.start.max(current);
        let end = range.end.min(context.end);

        if end > start {
            items.push(RangeSegment {
                range: start..end,
                is_match: true,
            });
        }

        current = end;
    }

    let end = context.end;

    if current < end {
        items.push(RangeSegment {
            range: current..end,
            is_match: false,
        });
    }

    items
}

fn are_ranges_overlapping(a: &Range<usize>, b: &Range<usize>) -> bool {
    b.contains(&a.start) || b.contains(&a.end) || a.contains(&b.start) || a.contains(&b.end)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RangeSegment {
    pub range: Range<usize>,
    pub is_match: bool,
}

fn smart_iter_lines(mut s: &str) -> impl Iterator<Item = SmartLineItem<'_>> {
    let mut counter = 0;

    std::iter::from_fn(move || {
        if s.is_empty() {
            return None;
        }
        let next_newline_carriage_return = s.find("\r\n");
        let next_newline = s.find('\n');

        let offset = if next_newline_carriage_return.is_some() {
            2
        } else if next_newline.is_some() {
            1
        } else {
            0
        };

        let next_newline = next_newline_carriage_return
            .or(next_newline)
            .unwrap_or(s.len());

        let item = SmartLineItem {
            content: &s[..next_newline],
            start: counter,
        };

        counter += next_newline + offset;
        s = &s[next_newline + offset..]; // TODO: +1?

        Some(item)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmartLineItem<'a> {
    pub content: &'a str,
    pub start: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn smart_lines_basic() {
        let content = "alpha\nbeta\ngamma";

        let smart_lines: Vec<SmartLineItem> = smart_iter_lines(content).collect();

        assert_eq!(
            smart_lines,
            vec![
                SmartLineItem {
                    content: "alpha",
                    start: 0,
                },
                SmartLineItem {
                    content: "beta",
                    start: 6,
                },
                SmartLineItem {
                    content: "gamma",
                    start: 11,
                }
            ]
        );
    }

    #[test]
    fn smart_lines_carriage_return() {
        let content = "alpha\r\nbeta\rgamma";

        let smart_lines: Vec<SmartLineItem> = smart_iter_lines(content).collect();

        assert_eq!(
            smart_lines,
            vec![
                SmartLineItem {
                    content: "alpha",
                    start: 0,
                },
                SmartLineItem {
                    content: "beta\rgamma",
                    start: 7,
                },
            ]
        );
    }

    #[test]
    fn smart_lines_empty_line() {
        let content = "alpha\n\ngamma";

        let smart_lines: Vec<SmartLineItem> = smart_iter_lines(content).collect();

        assert_eq!(
            smart_lines,
            vec![
                SmartLineItem {
                    content: "alpha",
                    start: 0,
                },
                SmartLineItem {
                    content: "",
                    start: 6,
                },
                SmartLineItem {
                    content: "gamma",
                    start: 7,
                },
            ]
        );
    }

    #[test_case(0..100, vec![25..50] => vec![0..25, 25..50, 50..100] ; "basic")]
    #[test_case(0..100, vec![25..150] => vec![0..25, 25..100] ; "overflow")]
    #[test_case(0..100, vec![200..300] => vec![0..100] ; "disjoint right")]
    #[test_case(200..300, vec![0..100] => vec![200..300] ; "disjoint left")]
    #[test_case(50..100, vec![0..75] => vec![50..75, 75..100] ; "underflow")]
    #[test_case(0..100, vec![0..100] => vec![0..100] ; "full")]
    #[test_case(0..100, vec![] => vec![0..100] ; "empty")]
    #[test_case(0..100, vec![3..11, 32..75] => vec![0..3, 3..11, 11..32, 32..75, 75..100] ; "disjoint")]
    #[test_case(0..100, vec![3..11, 11..75] => vec![0..3, 3..11, 11..75, 75..100] ; "touching")]
    fn fill_out_ranges(context: Range<usize>, ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
        let segments = fill_out_range_list(context, ranges);
        segments.into_iter().map(|s| s.range).collect::<Vec<_>>()
    }

    #[test]
    fn fill_out_ranges_annotations() {
        let context = 0..100;
        let segments = fill_out_range_list(context, std::iter::once(25..75));

        assert_eq!(
            segments,
            vec![
                RangeSegment {
                    range: 0..25,
                    is_match: false,
                },
                RangeSegment {
                    range: 25..75,
                    is_match: true,
                },
                RangeSegment {
                    range: 75..100,
                    is_match: false,
                },
            ]
        );
    }

    #[test_case(0..100, 25..150 => true)]
    #[test_case(0..100, 25..75 => true)]
    #[test_case(25..100, 0..50 => true)]
    #[test_case(0..100, 200..300 => false)]
    fn range_overlap(a: Range<usize>, b: Range<usize>) -> bool {
        are_ranges_overlapping(&a, &b)
    }
}
