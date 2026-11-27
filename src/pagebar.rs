//! Where the page bar lies: along the bottom of the window, or standing down its left or right
//! side. A grip at the bar's start moves it: let go in the left or right quarter of the window, it
//! docks on that side; anywhere between, back along the bottom. The choice is the writer's, not the
//! story's: it is kept in the data folder (`page_bar.json`), beside `theme.json`.
//!
//! Standing, the bar holds the same things as lying, made to fit its width: the track runs from
//! top to bottom, New page is "+" and the Scratchpad an icon. The pages, the status message and the
//! windows that open beside them keep clear of the side the bar is on (`view_area`).

use std::path::PathBuf;

use eframe::egui::{self, FontId, Id, Rect, Sense, Stroke, pos2, vec2};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::theme::pal;
use crate::ui::{DOCK_GAP, DOCK_H};

/// What the page bar's grip says to the pointer and to a screen reader.
pub const GRIP_NAME: &str = "Move the page bar";
/// The grip's length along the bar, and the room the bar keeps for it at its start.
const GRIP_LEN: f32 = 16.0;
pub const GRIP_ROOM: f32 = 28.0;

/// The edge of the window the page bar is docked to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    #[default]
    Bottom,
    Right,
}

impl Side {
    /// Standing down a side of the window rather than lying along its bottom.
    pub fn vertical(self) -> bool {
        self != Side::Bottom
    }

    /// Where the bar goes when its grip is let go of at `x`: the left or right quarter of the
    /// window docks it on that side, the middle half along the bottom.
    pub fn at(area: Rect, x: f32) -> Side {
        let quarter = area.width() / 4.0;
        if x < area.left() + quarter {
            Side::Left
        } else if x > area.right() - quarter {
            Side::Right
        } else {
            Side::Bottom
        }
    }
}

/// The side chosen, where it is kept, and where the grip is while it is being dragged.
pub struct PageBar {
    pub side: Side,
    pub file: Option<PathBuf>,
    dragging: Option<egui::Pos2>,
    /// The bar has just docked, or the scratchpad just opened: a window lying over the bar is put
    /// back where it starts, beside it (`App::scratchpad`).
    pub clear_windows: bool,
}

impl PageBar {
    /// The side kept in the data folder, beside the theme.
    pub fn new() -> Self {
        Self::kept_in(crate::versions::default_dir().and_then(|d| d.parent().map(|p| p.join("page_bar.json"))))
    }

    pub fn kept_in(file: Option<PathBuf>) -> Self {
        let side = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|j| serde_json::from_str(&j).ok());
        Self { side: side.unwrap_or_default(), file, dragging: None, clear_windows: false }
    }

    /// Dock the bar on `side`, and keep it there for next time.
    pub fn choose(&mut self, side: Side) {
        if side == self.side {
            return;
        }
        self.side = side;
        self.clear_windows = true;
        if let (Some(file), Ok(json)) = (&self.file, serde_json::to_string(&side)) {
            let _ = file.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(file, json);
        }
    }
}

impl App {
    /// The page bar's dock in a window of `area`, on the side it is docked to. Standing, it runs
    /// from under the toolbar (and its second row, if open) down to the bottom of the window.
    pub fn page_bar_rect(&self, area: Rect) -> Rect {
        self.bar_rect_on(area, self.bar.side)
    }

    fn bar_rect_on(&self, area: Rect, side: Side) -> Rect {
        let x = match side {
            Side::Bottom => return Self::dock_rect(area, false),
            Side::Left => area.left() + DOCK_GAP,
            Side::Right => area.right() - DOCK_GAP - DOCK_H,
        };
        let top = self.tools_bottom.max(area.top() + DOCK_GAP + DOCK_H) + 10.0;
        Rect::from_min_max(pos2(x, top), pos2(x + DOCK_H, (area.bottom() - DOCK_GAP).max(top)))
    }

    /// The grip at the start of the bar (its left end, or its top). Dragged, it shows where the bar
    /// would go; let go, the bar docks there, and a page not fitted to the window keeps its place
    /// in the room left for the pages.
    pub fn page_bar_grip(&mut self, ui: &mut egui::Ui, area: Rect, rect: Rect) {
        let grip = if self.bar.side.vertical() {
            Rect::from_min_size(pos2(rect.center().x - 15.0, rect.top() + 5.0), vec2(30.0, GRIP_LEN))
        } else {
            Rect::from_min_size(pos2(rect.left() + 5.0, rect.center().y - 15.0), vec2(GRIP_LEN, 30.0))
        };
        let resp = ui.interact(grip, Id::new("page_bar_grip"), Sense::drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, GRIP_NAME));
        let held = resp.dragged() || resp.is_pointer_button_down_on();
        let ink = if held { pal().accent } else if resp.hovered() { pal().text } else { pal().text_dim };
        // Six dots, two rows of three across the grip.
        for k in 0..6 {
            let (a, b) = ((k % 3) as f32 - 1.0, (k / 3) as f32 - 0.5);
            let d = if self.bar.side.vertical() { vec2(a * 6.0, b * 6.0) } else { vec2(b * 6.0, a * 6.0) };
            ui.painter().circle_filled(grip.center() + d, 1.5, ink);
        }
        if resp.hovered() || held {
            ui.output_mut(|o| o.cursor_icon = if held { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
        }
        let resp = resp.on_hover_text(GRIP_NAME);
        let at = ui.input(|i| i.pointer.latest_pos());
        if resp.dragged() {
            self.bar.dragging = at.or(self.bar.dragging);
            if let Some(p) = self.bar.dragging {
                // Where it would dock, outlined over everything.
                let to = self.bar_rect_on(area, Side::at(area, p.x));
                let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("page_bar_drop"));
                let painter = ui.ctx().layer_painter(layer);
                painter.rect_filled(to, 16.0, pal().accent.gamma_multiply(0.15));
                painter.rect_stroke(to, 16.0, Stroke::new(2.0, pal().accent), egui::StrokeKind::Inside);
            }
        }
        if resp.drag_stopped() {
            if let Some(p) = at.or(self.bar.dragging) {
                let before = self.view_area(area);
                self.bar.choose(Side::at(area, p.x));
                self.origin.x += self.view_area(area).center().x - before.center().x;
            }
            self.bar.dragging = None;
        }
    }

    /// The page bar standing down a side: the grip and the page count at the top, then the track,
    /// then "+", the zoom, Fit and the Scratchpad down at the bottom. A short bar drops the zoom,
    /// then the Scratchpad, then "+", the track giving way first.
    pub fn standing_bar(&mut self, ui: &mut egui::Ui, r: Rect) {
        let n = self.doc.pages();
        let cx = r.center().x;
        let top = r.top() + GRIP_ROOM;
        let mut y = r.bottom() - 9.0;
        let room = y - top;
        let (show_fit, show_new, show_scratch, show_zoom) = (room >= 30.0, room >= 68.0, room >= 106.0, room >= 130.0);
        if show_scratch {
            let at = Rect::from_min_max(pos2(cx - 15.0, y - 30.0), pos2(cx + 15.0, y));
            let tip = "Plain notes on the right, beside the pages, saved with the document";
            if crate::ui::pad_button(ui, at, self.scratchpad_open, "Scratchpad", tip).clicked() {
                self.toggle_scratchpad();
            }
            y -= 38.0;
        }
        if show_fit {
            let at = Rect::from_min_max(pos2(cx - 20.0, y - 30.0), pos2(cx + 20.0, y));
            let padding = ui.spacing().button_padding;
            // "Fit" in the bar's width.
            ui.spacing_mut().button_padding.x = 6.0;
            let fit = ui.put(at, egui::Button::new("Fit").selected(self.fit));
            ui.spacing_mut().button_padding = padding;
            if fit.on_hover_text("Show the full page (Ctrl+0). Ctrl+drag moves the page.").clicked() {
                self.fit = true;
                self.fit_settling = true;
            }
            y -= 38.0;
        }
        if show_zoom {
            let text = format!("{:.0}%", self.zoom * 100.0);
            ui.painter().text(pos2(cx, y - 8.0), egui::Align2::CENTER_CENTER, text, FontId::proportional(11.0), pal().text_dim);
            y -= 24.0;
        }
        if show_new {
            let at = Rect::from_min_max(pos2(cx - 15.0, y - 30.0), pos2(cx + 15.0, y));
            if ui.put(at, egui::Button::new("+")).on_hover_text("New page").clicked() {
                let cur = self.target;
                let ctx = ui.ctx().clone();
                self.insert_page_after(&ctx, cur);
            }
            y -= 30.0;
        }
        // The track, and the page count above it when there is room for both.
        let track_bottom = y - 16.0;
        let show_label = track_bottom - top >= 40.0 + 24.0;
        let track_top = if show_label { top + 24.0 } else { top + 6.0 };
        if show_label {
            let scrubbed = self.scrub_page().filter(|_| self.appearance == crate::Appearance::Paperstack);
            let shown = scrubbed.unwrap_or(self.pos.round() as usize);
            let text = format!("{}/{}", shown + 1, n);
            ui.painter().text(pos2(cx, top + 6.0), egui::Align2::CENTER_CENTER, text, FontId::proportional(11.0), pal().text_dim);
        }
        if track_bottom - track_top >= 40.0 {
            self.page_track(ui, Rect::from_min_max(pos2(cx - 4.0, track_top), pos2(cx + 4.0, track_bottom)), true);
        }
    }
}

