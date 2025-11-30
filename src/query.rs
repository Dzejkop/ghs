use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Query<'a> {
    pub raw: &'a str,

    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub span: Range<usize>,
    pub span_type: SpanType,
}

#[derive(Debug, Clone)]
pub enum SpanType {
    Negative,
}
