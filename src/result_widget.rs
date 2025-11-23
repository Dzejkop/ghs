use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

use crate::results::{CodeResults, MatchSegment, TextMatch};

pub struct ResultWidget<'a> {
    results: &'a CodeResults,
}

pub struct ResultWidgetState {
    pub scroll: usize,
}

impl<'a> StatefulWidget for ResultWidget<'a> {
    type State = ResultWidgetState;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        todo!()
    }
}

fn render_text_match(text_match: &TextMatch, area: Rect, buf: &mut Buffer) {
    let block = Block::new().borders(Borders::NONE);

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

    Paragraph::new(lines).clone().block(block).render(area, buf);
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
