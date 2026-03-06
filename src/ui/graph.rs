use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType};

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
