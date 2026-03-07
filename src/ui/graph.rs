use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Sparkline};

/// Reusable graph widget that handles sparkline/line mode, empty state,
/// block+borders, and focus highlighting — single component for all graph boxes.
pub struct MetricGraph<'a> {
    pub title: &'a str,
    pub data: &'a [u64],
    pub color: Color,
    pub graph_mode: &'a str,
    pub focused: bool,
    pub focus_index: Option<u8>,
    pub empty_message: &'a str,
    pub accent: Color,
    pub muted: Color,
    pub header_fg: Color,
}

impl<'a> MetricGraph<'a> {
    pub fn new(title: &'a str, data: &'a [u64], color: Color) -> Self {
        Self {
            title,
            data,
            color,
            graph_mode: "sparkline",
            focused: false,
            focus_index: None,
            empty_message: "No data",
            accent: Color::Rgb(137, 180, 250),
            muted: Color::DarkGray,
            header_fg: Color::Rgb(205, 214, 244),
        }
    }

    pub fn graph_mode(mut self, mode: &'a str) -> Self {
        self.graph_mode = mode;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn focus_index(mut self, index: Option<u8>) -> Self {
        self.focus_index = index;
        self
    }

    pub fn empty_message(mut self, msg: &'a str) -> Self {
        self.empty_message = msg;
        self
    }

    pub fn palette(mut self, accent: Color, muted: Color, header_fg: Color) -> Self {
        self.accent = accent;
        self.muted = muted;
        self.header_fg = header_fg;
        self
    }

    pub fn render(self, frame: &mut Frame, area: Rect) {
        let title = if let Some(idx) = self.focus_index {
            format!("[{}] {}", idx, self.title)
        } else {
            self.title.to_string()
        };

        let border_color = if self.focused {
            self.accent
        } else {
            self.muted
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_style(
                Style::default()
                    .fg(self.header_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .border_style(Style::default().fg(border_color));

        if self.data.is_empty() {
            let para = Paragraph::new(self.empty_message)
                .block(block)
                .alignment(Alignment::Center)
                .style(Style::default().fg(self.muted));
            frame.render_widget(para, area);
        } else if self.graph_mode == "line" {
            render_line_graph(frame, area, block, self.title, self.data, self.color);
        } else {
            let sparkline = Sparkline::default()
                .block(block)
                .data(self.data)
                .style(Style::default().fg(self.color));
            frame.render_widget(sparkline, area);
        }
    }
}

pub fn render_line_graph(
    frame: &mut Frame,
    area: Rect,
    block: Block,
    name: &str,
    series: &[u64],
    color: Color,
) {
    let points = series
        .iter()
        .enumerate()
        .map(|(idx, value)| (idx as f64, *value as f64))
        .collect::<Vec<_>>();
    let max_y = points
        .iter()
        .map(|(_, y)| *y)
        .fold(1.0_f64, |acc, y| acc.max(y));
    let max_x = points.len().saturating_sub(1) as f64;

    let dataset = Dataset::default()
        .name(name)
        .graph_type(GraphType::Line)
        .marker(Marker::Dot)
        .style(Style::default().fg(color))
        .data(&points);

    let chart = Chart::new(vec![dataset])
        .block(block)
        .x_axis(Axis::default().bounds([0.0, max_x.max(1.0)]))
        .y_axis(Axis::default().bounds([0.0, max_y]));
    frame.render_widget(chart, area);
}
