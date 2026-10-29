use gpui_kit::*;
use std::f32::consts::{PI, TAU};

pub fn pie_chart(values: [(u64, Hsla); 3], empty: Hsla) -> impl IntoElement {
    canvas(
        move |_, _, _| (),
        move |bounds, _, window, _| {
            let total = values.iter().map(|(count, _)| *count as f64).sum::<f64>();
            let center = bounds.center();
            let radius = bounds.size.width.min(bounds.size.height) * 0.44;
            if radius <= px(0.) {
                return;
            }
            let mut angle = -PI / 2.;
            let slices = if total == 0. {
                vec![(1., empty)]
            } else {
                values
                    .iter()
                    .filter(|(count, _)| *count > 0)
                    .map(|(count, color)| (*count as f64 / total, *color))
                    .collect::<Vec<_>>()
            };
            for (fraction, color) in slices {
                let end = angle + TAU * fraction as f32;
                let position = |value: f32| {
                    point(
                        center.x + radius * value.cos(),
                        center.y + radius * value.sin(),
                    )
                };
                let mut path = PathBuilder::fill();
                path.move_to(center);
                path.line_to(position(angle));
                if fraction > 0.999_999 {
                    path.arc_to(
                        point(radius, radius),
                        px(0.),
                        false,
                        true,
                        position(angle + PI),
                    );
                    path.arc_to(point(radius, radius), px(0.), false, true, position(end));
                } else {
                    path.arc_to(
                        point(radius, radius),
                        px(0.),
                        fraction > 0.5,
                        true,
                        position(end),
                    );
                }
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
                angle = end;
            }
        },
    )
    .w(px(240.))
    .h(px(240.))
    .flex_shrink_0()
}
