use super::ShieldView;
use crate::ui::{components::pie_chart, theme::Palette};
use gpui_kit::{prelude::*, *};

impl ShieldView {
    pub(super) fn flush_statistics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let revision = self.history.revision();
        if self.statistics_saving
            || self.statistics_failed
            || self.quitting
            || revision == self.statistics_saved
        {
            return;
        }
        self.statistics_saving = true;
        let saving = self.statistics_store.save(self.history.lifetime());
        cx.spawn_in(window, async move |view, cx| {
            let result = saving.await;
            let _ = view.update_in(cx, |this, _, cx| {
                this.statistics_saving = false;
                match result {
                    Ok(()) => this.statistics_saved = revision,
                    Err(error) => {
                        this.statistics_failed = true;
                        this.fail(error, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn quit(&mut self, cx: &mut Context<Self>) {
        if self.quitting {
            return;
        }
        self.cancel_processing(cx);
        self.shortcuts.take();
        self.quitting = true;
        if self.statistics_failed {
            cx.quit();
            return;
        }
        let saving = self.statistics_store.save(self.history.lifetime());
        cx.spawn(async move |view, cx| {
            let result = saving.await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(()) => cx.quit(),
                Err(error) => {
                    this.quitting = false;
                    this.statistics_failed = true;
                    this.fail(error, cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn render_statistics(&self, compact: bool, cx: &Context<Self>) -> impl IntoElement {
        let palette = Palette::current(cx);
        let totals = self.history.lifetime();
        let count = totals.operations();
        let cards = [
            (
                "Operations",
                count.to_string(),
                "Completed, cancelled and failed",
            ),
            (
                "Replacements",
                totals.replacements.to_string(),
                "In completed operations",
            ),
            (
                "Input volume",
                format_bytes(totals.input_bytes),
                "All started operations",
            ),
            (
                "Average duration",
                format!("{} ms", totals.average_millis()),
                "Including waits and cancellations",
            ),
        ]
        .into_iter()
        .map(|(label, value, note)| metric(label, value, note, palette))
        .collect::<Vec<_>>();
        let legend = [
            ("Completed", totals.completed, palette.success),
            ("Cancelled", totals.cancelled, palette.warning),
            ("Failed", totals.failed, palette.danger),
        ]
        .into_iter()
        .map(|(label, value, color)| {
            let percent = if count == 0 {
                0.
            } else {
                value as f64 * 100. / count as f64
            };
            div()
                .flex()
                .items_center()
                .gap_3()
                .py_3()
                .border_b_1()
                .border_color(palette.border)
                .child(div().w(px(10.)).h(px(10.)).rounded_full().bg(color))
                .child(div().flex_1().child(label))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(value.to_string()),
                )
                .child(
                    div()
                        .w(px(66.))
                        .text_right()
                        .text_color(palette.muted)
                        .text_sm()
                        .child(format!("{percent:.1} %")),
                )
        })
        .collect::<Vec<_>>();
        div().id("lifetime-statistics").debug_selector(|| "lifetime-statistics".into()).flex().flex_col().gap_6().flex_1().min_h_0().overflow_y_scroll()
            .child(div().debug_selector(|| "operation-metrics".into()).flex().flex_wrap().gap_4().children(cards))
            .child(div().debug_selector(|| "outcome-panel".into()).flex().gap_6().items_center().when(compact, |panel| panel.flex_col()).bg(palette.surface).rounded(px(8.)).border_1().border_color(palette.border).p_6()
                .child(pie_chart([(totals.completed, palette.success), (totals.cancelled, palette.warning), (totals.failed, palette.danger)], palette.border))
                .child(div().debug_selector(|| "outcome-legend".into()).flex_1().flex().flex_col().gap_2()
                    .child(div().text_size(px(16.)).line_height(px(24.)).font_weight(FontWeight::MEDIUM).child("Operation outcomes"))
                    .child(div().text_sm().text_color(palette.muted).child(if count == 0 { "The chart will populate after the first operation." } else { "Since statistics collection began" }))
                    .children(legend)))
            .child(div().flex().flex_wrap().gap_4()
                .child(metric("Anonymization", totals.anonymize.to_string(), "Operations without local markers", palette))
                .child(metric("Restoration", totals.restore.to_string(), "Exact restoration of local markers", palette))
                .child(metric("Unresolved direction", totals.unresolved.saturating_add(totals.unknown_direction).to_string(), "Unknown markers or rejected input", palette)))
            .child(div().text_sm().text_color(palette.muted).child(if self.statistics_store.is_persistent() && !self.statistics_failed {
                "Only aggregate numeric totals are saved. Documents, markers, mappings and individual history entries are not saved. Collection begins with this version; older runs cannot be reconstructed. Clear totals in Settings → Sessions and data."
            } else { "Temporary totals for this preview / run only. No documents or individual operations are saved." }))
    }
}

fn metric(
    label: &'static str,
    value: String,
    note: &'static str,
    palette: Palette,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(180.))
        .flex_shrink_0()
        .bg(palette.surface)
        .rounded(px(8.))
        .border_1()
        .border_color(palette.border)
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().text_color(palette.muted).child(label))
        .child(
            div()
                .text_size(px(28.))
                .line_height(px(36.))
                .font_weight(FontWeight::LIGHT)
                .text_color(palette.ink)
                .child(value),
        )
        .child(div().text_xs().text_color(palette.muted).child(note))
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.)
    } else if bytes >= 1_024 {
        format!("{:.1} KiB", bytes as f64 / 1_024.)
    } else {
        format!("{bytes} B")
    }
}
