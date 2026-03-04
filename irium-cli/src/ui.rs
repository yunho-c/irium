use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Tabs, Wrap,
    },
};
use ratatui_image::{FilterType, Resize, StatefulImage};

use crate::{
    model::{
        AiSettingsField, AppState, Capitalization, ClickTarget, FilesPreviewStatus, FocusPane,
        InputMode, NameLength, NamingAiStatus, NamingTab, RenameRow, ScopeTab, Separator, Stage,
        ToastLevel,
    },
    theme::Theme,
};

pub fn draw(frame: &mut Frame<'_>, app: &mut AppState) {
    app.click_regions.clear();

    let area = frame.area();
    frame.render_widget(Block::default().style(Theme::base()), area);

    if area.width < 80 || area.height < 24 {
        draw_minimum_size_warning(frame, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(area);

    draw_stage_tabs(frame, app, chunks[0]);
    draw_subtabs(frame, app, chunks[1]);

    match app.stage {
        Stage::Scope => draw_scope(frame, app, chunks[2]),
        Stage::Naming => draw_naming(frame, app, chunks[2]),
        Stage::Apply => draw_apply(frame, app, chunks[2]),
    }

    draw_footer(frame, app, chunks[3]);
    draw_toast(frame, app, chunks[4]);

    if app.show_help {
        draw_help_overlay(frame, app);
    }

    if app.mode == InputMode::EditingOverride {
        draw_override_overlay(frame, app);
    }

    if app.mode == InputMode::NewCategory {
        draw_new_category_overlay(frame, app);
    }

    if app.show_log_overlay {
        draw_log_overlay(frame, app);
    }

    if app.show_prompt_history_overlay {
        draw_prompt_history_overlay(frame, app);
    }
}

fn draw_minimum_size_warning(frame: &mut Frame<'_>, area: Rect) {
    let block = Block::default().title(" irm ").borders(Borders::ALL);
    let text = Paragraph::new("Terminal too small. Resize to at least 80x24.")
        .style(Theme::muted_text())
        .block(block)
        .centered();
    frame.render_widget(text, center(area, 60, 30));
}

fn draw_stage_tabs(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let title_width = area.width.min(30);
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(title_width), Constraint::Min(0)])
        .split(area);
    let title_color = if app.title_startup_fx.is_some() {
        Theme::warning()
    } else {
        Theme::text()
    };
    let title_line = Line::from(vec![
        Span::styled(
            "IRIUM",
            Style::default()
                .fg(title_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(": intelligent renamer", Style::default().fg(title_color)),
    ]);
    frame.render_widget(Paragraph::new(title_line), layout[0]);
    if let Some(effect) = app.title_startup_fx.as_mut() {
        let now = Instant::now();
        let delta = app
            .title_fx_last_frame
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_else(|| Duration::from_millis(0));
        app.title_fx_last_frame = Some(now);
        effect.process(delta.into(), frame.buffer_mut(), layout[0]);
        if effect.done() {
            app.title_startup_fx = None;
            app.title_fx_last_frame = None;
            let title_line = Line::from(vec![
                Span::styled(
                    "IRIUM",
                    Style::default()
                        .fg(Theme::text())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(": intelligent renamer", Style::default().fg(Theme::text())),
            ]);
            frame.render_widget(Paragraph::new(title_line), layout[0]);
        }
    }

    if layout[1].width == 0 {
        return;
    }

    let titles: Vec<Line<'_>> = Stage::ALL
        .iter()
        .map(|stage| Line::from(Span::styled(stage.title(), Theme::muted_text())))
        .collect();
    let selected = Stage::ALL
        .iter()
        .position(|stage| *stage == app.stage)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .style(Theme::panel())
        .highlight_style(Theme::accent_text())
        .select(selected)
        .divider(" | ");
    frame.render_widget(tabs, layout[1]);

    for (idx, rect) in segment_rects(layout[1], Stage::ALL.len())
        .into_iter()
        .enumerate()
    {
        app.click_regions
            .register(rect, ClickTarget::StageTab(Stage::ALL[idx]));
    }
}

fn draw_subtabs(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    match app.stage {
        Stage::Scope => {}
        Stage::Naming => {
            frame.render_widget(
                Paragraph::new(
                    "Right pane: AI suggestions (top) + Style Controls (bottom). p: prompt history",
                )
                .style(Theme::muted_text()),
                area,
            );
        }
        Stage::Apply => {
            let text = Paragraph::new("Review simulated changes and session undo history")
                .style(Theme::muted_text());
            frame.render_widget(text, area);
        }
    }
}

fn draw_scope(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    draw_scope_left(frame, app, split[0]);
    if app.files_preview_visible && app.scope_tab == ScopeTab::Files {
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(split[1]);
        draw_scope_right(frame, app, right[0]);
        draw_file_preview_pane(frame, app, right[1]);
    } else {
        draw_scope_right(frame, app, split[1]);
    }
}

fn draw_scope_left(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let focused = matches!(app.focus, FocusPane::ScopeCategory | FocusPane::ScopeFiles);
    frame.render_widget(focus_block("Select", focused), area);
    let inner = inner_rect(area);
    if inner.height < 6 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(inner);

    if sections[0].height > 0 && sections[0].width > 0 {
        draw_scope_select(
            frame,
            app,
            sections[0],
            app.focus == FocusPane::ScopeCategory,
        );
    }

    let files_focus = app.focus == FocusPane::ScopeFiles;
    let files_title = if app.show_selected_categories_only {
        "Files (filtered)"
    } else {
        "Files"
    };
    frame.render_widget(focus_block(files_title, files_focus), sections[1]);
    let settings_label = "\u{f013} [p]";
    let settings_width = settings_label.chars().count() as u16;
    if sections[1].width > settings_width + 1 {
        let icon_area = Rect::new(
            sections[1].x + sections[1].width.saturating_sub(settings_width + 1),
            sections[1].y,
            settings_width,
            1,
        );
        let icon_line = Line::from(vec![
            Span::styled("\u{f013}", Theme::accent_text()),
            Span::raw(" "),
            Span::raw("[p]"),
        ]);
        frame.render_widget(Paragraph::new(icon_line), icon_area);
        app.click_regions
            .register(icon_area, ClickTarget::ScopeFilesSettingsButton);
    }
    let files_inner = inner_rect(sections[1]);
    if files_inner.height > 0 && files_inner.width > 0 {
        draw_scope_files(frame, app, files_inner);
    }

    app.click_regions.register(
        sections[0],
        ClickTarget::ScopePane(FocusPane::ScopeCategory),
    );
    app.click_regions
        .register(sections[1], ClickTarget::ScopePane(FocusPane::ScopeFiles));
}

fn draw_scope_right(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let focused = app.focus == FocusPane::ScopeOptions;
    frame.render_widget(focus_block("Options", focused), area);
    let inner = inner_rect(area);
    if inner.height < 3 {
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let right_tabs = [
        ScopeTab::Constraint,
        ScopeTab::Preset,
        ScopeTab::Marketplace,
    ];
    let selected = match app.scope_right_tab {
        ScopeTab::Constraint => 0,
        ScopeTab::Preset => 1,
        ScopeTab::Marketplace => 2,
        ScopeTab::Files | ScopeTab::Select => 0,
    };
    frame.render_widget(
        Tabs::new(
            right_tabs
                .iter()
                .map(|tab| Line::from(tab.title()))
                .collect::<Vec<_>>(),
        )
        .style(Theme::muted_text())
        .highlight_style(Theme::accent_text())
        .select(selected)
        .divider("  "),
        layout[0],
    );
    for (idx, rect) in segment_rects(layout[0], right_tabs.len())
        .into_iter()
        .enumerate()
    {
        app.click_regions
            .register(rect, ClickTarget::ScopeTab(right_tabs[idx]));
    }

    match app.scope_right_tab {
        ScopeTab::Constraint => draw_scope_constraint(frame, app, layout[1]),
        ScopeTab::Preset => draw_scope_preset(frame, app, layout[1]),
        ScopeTab::Marketplace => draw_scope_marketplace(frame, app, layout[1]),
        ScopeTab::Files | ScopeTab::Select => draw_scope_constraint(frame, app, layout[1]),
    }

    app.click_regions
        .register(layout[1], ClickTarget::ScopePane(FocusPane::ScopeOptions));
}

fn draw_file_preview_pane(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    frame.render_widget(focus_block("Preview", false), area);
    let inner = inner_rect(area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    match app.files_preview_status {
        FilesPreviewStatus::Ready => {
            let draw_area = centered_preview_area(app, inner);
            if let Some(protocol) = app.files_preview_protocol.as_mut() {
                frame.render_stateful_widget(
                    StatefulImage::default().resize(Resize::Fit(Some(FilterType::CatmullRom))),
                    draw_area,
                    protocol,
                );
                if let Some(error) = protocol.last_encoding_result().and_then(Result::err) {
                    app.files_preview_status = FilesPreviewStatus::Error;
                    app.files_preview_error = Some(format!("Preview render error: {error}"));
                    app.files_preview_protocol = None;
                }
            } else {
                frame.render_widget(
                    Paragraph::new("Preview data is not available yet.")
                        .style(Theme::muted_text())
                        .wrap(Wrap { trim: true }),
                    inner,
                );
            }
        }
        FilesPreviewStatus::Loading => {
            frame.render_widget(
                Paragraph::new("Loading preview...")
                    .style(Theme::muted_text())
                    .wrap(Wrap { trim: true }),
                inner,
            );
        }
        FilesPreviewStatus::Unsupported => {
            let message = app
                .files_preview_error
                .as_deref()
                .unwrap_or("Preview is not supported for this item.");
            frame.render_widget(
                Paragraph::new(message)
                    .style(Theme::muted_text())
                    .wrap(Wrap { trim: true }),
                inner,
            );
        }
        FilesPreviewStatus::Error => {
            let message = app
                .files_preview_error
                .as_deref()
                .unwrap_or("Failed to render preview.");
            frame.render_widget(
                Paragraph::new(message)
                    .style(Style::default().fg(Theme::error()))
                    .wrap(Wrap { trim: true }),
                inner,
            );
        }
        FilesPreviewStatus::Empty => {
            frame.render_widget(
                Paragraph::new("Move focus to a file row to preview images and PDFs.")
                    .style(Theme::muted_text())
                    .wrap(Wrap { trim: true }),
                inner,
            );
        }
        FilesPreviewStatus::Hidden => {
            frame.render_widget(
                Paragraph::new("Press v to toggle preview.")
                    .style(Theme::muted_text())
                    .wrap(Wrap { trim: true }),
                inner,
            );
        }
    }
}

fn centered_preview_area(app: &AppState, area: Rect) -> Rect {
    let Some(meta) = app.files_preview_source_meta.as_ref() else {
        return area;
    };
    if meta.width == 0 || meta.height == 0 || area.width == 0 || area.height == 0 {
        return area;
    }

    let (cell_w, cell_h) = app
        .files_preview_picker
        .as_ref()
        .map(|picker| picker.font_size())
        .unwrap_or((10, 20));
    if cell_w == 0 || cell_h == 0 {
        return area;
    }

    let source_w = u64::from(meta.width);
    let source_h = u64::from(meta.height);
    let avail_w_px = u64::from(area.width) * u64::from(cell_w);
    let avail_h_px = u64::from(area.height) * u64::from(cell_h);
    let max_w_px = avail_w_px.min(source_w);
    let max_h_px = avail_h_px.min(source_h);
    if max_w_px == 0 || max_h_px == 0 {
        return area;
    }

    let scale_w = max_w_px as f64 / source_w as f64;
    let scale_h = max_h_px as f64 / source_h as f64;
    let scale = scale_w.min(scale_h);

    let fitted_w_px = ((source_w as f64 * scale).round() as u64).max(1);
    let fitted_h_px = ((source_h as f64 * scale).round() as u64).max(1);

    let fitted_w = ((fitted_w_px + u64::from(cell_w) - 1) / u64::from(cell_w))
        .min(u64::from(area.width))
        .max(1) as u16;
    let fitted_h = ((fitted_h_px + u64::from(cell_h) - 1) / u64::from(cell_h))
        .min(u64::from(area.height))
        .max(1) as u16;

    let offset_x = area.width.saturating_sub(fitted_w) / 2;
    let offset_y = area.height.saturating_sub(fitted_h) / 2;

    Rect::new(area.x + offset_x, area.y + offset_y, fitted_w, fitted_h)
}

fn draw_scope_files(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    if area.height < 1 {
        return;
    }

    let list_area = if app.files_settings_open {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);
        draw_files_settings_popup(frame, app, sections[0]);
        sections[1]
    } else {
        area
    };

    app.clear_scope_files_scrollbar();

    let visible = app.scope_files_rows();
    if visible.is_empty() {
        let text = if app.show_selected_categories_only && app.selected_extensions.is_empty() {
            "No categories selected".to_string()
        } else if app.show_selected_categories_only {
            "No files match currently selected categories".to_string()
        } else {
            app.scan_error
                .as_ref()
                .map(|error| format!("Could not read directory tree: {error}"))
                .unwrap_or_else(|| "No files found in current directory".to_string())
        };
        frame.render_widget(
            Paragraph::new(text)
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            list_area,
        );
        app.tree.scroll = 0;
        app.tree.cursor = 0;
        return;
    }

    let mut rows_area = list_area;
    let mut list_height = rows_area.height.saturating_sub(1) as usize;
    if list_height == 0 || rows_area.width == 0 {
        return;
    }

    let needs_scrollbar = visible.len() > list_height;
    let show_scrollbar = needs_scrollbar && rows_area.width > 1;
    if show_scrollbar {
        rows_area.width = rows_area.width.saturating_sub(1);
        list_height = rows_area.height.saturating_sub(1) as usize;
    }
    if list_height == 0 || rows_area.width == 0 {
        return;
    }

    if app.tree.cursor < app.tree.scroll {
        app.tree.scroll = app.tree.cursor;
    } else if app.tree.cursor >= app.tree.scroll + list_height && list_height > 0 {
        app.tree.scroll = app.tree.cursor + 1 - list_height;
    }

    let mut items = Vec::new();
    for (idx, row) in visible
        .iter()
        .enumerate()
        .skip(app.tree.scroll)
        .take(list_height)
    {
        let node = &app.tree.nodes[row.node_id];
        let mut indent = "  ".repeat(row.depth as usize);
        let _ = indent.pop();
        let indent_width = indent.chars().count() as u16;
        let icon = node_icon(node);
        let mark = if app.node_selected_for_display(row.node_id) {
            "[x]"
        } else {
            "[ ]"
        };
        let mut line = vec![
            Span::raw(indent),
            Span::raw(mark),
            Span::raw(" "),
            if node.is_dir {
                Span::styled(icon, Theme::accent_text())
            } else {
                Span::raw(icon)
            },
            Span::raw(" "),
        ];
        append_name_with_muted_extension(&mut line, node);
        if node.unreadable {
            line.push(Span::raw(" !"));
        }

        let style = if idx == app.tree.cursor || app.scope_files_focus_nodes.contains(&row.node_id)
        {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(Line::from(line)).style(style));

        let y = rows_area.y + (idx - app.tree.scroll) as u16;
        let checkbox_start = indent_width.min(rows_area.width);
        let checkbox_width = if checkbox_start < rows_area.width {
            4u16.min(rows_area.width - checkbox_start)
        } else {
            0
        };

        if checkbox_start > 0 {
            app.click_regions.register(
                Rect::new(rows_area.x, y, checkbox_start, 1),
                ClickTarget::ScopeFileRow(idx),
            );
        }
        if checkbox_width > 0 {
            app.click_regions.register(
                Rect::new(rows_area.x + checkbox_start, y, checkbox_width, 1),
                ClickTarget::ScopeFileCheckbox(idx),
            );
        }
        let after_checkbox = checkbox_start.saturating_add(checkbox_width);
        if after_checkbox < rows_area.width {
            app.click_regions.register(
                Rect::new(
                    rows_area.x + after_checkbox,
                    y,
                    rows_area.width - after_checkbox,
                    1,
                ),
                ClickTarget::ScopeFileRow(idx),
            );
        }
    }

    frame.render_widget(List::new(items), rows_area);

    if show_scrollbar {
        let track_area = Rect::new(
            rows_area.x + rows_area.width,
            rows_area.y,
            1,
            list_height as u16,
        );
        app.set_scope_files_scrollbar_geometry(track_area, visible.len(), list_height);
        if let Some(geometry) = app.scope_files_scrollbar {
            let mut scrollbar_state = ScrollbarState::new(geometry.content_len)
                .position(app.tree.scroll)
                .viewport_content_length(geometry.viewport_len);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_style(Theme::muted_text())
                .thumb_style(Theme::accent_text());
            frame.render_stateful_widget(scrollbar, geometry.track_area, &mut scrollbar_state);

            app.click_regions
                .register(geometry.thumb_area, ClickTarget::ScopeFilesScrollbarThumb);
            app.click_regions
                .register(geometry.track_area, ClickTarget::ScopeFilesScrollbarTrack);
        }
    }
}

fn draw_files_settings_popup(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Settings ");
    frame.render_widget(block, area);
    let inner = inner_rect(area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let mark = if app.show_selected_categories_only {
        "[x]"
    } else {
        "[ ]"
    };
    let row = format!("{mark} Show selected categories only [f]");
    frame.render_widget(Paragraph::new(row).style(Theme::panel()), inner);
    app.click_regions.register(
        Rect::new(inner.x, inner.y, inner.width, 1),
        ClickTarget::ScopeFilesSettingShowSelectedCategoriesOnly,
    );
}

fn append_name_with_muted_extension(line: &mut Vec<Span<'_>>, node: &crate::model::FileNode) {
    if node.is_dir {
        line.push(Span::raw(node.name.clone()));
        return;
    }

    if let Some(dot_pos) = node.name.rfind('.')
        && dot_pos > 0
    {
        let (base, ext) = node.name.split_at(dot_pos);
        line.push(Span::raw(base.to_string()));
        line.push(Span::styled(ext.to_string(), Theme::muted_text()));
        return;
    }

    line.push(Span::raw(node.name.clone()));
}

fn draw_scope_select(frame: &mut Frame<'_>, app: &mut AppState, area: Rect, focused: bool) {
    frame.render_widget(focus_block("Category", focused), area);
    let inner = inner_rect(area);
    if inner.width < 6 || inner.height < 3 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let left_width = (sections[0].width as usize * 36 / 100).clamp(8, sections[0].width as usize);
    let gap = 2usize;
    let right_width = sections[0]
        .width
        .saturating_sub(left_width as u16)
        .saturating_sub(gap as u16) as usize;

    let header = Line::from(vec![
        Span::styled(fit_to_width("Category", left_width), Theme::accent_text()),
        Span::raw("  "),
        Span::styled(
            fit_to_width("Extensions", right_width),
            Theme::accent_text(),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), sections[0]);

    let mut items = Vec::new();
    for (idx, category) in app.category_filters.iter().enumerate() {
        let is_active = category
            .extensions
            .iter()
            .all(|ext| app.selected_extensions.contains(ext));
        let extensions = category.extensions.join(",");
        let category_name = fit_to_width(&category.name, left_width);
        let category_span = if app.show_selected_categories_only && is_active {
            Span::styled(
                category_name,
                Style::default()
                    .fg(Theme::success())
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw(category_name)
        };
        let line = Line::from(vec![
            category_span,
            Span::raw("  "),
            Span::styled(
                fit_to_width(&extensions, right_width),
                if is_active {
                    Theme::accent_text()
                } else {
                    Theme::muted_text()
                },
            ),
        ]);
        let style = if idx == app.select_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(line).style(style));

        let y = sections[1].y + idx as u16;
        if y < sections[1].y + sections[1].height {
            app.click_regions.register(
                Rect::new(sections[1].x, y, sections[1].width, 1),
                ClickTarget::ScopeCategoryRow(idx),
            );
        }
    }

    frame.render_widget(List::new(items), sections[1]);
}

fn fit_to_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let text_len = text.chars().count();
    if text_len <= width {
        return format!("{text:<width$}");
    }

    if width == 1 {
        return "…".to_string();
    }

    let mut out = text.chars().take(width - 1).collect::<String>();
    out.push('…');
    out
}

fn node_icon(node: &crate::model::FileNode) -> &'static str {
    if node.is_dir {
        return if node.expanded {
            "\u{f07c}"
        } else {
            "\u{f07b}"
        };
    }

    let ext = node
        .path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        // Photos
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "heic" | "heif"
        | "svg" => "\u{f03e}",
        // PDFs
        "pdf" => "\u{f1c1}",
        // Audio
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "opus" => "\u{f001}",
        // Video
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" | "mpg" | "mpeg" => "\u{f03d}",
        // Generic file fallback
        _ => "\u{f15b}",
    }
}

fn draw_scope_constraint(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let mut items = Vec::new();
    let mut index = 0usize;

    for item in crate::model::TimeConstraint::ALL {
        let selected = app.time_constraint == item;
        let mark = if selected { "●" } else { "○" };
        let style = if index == app.constraint_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(format!("{mark} {}", item.title())).style(style));
        let y = area.y + index as u16;
        if y < area.y + area.height {
            app.click_regions.register(
                Rect::new(area.x, y, area.width, 1),
                ClickTarget::ScopeConstraintRow(index),
            );
        }
        index += 1;
    }

    items.push(ListItem::new(""));
    index += 1;

    for item in crate::model::SizeConstraint::ALL {
        let selected = app.size_constraint == item;
        let mark = if selected { "●" } else { "○" };
        let style = if index - 1 == app.constraint_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(format!("{mark} {}", item.title())).style(style));

        let y = area.y + (index - 1) as u16;
        if y < area.y + area.height {
            app.click_regions.register(
                Rect::new(area.x, y, area.width, 1),
                ClickTarget::ScopeConstraintRow(index - 1),
            );
        }
        index += 1;
    }

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Constraint options")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn draw_scope_preset(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let mut items = Vec::new();
    for idx in 0..9usize {
        let has_value = app.presets[idx].is_some();
        let status = if has_value { "saved" } else { "empty" };
        let style = if idx == app.preset_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(format!("{}: {status}", idx + 1)).style(style));

        let y = area.y + idx as u16;
        if y < area.y + area.height {
            app.click_regions.register(
                Rect::new(area.x, y, area.width, 1),
                ClickTarget::ScopePresetRow(idx),
            );
        }
    }

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Presets (Ctrl+1..9 save | 1..9 load)")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn draw_scope_marketplace(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let mut items = Vec::new();
    for (idx, preset) in app.marketplace_presets.iter().enumerate() {
        let style = if idx == app.marketplace_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        let line = format!("{} - {}", preset.name, preset.description);
        items.push(ListItem::new(line).style(style));

        let y = area.y + idx as u16;
        if y < area.y + area.height {
            app.click_regions.register(
                Rect::new(area.x, y, area.width, 1),
                ClickTarget::ScopeMarketplaceRow(idx),
            );
        }
    }

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Marketplace (Enter or click to apply)")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn draw_naming(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let chunks = if area.width >= 110 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(3)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(12), Constraint::Length(3)])
            .split(area)
    };

    let top = if chunks[0].width >= 110 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(chunks[0])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(chunks[0])
    };

    draw_naming_table(frame, app, top[0]);
    let right_area = if app.files_preview_visible {
        let right_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(top[1]);
        draw_naming_right(frame, app, right_split[0]);
        draw_file_preview_pane(frame, app, right_split[1]);
        right_split[0]
    } else {
        draw_naming_right(frame, app, top[1]);
        top[1]
    };
    draw_command_box(frame, app, chunks[1]);
    app.click_regions
        .register(top[0], ClickTarget::NamingPane(FocusPane::NamingTable));
    app.click_regions
        .register(right_area, ClickTarget::NamingPane(FocusPane::NamingRight));
    app.click_regions
        .register(chunks[1], ClickTarget::NamingPane(FocusPane::NamingCommand));
}

fn draw_naming_table(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let block = focus_block("Rename Preview", app.focus == FocusPane::NamingTable);
    frame.render_widget(block, area);
    let inner = inner_rect(area);

    if app.rename_rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No files selected yet. Use Scope > Files.")
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let list_height = inner.height as usize;
    if app.rename_cursor < app.rename_scroll {
        app.rename_scroll = app.rename_cursor;
    } else if app.rename_cursor >= app.rename_scroll + list_height && list_height > 0 {
        app.rename_scroll = app.rename_cursor + 1 - list_height;
    }

    let mut items = Vec::new();
    for (idx, row) in app
        .rename_rows
        .iter()
        .enumerate()
        .skip(app.rename_scroll)
        .take(list_height)
    {
        let mark = if row.selected { "[x]" } else { "[ ]" };
        let conflict = if row.conflict { " ⚠" } else { "" };
        let row_option_index = app
            .suggestion_set
            .as_ref()
            .and_then(|set| {
                if set.per_path_options.contains_key(&row.path) {
                    Some(
                        set.per_path_override_index
                            .get(&row.path)
                            .copied()
                            .unwrap_or(set.global_option_index)
                            .min(2),
                    )
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let option_suffix = format!(
            " [1{}][2{}][3{}]",
            if row_option_index == 0 { "*" } else { "" },
            if row_option_index == 1 { "*" } else { "" },
            if row_option_index == 2 { "*" } else { "" }
        );
        let show_placeholder = should_show_pending_proposed_name(app, row);
        let proposed_part = if show_placeholder {
            "...".to_string()
        } else {
            format!("{}{}{}", row.proposed_name, option_suffix, conflict)
        };
        let style = if idx == app.rename_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        let is_analyzing_this_row = app.naming_ai_status == NamingAiStatus::AnalyzingFiles
            && app.ai_analyzing_paths.contains(&row.path);
        let current_name_style = if is_analyzing_this_row {
            style.fg(analysis_cycle_color(app.ticks, idx))
        } else {
            style
        };
        let show_processing_star = show_placeholder
            && matches!(
                app.naming_ai_status,
                NamingAiStatus::AnalyzingFiles | NamingAiStatus::Generating
            );
        let transition_span = if show_processing_star {
            Span::styled(
                format!(" {} ", pending_twinkle_star(app.ticks, idx)),
                style
                    .fg(analysis_cycle_color(app.ticks, idx))
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" -> ", style)
        };
        let line = Line::from(vec![
            Span::styled(format!("{mark} G{} ", row.group_id), style),
            Span::styled(row.current_name.clone(), current_name_style),
            transition_span,
            Span::styled(proposed_part, style),
        ]);
        items.push(ListItem::new(line));

        let y = inner.y + (idx - app.rename_scroll) as u16;
        app.click_regions.register(
            Rect::new(inner.x, y, inner.width, 1),
            ClickTarget::NamingRow(idx),
        );
        if inner.width >= 14 {
            let start = inner.x + inner.width.saturating_sub(14);
            app.click_regions.register(
                Rect::new(start, y, 4, 1),
                ClickTarget::NamingRowSuggestionOption {
                    row_index: idx,
                    option_index: 0,
                },
            );
            app.click_regions.register(
                Rect::new(start + 4, y, 4, 1),
                ClickTarget::NamingRowSuggestionOption {
                    row_index: idx,
                    option_index: 1,
                },
            );
            app.click_regions.register(
                Rect::new(start + 8, y, 4, 1),
                ClickTarget::NamingRowSuggestionOption {
                    row_index: idx,
                    option_index: 2,
                },
            );
        }
    }

    frame.render_widget(List::new(items), inner);
}

fn draw_naming_right(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    if area.height < 3 || area.width < 6 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    frame.render_widget(
        Block::default()
            .title("Suggestions")
            .borders(Borders::ALL)
            .border_style(
                if app.focus == FocusPane::NamingRight && app.naming_tab == NamingTab::Suggestions {
                    Theme::accent_text()
                } else {
                    Theme::muted_text()
                },
            ),
        sections[0],
    );
    let sugg_inner = inner_rect(sections[0]);
    let settings_label = "\u{f013} [m] [r]";
    if sections[0].width > settings_label.chars().count() as u16 + 2 {
        let icon_x = sections[0]
            .x
            .saturating_add(sections[0].width)
            .saturating_sub(settings_label.chars().count() as u16 + 1);
        let icon_area = Rect::new(
            icon_x,
            sections[0].y,
            settings_label.chars().count() as u16,
            1,
        );
        let line = Line::from(vec![
            Span::styled("\u{f013}", Theme::accent_text()),
            Span::raw(" [m] [r]"),
        ]);
        frame.render_widget(Paragraph::new(line), icon_area);
        let m_area = Rect::new(icon_x, sections[0].y, 5, 1);
        let r_area = Rect::new(icon_x + 6, sections[0].y, 4, 1);
        app.click_regions
            .register(m_area, ClickTarget::NamingSuggestionsSettingsButton);
        app.click_regions
            .register(r_area, ClickTarget::NamingSuggestionsRefreshButton);
    }

    let list_area = if app.ai_settings.popup_open {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(sugg_inner);
        draw_naming_ai_settings_popup(frame, app, split[0]);
        split[1]
    } else {
        sugg_inner
    };
    let mut suggestion_items = Vec::new();
    let status_line = format!(
        "Status: {}",
        match app.naming_ai_status {
            NamingAiStatus::Idle => "Idle",
            NamingAiStatus::NeedsConfig => "Needs config (m)",
            NamingAiStatus::DiscoveringModels => "Discovering models...",
            NamingAiStatus::AnalyzingFiles => "Analyzing files...",
            NamingAiStatus::Generating => "Generating suggestions...",
            NamingAiStatus::Ready => "Ready",
            NamingAiStatus::Error => "Error",
        }
    );
    suggestion_items.push(ListItem::new(status_line).style(Theme::muted_text()));
    if let Some(set) = &app.suggestion_set {
        suggestion_items.push(
            ListItem::new(format!(
                "Model: {} (req #{})",
                set.source_model, set.request_id
            ))
            .style(Theme::muted_text()),
        );
    } else if let Some(model) = &app.ai_settings.selected_model {
        suggestion_items.push(ListItem::new(format!("Model: {model}")).style(Theme::muted_text()));
    }
    for (idx, item) in app.suggestions.iter().enumerate() {
        let marker = if app.suggestion_set.as_ref().map(|s| s.global_option_index) == Some(idx) {
            "*"
        } else {
            " "
        };
        let style = if app.naming_tab == NamingTab::Suggestions && idx == app.suggestion_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        suggestion_items.push(ListItem::new(format!("[{}] {item}", marker)).style(style));

        let y = list_area.y + (idx + 2) as u16;
        if y < list_area.y + list_area.height {
            app.click_regions.register(
                Rect::new(list_area.x, y, list_area.width, 1),
                ClickTarget::NamingSuggestion(idx),
            );
        }
    }
    if let Some(error) = &app.naming_ai_error {
        suggestion_items.push(
            ListItem::new(format!("Error: {error}")).style(Style::default().fg(Theme::warning())),
        );
    }
    frame.render_widget(List::new(suggestion_items), list_area);

    frame.render_widget(
        Block::default()
            .title("Style Controls")
            .borders(Borders::ALL)
            .border_style(
                if app.focus == FocusPane::NamingRight && app.naming_tab == NamingTab::Style {
                    Theme::accent_text()
                } else {
                    Theme::muted_text()
                },
            ),
        sections[1],
    );
    let style_inner = inner_rect(sections[1]);
    let style_rows = vec![
        format!("Length: {}", format_length(app.style.length)),
        format!("Capitalization: {}", format_caps(app.style.capitalization)),
        format!("Separator: {}", format_separator(app.style.separator)),
        format!(
            "Keep extension: {}",
            if app.style.keep_extension {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "Strip colons: {}",
            if app.style.strip_colons { "yes" } else { "no" }
        ),
    ];

    let mut style_items = Vec::new();
    for (idx, line) in style_rows.into_iter().enumerate() {
        let style = if app.naming_tab == NamingTab::Style && idx == app.style_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        style_items.push(ListItem::new(line).style(style));

        let y = style_inner.y + idx as u16;
        if y < style_inner.y + style_inner.height {
            app.click_regions.register(
                Rect::new(style_inner.x, y, style_inner.width, 1),
                ClickTarget::NamingStyleRow(idx),
            );
        }
    }
    frame.render_widget(List::new(style_items), style_inner);
}

fn analysis_cycle_color(ticks: u64, row_index: usize) -> Color {
    // Faster color cycle for active processing rows.
    let t = (ticks as f32 * 0.22) + (row_index as f32 * 0.35);
    let pulse = (t.sin() + 1.0) * 0.5;
    // Light blue/cyan sweep for in-progress analysis rows.
    let hue = 188.0 + (pulse * 18.0);
    let sat = 0.82 - (pulse * 0.10);
    let light = 0.62 + (pulse * 0.10);
    hsl_to_rgb(hue, sat, light)
}

fn pending_twinkle_star(ticks: u64, row_index: usize) -> char {
    let twinkle_star = ['✦', '✧'];
    // Slower glyph sweep than color cycling.
    let phase = ((ticks / 12) + row_index as u64) % (twinkle_star.len() as u64);
    twinkle_star[phase as usize]
}

fn should_show_pending_proposed_name(app: &AppState, row: &RenameRow) -> bool {
    let has_override = row
        .override_name
        .as_ref()
        .map(|name| !name.trim().is_empty())
        .unwrap_or(false);
    if has_override {
        return false;
    }

    app.suggestion_set
        .as_ref()
        .map(|set| !set.per_path_options.contains_key(&row.path))
        .unwrap_or(true)
}

fn hsl_to_rgb(hue_deg: f32, sat: f32, light: f32) -> Color {
    let hue = hue_deg.rem_euclid(360.0);
    let sat = sat.clamp(0.0, 1.0);
    let light = light.clamp(0.0, 1.0);

    let c = (1.0 - (2.0 * light - 1.0).abs()) * sat;
    let h_prime = hue / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());

    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = light - c / 2.0;

    Color::Rgb(
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn draw_naming_ai_settings_popup(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    frame.render_widget(
        Block::default()
            .title(" AI Settings ")
            .borders(Borders::ALL),
        area,
    );
    let inner = inner_rect(area);
    if inner.height == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    let api_label = if app.ai_settings.reveal_api_key {
        app.ai_settings.api_key_input.clone()
    } else {
        mask_secret(&app.ai_settings.api_key_input)
    };
    let api_style = if app.ai_settings.active_field == AiSettingsField::ApiKey {
        Theme::accent_text()
    } else {
        Theme::panel()
    };
    frame.render_widget(
        Paragraph::new(format!("API key: {api_label}")).style(api_style),
        rows[0],
    );
    app.click_regions
        .register(rows[0], ClickTarget::NamingAiSettingsApiKeyField);

    let reveal_line = format!(
        "[{}] Reveal key",
        if app.ai_settings.reveal_api_key {
            "x"
        } else {
            " "
        }
    );
    frame.render_widget(
        Paragraph::new(reveal_line).style(Theme::muted_text()),
        rows[1],
    );
    app.click_regions
        .register(rows[1], ClickTarget::NamingAiSettingsToggleReveal);

    let search_style = if app.ai_settings.active_field == AiSettingsField::ModelSearch {
        Theme::accent_text()
    } else {
        Theme::panel()
    };
    frame.render_widget(
        Paragraph::new(format!("Search: {}", app.ai_settings.model_search_query))
            .style(search_style),
        rows[2],
    );
    app.click_regions
        .register(rows[2], ClickTarget::NamingAiSettingsModelSearchField);

    let manual_style = if app.ai_settings.active_field == AiSettingsField::ManualModel {
        Theme::accent_text()
    } else {
        Theme::panel()
    };
    frame.render_widget(
        Paragraph::new(format!("Model: {}", app.ai_settings.manual_model_input))
            .style(manual_style),
        rows[3],
    );
    app.click_regions
        .register(rows[3], ClickTarget::NamingAiSettingsManualModelField);

    let list_area = rows[4];
    let filtered = app.filtered_ai_models();
    let mut items = Vec::new();
    items.push(ListItem::new(
        "[d] Discover models   [s] Save   [c] Clear key",
    ));
    for (idx, (_source_idx, model)) in filtered
        .iter()
        .enumerate()
        .take((list_area.height as usize).saturating_sub(1))
    {
        let selected = idx == app.ai_settings.model_list_cursor;
        let style = if selected {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        let ctx = model
            .context_length
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".to_string());
        let price = match (&model.pricing_prompt, &model.pricing_completion) {
            (Some(p), Some(c)) => format!(" p:{p} c:{c}"),
            _ => String::new(),
        };
        items.push(
            ListItem::new(format!(
                "{} ({}) ctx:{}{}",
                model.name, model.id, ctx, price
            ))
            .style(style),
        );
        let y = list_area.y + idx as u16 + 1;
        app.click_regions.register(
            Rect::new(list_area.x, y, list_area.width, 1),
            ClickTarget::NamingAiSettingsModelRow(idx),
        );
    }
    frame.render_widget(List::new(items), list_area);
    app.click_regions.register(
        Rect::new(list_area.x, list_area.y, 12, 1),
        ClickTarget::NamingAiSettingsDiscoverModels,
    );
    app.click_regions.register(
        Rect::new(list_area.x + 15, list_area.y, 8, 1),
        ClickTarget::NamingAiSettingsSaveConfig,
    );
    app.click_regions.register(
        Rect::new(list_area.x + 26, list_area.y, 12, 1),
        ClickTarget::NamingAiSettingsClearApiKey,
    );
}

fn draw_command_box(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let block = focus_block("Natural prompt (/)", app.focus == FocusPane::NamingCommand);
    frame.render_widget(block, area);
    let inner = inner_rect(area);

    let placeholder = "Example: Make names concise, title case, include project + date";
    let mut content = if app.command_input.is_empty() {
        Span::styled(placeholder, Theme::muted_text())
    } else {
        Span::styled(app.command_input.as_str(), Theme::panel())
    };

    if app.mode == InputMode::Command {
        content = Span::styled(format!("{}▌", app.command_input), Theme::accent_text());
    }

    frame.render_widget(Paragraph::new(Line::from(content)), inner);
}

fn draw_apply(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let split = if area.width >= 110 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area)
    };

    draw_apply_review(frame, app, split[0]);
    draw_apply_history(frame, app, split[1]);
    app.click_regions
        .register(split[0], ClickTarget::ApplyPane(FocusPane::ApplyReview));
    app.click_regions
        .register(split[1], ClickTarget::ApplyPane(FocusPane::ApplyHistory));
}

fn draw_apply_review(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    frame.render_widget(
        focus_block(
            "Ready to Apply (simulated)",
            app.focus == FocusPane::ApplyReview,
        ),
        area,
    );

    let inner = inner_rect(area);
    let selected_rows: Vec<_> = app.rename_rows.iter().filter(|row| row.selected).collect();
    if selected_rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No selected rows. Select rows in Naming first.")
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let mut items = Vec::new();
    for (idx, row) in selected_rows.iter().enumerate() {
        let style = if idx == app.apply_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        let line = format!("{} -> {}", row.current_name, row.proposed_name);
        items.push(ListItem::new(line).style(style));
        let y = inner.y + idx as u16;
        if y < inner.y + inner.height {
            app.click_regions.register(
                Rect::new(inner.x, y, inner.width, 1),
                ClickTarget::ApplyRow(idx),
            );
        }
    }

    frame.render_widget(List::new(items), inner);
}

fn draw_apply_history(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    frame.render_widget(
        focus_block("Session Undo History", app.focus == FocusPane::ApplyHistory),
        area,
    );
    let inner = inner_rect(area);
    if app.undo_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No simulated apply operations yet.")
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let mut items = Vec::new();
    for (idx, entry) in app.undo_history.iter().enumerate() {
        let style = if idx == app.history_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        let sample = entry
            .previous_names
            .first()
            .zip(entry.simulated_new_names.first())
            .map(|(old, new)| format!(" | {old} -> {new}"))
            .unwrap_or_default();
        let line = format!(
            "{}: {} file(s){}",
            entry.timestamp,
            entry.affected_paths.len(),
            sample
        );
        items.push(ListItem::new(line).style(style));

        let y = inner.y + idx as u16;
        if y < inner.y + inner.height {
            app.click_regions.register(
                Rect::new(inner.x, y, inner.width, 1),
                ClickTarget::ApplyHistoryRow(idx),
            );
        }
    }

    frame.render_widget(List::new(items), inner);
}

fn draw_footer(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let hints = match app.stage {
        Stage::Scope => {
            "Scope: arrows/space, left/right expand, ctrl+arrows deep expand, f filter, v preview, t subtab"
        }
        Stage::Naming => {
            "Naming: r refresh, m settings, p prompt history, / prompt, v preview, t subtab"
        }
        Stage::Apply => "Apply: Enter submit+exit | Ctrl+Enter submit+stay (simulated)",
    };
    frame.render_widget(Paragraph::new(hints).style(Theme::muted_text()), area);
}

fn draw_toast(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let mut lines = Vec::new();
    for toast in app.toasts.iter().rev().take(2) {
        let style = match toast.level {
            ToastLevel::Info => Theme::muted_text(),
            ToastLevel::Success => Style::default().fg(Theme::success()),
            ToastLevel::Warning => Style::default().fg(Theme::warning()),
            ToastLevel::Error => Style::default().fg(Theme::error()),
        };
        lines.push(Line::from(Span::styled(toast.message.as_str(), style)));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_help_overlay(frame: &mut Frame<'_>, app: &AppState) {
    let area = center(frame.area(), 78, 58);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(app.help_lines.clone())
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_style(Theme::accent_text()),
            )
            .style(Theme::panel())
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_override_overlay(frame: &mut Frame<'_>, app: &AppState) {
    let area = center(frame.area(), 72, 24);
    frame.render_widget(Clear, area);
    let line = format!("{}▌", app.override_input);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Override filename"),
            Line::from(""),
            Line::from(Span::styled(line, Theme::accent_text())),
            Line::from(""),
            Line::from(Span::styled(
                "Enter: save, Esc: cancel, Tab: autocomplete",
                Theme::muted_text(),
            )),
        ])
        .block(
            Block::default()
                .title("Edit Override")
                .borders(Borders::ALL)
                .border_style(Theme::accent_text()),
        )
        .style(Theme::panel())
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_new_category_overlay(frame: &mut Frame<'_>, app: &AppState) {
    let area = center(frame.area(), 70, 26);
    frame.render_widget(Clear, area);
    let line = format!("{}▌", app.new_category_input);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Create category as name:ext1,ext2"),
            Line::from("Example: Research:pdf,md"),
            Line::from(""),
            Line::from(Span::styled(line, Theme::accent_text())),
        ])
        .block(
            Block::default()
                .title("New Category")
                .borders(Borders::ALL)
                .border_style(Theme::accent_text()),
        )
        .style(Theme::panel())
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_log_overlay(frame: &mut Frame<'_>, app: &mut AppState) {
    let area = center(frame.area(), 88, 72);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title("Logs (L to close)")
        .borders(Borders::ALL)
        .border_style(Theme::accent_text());
    frame.render_widget(block, area);
    let inner = inner_rect(area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    app.click_regions.register(inner, ClickTarget::LogOverlay);
    app.update_log_viewport(inner.height as usize, inner.width as usize);

    if app.logs.is_empty() {
        frame.render_widget(
            Paragraph::new("No logs yet.")
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let visible = app.log_view_height.max(1);
    let max_start = app.logs.len().saturating_sub(visible);
    let start = app.log_scroll.min(max_start);
    let end = (start + visible).min(app.logs.len());
    let col_start = app.log_col_scroll;
    let mut items = Vec::new();
    for (offset, line) in app.logs.iter().skip(start).take(end - start).enumerate() {
        let log_index = start + offset;
        let y = inner.y + offset as u16;
        let decorated = format!("\u{f0c5} {}", line);
        let visible_line = slice_char_start(&decorated, col_start).to_string();
        items.push(ListItem::new(visible_line).style(Theme::panel()));
        if col_start == 0 && inner.width > 0 {
            let icon_width = inner.width.min(2);
            app.click_regions.register(
                Rect::new(inner.x, y, icon_width, 1),
                ClickTarget::LogCopyLine(log_index),
            );
        }
    }
    frame.render_widget(List::new(items), inner);
}

fn draw_prompt_history_overlay(frame: &mut Frame<'_>, app: &mut AppState) {
    let area = center(frame.area(), 82, 64);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title("Prompt History (p to close, Enter to load)")
        .borders(Borders::ALL)
        .border_style(Theme::accent_text());
    frame.render_widget(block, area);

    let inner = inner_rect(area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    app.prompt_history_set_view_height(inner.height as usize);

    if app.prompt_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No prompt history yet. Submit prompts in Naming to store them.")
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let mut items = Vec::new();
    for (display_idx, prompt) in app.visible_prompt_history_items() {
        let style = if display_idx == app.prompt_history_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(prompt).style(style));
    }
    frame.render_widget(List::new(items), inner);
}

fn slice_char_start(line: &str, start: usize) -> &str {
    if start == 0 {
        return line;
    }
    if let Some((byte_idx, _)) = line.char_indices().nth(start) {
        &line[byte_idx..]
    } else {
        ""
    }
}

fn format_length(length: NameLength) -> &'static str {
    match length {
        NameLength::None => "none",
        NameLength::Long => "long",
        NameLength::Medium => "medium",
        NameLength::Short => "short",
    }
}

fn format_caps(caps: Capitalization) -> &'static str {
    match caps {
        Capitalization::None => "none",
        Capitalization::Lower => "lower",
        Capitalization::Title => "title",
        Capitalization::Upper => "upper",
    }
}

fn format_separator(separator: Separator) -> &'static str {
    match separator {
        Separator::Space => "space",
        Separator::Dash => "dash",
        Separator::Underscore => "underscore",
    }
}

fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "<empty>".to_string();
    }
    "•".repeat(value.chars().count().min(32))
}

fn focus_block<'a>(title: &'a str, focused: bool) -> Block<'a> {
    let border_style = if focused {
        Theme::accent_text()
    } else {
        Theme::muted_text()
    };

    Block::default()
        .title(Span::styled(
            title,
            if focused {
                Theme::accent_text()
            } else {
                Theme::muted_text()
            },
        ))
        .borders(Borders::ALL)
        .border_style(border_style)
}

fn inner_rect(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

fn center(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let width = area.width.saturating_mul(width_pct) / 100;
    let height = area.height.saturating_mul(height_pct) / 100;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.max(1), height.max(1))
}

fn segment_rects(area: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }

    let chunk_width = (area.width / count as u16).max(1);
    (0..count)
        .map(|idx| {
            let x = area.x + (idx as u16 * chunk_width);
            let width = if idx == count - 1 {
                area.x + area.width - x
            } else {
                chunk_width
            };
            Rect::new(x, area.y, width, area.height.max(1))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn install_flat_file_tree(app: &mut AppState, file_count: usize) {
        let mut nodes = Vec::with_capacity(file_count + 1);
        let child_ids: Vec<usize> = (1..=file_count).collect();
        nodes.push(crate::model::FileNode {
            parent: None,
            path: PathBuf::from("."),
            name: ".".to_string(),
            is_dir: true,
            children: child_ids.clone(),
            children_loaded: true,
            expanded: true,
            selected: false,
            unreadable: false,
        });
        for idx in 0..file_count {
            nodes.push(crate::model::FileNode {
                parent: Some(0),
                path: PathBuf::from(format!("./file-{idx}.txt")),
                name: format!("file-{idx}.txt"),
                is_dir: false,
                children: Vec::new(),
                children_loaded: false,
                expanded: false,
                selected: false,
                unreadable: false,
            });
        }
        app.tree = crate::model::FileTree {
            nodes,
            root: 0,
            cursor: 0,
            scroll: 0,
        };
        app.normalize_scope_files_cursor();
    }

    #[test]
    fn render_scope_screen_smoke() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = AppState::new(std::env::current_dir().expect("cwd"));
        app.stage = Stage::Scope;
        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("draw should work");
    }

    #[test]
    fn render_naming_screen_smoke() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = AppState::new(std::env::current_dir().expect("cwd"));
        app.stage = Stage::Naming;
        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("draw should work");
    }

    #[test]
    fn render_compact_screen_smoke() {
        let backend = TestBackend::new(85, 26);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = AppState::new(std::env::current_dir().expect("cwd"));
        app.stage = Stage::Apply;
        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("draw should work");
    }

    #[test]
    fn render_scope_files_registers_scrollbar_regions_on_overflow() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = AppState::new(std::env::current_dir().expect("cwd"));
        app.stage = Stage::Scope;
        install_flat_file_tree(&mut app, 300);

        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("draw should work");

        assert!(
            app.click_regions
                .regions()
                .iter()
                .any(|region| matches!(region.data, ClickTarget::ScopeFilesScrollbarTrack))
        );
        assert!(
            app.click_regions
                .regions()
                .iter()
                .any(|region| matches!(region.data, ClickTarget::ScopeFilesScrollbarThumb))
        );
    }

    #[test]
    fn render_scope_files_omits_scrollbar_regions_without_overflow() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = AppState::new(std::env::current_dir().expect("cwd"));
        app.stage = Stage::Scope;
        install_flat_file_tree(&mut app, 5);

        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("draw should work");

        assert!(
            !app.click_regions
                .regions()
                .iter()
                .any(|region| matches!(region.data, ClickTarget::ScopeFilesScrollbarTrack))
        );
        assert!(
            !app.click_regions
                .regions()
                .iter()
                .any(|region| matches!(region.data, ClickTarget::ScopeFilesScrollbarThumb))
        );
    }
}
