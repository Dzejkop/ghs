use std::ops::Range;

use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::{DefaultTerminal, prelude::*};

use crate::results::{CodeResults, ItemResult, MatchSegment, TextMatch};

#[derive(Debug, Clone)]
pub struct App {
    pub code: CodeResults,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub should_exit: bool,
    pub scrollbar_state: ScrollbarState,
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

impl Default for AppState {
    fn default() -> Self {
        Self {
            should_exit: false,
            scrollbar_state: Default::default(),
            vertical_scroll: 0,
            selected_item_idx: 0,
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
            match event {
                Event::Key(key) => app.handle_key(key, &mut app_state),
                Event::Resize(w, h) => app.handle_resize(w, h),
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
            KeyCode::Char('l') | KeyCode::Right => {
                state.vertical_scroll = state.vertical_scroll.saturating_sub(1);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                state.vertical_scroll = state.vertical_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    fn handle_resize(&mut self, w: u16, h: u16) {
        //
    }
}

impl StatefulWidget for &mut App {
    type State = AppState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut AppState) {
        buf.reset();

        let [_, main_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [sidebar_area, matches_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(4)])
                .margin(2)
                .areas(main_area);

        state.scrollbar_state = state.scrollbar_state.content_length(100);
        state.scrollbar_state = state.scrollbar_state.position(state.vertical_scroll);

        let mut lines = Vec::default();
        for (idx, item) in self.code.items.iter().enumerate() {
            let name = format!("{} ({})", item.name, item.repository.full_name);
            if idx == state.selected_item_idx {
                lines.push(Line::from(name).reversed());
            } else {
                lines.push(Line::from(name));
            }
        }
        let paragraph = Paragraph::new(lines);

        paragraph.render(sidebar_area, buf);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .render(sidebar_area, buf, &mut state.scrollbar_state);

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
        Paragraph::new("Use ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom.")
            .centered()
            .render(area, buf);
    }

    fn render_search_results(
        &self,
        state: &mut AppState,
        item_result: &ItemResult,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::LightRed))
            .style(Style::default().bg(Color::LightYellow))
            .title(
                Span::from(&item_result.name).style(
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                ),
            );

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

        for (idx, (_item, text_match)) in self.iter_text_matches().enumerate() {
            let area = areas[idx];
            self.render_text_match(idx, text_match, area, &mut tbuf, state);
        }

        // adjust the offset based on the selected item idx
        let calculated_offset_start: usize = text_match_heights
            .iter()
            .take(state.selected_item_idx)
            .copied()
            .sum();
        let calculated_offset_end: usize = text_match_heights
            .iter()
            .take(state.selected_item_idx + 1)
            .copied()
            .sum();

        if calculated_offset_start < state.vertical_scroll {
            state.vertical_scroll = calculated_offset_start;
        }
        let h = inner_area.height as usize;
        if calculated_offset_end > state.vertical_scroll + h {
            state.vertical_scroll = calculated_offset_start + h / 2;
        }

        if state.vertical_scroll != 0 {
            panic!(
                "Based on COS={}, COE={}, h={}, state.vertical_scroll is {}",
                calculated_offset_start, calculated_offset_end, h, state.vertical_scroll
            );
        }

        // blit the buffer with scrolling
        crate::buffers::blit(buf, &tbuf, (0, 0), (0, 0));
        // for y in inner_area.y..inner_area.height {
        //     for x in inner_area.x..inner_area.width {
        //         let tbuf_y = y - inner_area.y + state.vertical_scroll as u16;
        //         let tbuf_x = x - inner_area.x;
        //
        //         let c = tbuf.cell((tbuf_x, tbuf_y)).unwrap();
        //         let bc = buf.cell_mut((x, y)).unwrap();
        //
        //         bc.set_symbol(c.symbol());
        //         bc.set_style(c.style());
        //     }
        // }
    }

    fn render_text_match(
        &self,
        idx: usize,
        text_match: &TextMatch,
        area: Rect,
        buf: &mut Buffer,
        state: &AppState,
    ) {
        let block = Block::new().borders(Borders::ALL);

        let mut lines = vec![];

        for line in smart_iter_lines(&text_match.fragment) {
            let line_start = line.start;
            let line_end = line_start + line.content.len();
            let abs_line_range = line_start..line_end;

            let segments = fill_out_segments(abs_line_range.clone(), &text_match.matches);

            let mut vis_line = Line::default();
            for segment_match in segments {
                let local_start = segment_match.range.start - line.start;
                let local_end = segment_match.range.end - line.start;

                let local_range = local_start..local_end;

                let text = &line.content[local_range];

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

fn count_lines(s: &str) -> usize {
    smart_iter_lines(s).count()
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
