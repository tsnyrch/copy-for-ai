use super::ShieldView;
use crate::{application::OperationStatus, ui::theme::Palette};
use copy_for_ai_core::Direction;
use gpui_kit::{prelude::*, *};

impl ShieldView {
    pub(super) fn render_history(&self, cx: &Context<Self>) -> impl IntoElement {
        let palette = Palette::current(cx);
        let summary = self.history.summary();
        let rows = self
            .history
            .entries()
            .iter()
            .rev()
            .enumerate()
            .map(|(index, entry)| {
                let seconds = entry
                    .time
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let direction = match entry.direction {
                    Some(Direction::Anonymize) => "Anonymization",
                    Some(Direction::Restore) => "Restoration",
                    Some(Direction::Unresolved) => "Unknown markers",
                    None => "Unresolved direction",
                };
                let (status, color) = match entry.status {
                    OperationStatus::Completed => ("Completed", palette.success),
                    OperationStatus::Cancelled => ("Cancelled", palette.warning),
                    OperationStatus::Failed => ("Failed", palette.danger),
                };
                div()
                    .id(("history-row", index))
                    .flex()
                    .items_center()
                    .gap_4()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(palette.border)
                    .text_sm()
                    .child(div().w(px(76.)).text_color(palette.muted).child(format!(
                        "{:02}:{:02}:{:02}",
                        seconds / 3600 % 24,
                        seconds / 60 % 60,
                        seconds % 60
                    )))
                    .child(div().flex_1().child(
                        div().flex().flex_col().gap_1()
                            .child(direction)
                            .child(div().text_xs().text_color(palette.muted).child(
                                entry.metrics.as_ref().map_or_else(
                                    || "Phase diagnostics are unavailable.".to_owned(),
                                    |metrics| format!(
                                        "Model {} ms · preparation {} ms · loading {} ms · core {} ms · paste {} ms · chunks {} / reuse {}{}",
                                        metrics.ner.inference.as_millis(),
                                        (metrics.ner.windowing + metrics.ner.tokenization + metrics.ner.preprocessing + metrics.ner.decoding).as_millis(),
                                        (metrics.ner.verification + metrics.ner.loading).as_millis(),
                                        metrics.core.as_millis(), metrics.paste_wait.as_millis(),
                                        metrics.ner.windows, metrics.ner.reused_windows,
                                        if metrics.cached_detection { " · last detection from memory" } else { "" },
                                    ),
                                )
                            ))
                            .when_some(entry.failure, |row, failure| row.child(div().text_xs().text_color(palette.danger).child(failure.message())))
                    ))
                    .child(
                        div().w(px(100.)).child(
                            div()
                                .rounded(px(10.))
                                .px_2()
                                .py(px(1.))
                                .text_xs()
                                .line_height(px(18.))
                                .text_color(color)
                                .bg(color.opacity(0.1))
                                .child(status),
                        ),
                    )
                    .child(
                        div()
                            .w(px(72.))
                            .text_right()
                            .child(format!("{} ms", entry.duration.as_millis())),
                    )
                    .child(
                        div()
                            .w(px(92.))
                            .text_right()
                            .child(format!("{} B", entry.input_bytes)),
                    )
                    .child(
                        div()
                            .w(px(56.))
                            .text_right()
                            .font_weight(FontWeight::MEDIUM)
                            .child(entry.replacements.to_string()),
                    )
            })
            .collect::<Vec<_>>();
        div().flex().flex_col().gap_4().flex_1().min_h_0()
            .child(div().text_sm().text_color(palette.muted).child(format!("{} completed · {} cancelled · {} failed · {} replacements", summary.completed, summary.cancelled, summary.failed, summary.replacements)))
            .child(div().id("history-table-scroll").debug_selector(|| "history-table-scroll".into()).flex().flex_col().flex_1().min_h_0().min_w_0().overflow_x_scroll()
                .child(div().flex().flex_col().flex_1().min_h_0().min_w(px(760.)).bg(palette.surface).rounded(px(8.)).border_1().border_color(palette.border).overflow_hidden()
                .child(div().flex().gap_4().px_4().py_3().text_xs().font_weight(FontWeight::MEDIUM).text_color(palette.muted).border_b_1().border_color(palette.border)
                    .child(div().w(px(76.)).child("Time · UTC")).child(div().flex_1().child("Direction"))
                    .child(div().w(px(100.)).child("Status")).child(div().w(px(72.)).text_right().child("Duration"))
                    .child(div().w(px(92.)).text_right().child("Input")).child(div().debug_selector(|| "replacement-column".into()).w(px(56.)).text_right().child("Replacements")))
                .child(div().id("session-history").debug_selector(|| "session-history".into()).flex_1().min_h_0().overflow_y_scroll().children(rows)
                    .when(self.history.entries().is_empty(), |table| table.child(div().p_8().text_center().text_color(palette.muted).child("No operations yet. Your first operation will appear here."))))))
            .child(div().text_xs().text_color(palette.muted).child("Last 100 operations in this run, metadata in memory only. Lifetime totals are in Statistics."))
    }
}
