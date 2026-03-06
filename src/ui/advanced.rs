use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Sparkline};

use crate::app::App;
use crate::ui::theme::resolve_palette_from_config;

fn render_line_graph(
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

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let Some(latest) = app.training.latest.as_ref() else {
        let text = Paragraph::new("No diagnostics available yet")
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(text, area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(36)])
        .split(rows[0]);

    let graphs = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(6)])
        .split(top[0]);

    let eval_width = usize::from(graphs[0].width.saturating_sub(2).max(1));
    let eval_history = app.training_viewport_series(&app.training.eval_loss_history, eval_width);
    let eval_block = Block::default()
        .title("Eval Loss (source: validation, unit: loss)")
        .borders(Borders::ALL);
    if eval_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No eval loss data")
                .block(eval_block)
                .style(Style::default().fg(palette.muted)),
            graphs[0],
        );
    } else if app.config.graph_mode == "line" {
        render_line_graph(
            frame,
            graphs[0],
            eval_block,
            "eval_loss",
            &eval_history,
            palette.loss_color,
        );
    } else {
        frame.render_widget(
            Sparkline::default()
                .block(eval_block)
                .data(&eval_history)
                .style(Style::default().fg(palette.loss_color)),
            graphs[0],
        );
    }

    let grad_width = usize::from(graphs[1].width.saturating_sub(2).max(1));
    let grad_history = app.training_viewport_series(&app.training.grad_norm_history, grad_width);
    let grad_block = Block::default()
        .title("Grad Norm (source: optimizer, unit: norm)")
        .borders(Borders::ALL);
    if grad_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No grad norm data")
                .block(grad_block)
                .style(Style::default().fg(palette.muted)),
            graphs[1],
        );
    } else if app.config.graph_mode == "line" {
        render_line_graph(
            frame,
            graphs[1],
            grad_block,
            "grad_norm",
            &grad_history,
            palette.lr_color,
        );
    } else {
        frame.render_widget(
            Sparkline::default()
                .block(grad_block)
                .data(&grad_history)
                .style(Style::default().fg(palette.lr_color)),
            graphs[1],
        );
    }

    let summary = Paragraph::new(format!(
        "Perplexity: {}\nLoss spikes: {}\nNaN/Inf count: {}\nParser ok/skip/err: {}/{}/{}\n\nLast loss spike: {}\nLast NaN/Inf: {}\nParser mode: {}\nHealth: {}",
        latest_or_dash(app.training.perplexity_latest, 3),
        app.training.loss_spike_count,
        app.training.nan_inf_count,
        app.training.parser_success_count,
        app.training.parser_skipped_count,
        app.training.parser_error_count,
        anomaly_age(app.training.last_loss_spike_at),
        anomaly_age(app.training.last_nan_inf_at),
        app.config.parser,
        app.training_data_health_state().label(),
    ))
    .alignment(Alignment::Left)
    .block(
        Block::default()
            .title("Stability Summary")
            .borders(Borders::ALL),
    )
    .style(Style::default().fg(palette.accent));
    frame.render_widget(summary, top[1]);

    let throughput_items = [
        (
            "tokens_per_second",
            "Tokens/s",
            "tok/s (throughput)",
            latest.tokens_per_second,
        ),
        (
            "samples_per_second",
            "Samples/s",
            "samples/s (dataloader)",
            latest.samples_per_second,
        ),
        (
            "steps_per_second",
            "Steps/s",
            "steps/s (optimizer)",
            latest.steps_per_second,
        ),
    ];

    let visible_items = throughput_items
        .iter()
        .filter(|(id, _, _, value)| app.should_show_metric_panel(id, value.is_some()))
        .collect::<Vec<_>>();

    if visible_items.is_empty() {
        let empty = Paragraph::new("No throughput metrics")
            .alignment(Alignment::Center)
            .block(Block::default().title("Throughput").borders(Borders::ALL))
            .style(Style::default().fg(palette.muted));
        frame.render_widget(empty, rows[1]);
    } else {
        let constraints =
            vec![Constraint::Ratio(1, visible_items.len() as u32); visible_items.len()];
        let throughput = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(rows[1]);

        for (idx, (_, label, unit, value)) in visible_items.iter().enumerate() {
            let panel = Paragraph::new(format!("{}\nunit: {unit}", latest_or_dash(*value, 3)))
                .alignment(Alignment::Center)
                .block(Block::default().title(*label).borders(Borders::ALL))
                .style(Style::default().fg(palette.accent));
            frame.render_widget(panel, throughput[idx]);
        }
    }
}

fn latest_or_dash(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(v) => format!("{v:.decimals$}"),
        None => "-".to_string(),
    }
}

fn anomaly_age(ts: Option<std::time::Instant>) -> String {
    match ts {
        Some(t) => format!("{}s ago", t.elapsed().as_secs()),
        None => "never".to_string(),
    }
}
