# rimelight_ui

Slint component library: markup only, compiled by the app crates. See `src/lib.rs` for how an app uses it.

## Charts (`ui/components/chart.slint`)

Four components, drawn with `Rectangle` and `Path` from models. Colours come from `Tokens` only.

| Component | Input | Notes |
|---|---|---|
| `LineChart` | `points: [ChartPoint]`, `y-min`, `y-max`, `unit`, `colour` | Line, a dot per point, five value guides, point labels, hover tooltip "label: value unit". |
| `BarChart` | the same as `LineChart` | Bars from `y-min`, same axes and tooltip. |
| `ProgressRing` | `value` (0 to 1), `colour`, `centre-text`, `thickness` | Track plus arc from the top, clockwise. Use it for one value that can reach 100 %. |
| `DonutChart` | `slices: [ChartSlice]`, `centre-text`, `thickness` | Arcs with `start` and `span` as fractions of the circle. No per-slice hover. |

- `ChartPoint { label: string, value: float }`: the caller passes points oldest first.
- `ChartSlice { label, value, start, span, colour }`: the caller computes `start` and `span` from its values, because
  the markup has no loop that can sum them.
- `ChartPalette.series`: the series colours from the Tokens palette (accent, success, warning, danger, info).
- Empty state: with no points or slices the chart draws its frame and `empty-text` ("No data yet" by default).
- The value range is set by the caller (`y-min`, `y-max`). The markup does not find the range.
- Long label lists: `x-label-every` shows every nth label.
- `ui_check` (`apps/ui_check/ui/main.slint`) places one of each so the Slint compiler checks them.

Not yet: area fills, a legend component, gridlines at custom steps, per-slice hover on the donut, and a time axis that
spaces points by date rather than by index.
