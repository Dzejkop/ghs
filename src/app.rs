use std::ops::Range;

use color_eyre::eyre;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, prelude::*};

use crate::results::{CodeResults, ItemResult, MatchSegment};

#[derive(Debug, Clone)]
pub struct App {
    pub should_exit: bool,
    pub code: CodeResults,
}

impl App {
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> eyre::Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            };
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                // todo
            }
            _ => {}
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [list_area, item_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(main_area);

        App::render_footer(footer_area, buf);
        App::render_search_results(self.code.items[0].clone(), list_area, buf);
    }
}

impl App {
    fn render_footer(area: Rect, buf: &mut Buffer) {
        Paragraph::new("Use ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom.")
            .centered()
            .render(area, buf);
    }

    fn render_search_results(item_result: ItemResult, area: Rect, buf: &mut Buffer) {
        let block = Block::new().borders(Borders::ALL).title(item_result.name);

        let text_match = item_result.text_matches[0].clone();

        let mut lines = vec![];

        for line in smart_iter_lines(&text_match.fragment) {
            let line_start = line.start;
            let line_end = line_start + line.content.len();
            let abs_line_range = line_start..line_end;

            let mut curr_idx = 0;

            let mut line = Line::default();

            let find_next_segment_end = |idx: usize| {
                // we want to get the minimum of either (end of line idx, )
            };

            // loop {
            //     let next_segment_end = find_next_segment_end();
            //     line.push_span(span);
            // }

            for segment_match in &text_match.matches {
                let match_range = segment_match.indices.0..segment_match.indices.1;

                // if abs_line_range.contains(match_range) {}
            }

            // lines.push(Line::from(line.content));
        }

        Paragraph::new(lines).clone().block(block).render(area, buf);
    }
}

/// Takes in a list of segments and returns a fully allocated list of segments
///
/// e.g. given 11..20, 32..40 in context 0..100 it should return
/// 0..11, 11..20, 20..32, 32..40, 40..100
fn fill_out_segments(context: Range<usize>, segments: &[MatchSegment]) -> Vec<RangeSegment> {
    let mut items = vec![];

    items
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
    #[test_case(50..100, vec![0..75] => vec![50..75, 75..100] ; "underflow")]
    #[test_case(0..100, vec![0..100] => vec![0..100] ; "full")]
    #[test_case(0..100, vec![] => vec![0..100] ; "empty")]
    #[test_case(0..100, vec![3..11, 32..75] => vec![0..3, 3..11, 11..32, 32..75, 75..100] ; "disjoint")]
    #[test_case(0..100, vec![3..11, 11..75] => vec![0..3, 3..11, 11..75, 75..100] ; "touching")]
    fn fill_out_ranges(context: Range<usize>, ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
        let segments = fill_out_range_list(context, ranges);
        segments.into_iter().map(|s| s.range).collect::<Vec<_>>()
    }
}
