use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::{
    model::{
        AppState, Capitalization, ClickTarget, FocusPane, InputMode, NameLength, NamingTab,
        ScopeTab, Separator, Stage, ToastLevel,
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
    frame.render_widget(tabs, area);

    for (idx, rect) in segment_rects(area, Stage::ALL.len())
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
            let titles: Vec<Line<'_>> = NamingTab::ALL
                .iter()
                .map(|tab| Line::from(tab.title()))
                .collect();
            let selected = NamingTab::ALL
                .iter()
                .position(|tab| *tab == app.naming_tab)
                .unwrap_or(0);
            frame.render_widget(
                Tabs::new(titles)
                    .style(Theme::muted_text())
                    .highlight_style(Theme::accent_text())
                    .select(selected)
                    .divider("  "),
                area,
            );
            for (idx, rect) in segment_rects(area, NamingTab::ALL.len())
                .into_iter()
                .enumerate()
            {
                app.click_regions
                    .register(rect, ClickTarget::NamingTab(NamingTab::ALL[idx]));
            }
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
    draw_scope_right(frame, app, split[1]);
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
        draw_scope_select(frame, app, sections[0], app.focus == FocusPane::ScopeCategory);
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

    app.click_regions
        .register(sections[0], ClickTarget::ScopePane(FocusPane::ScopeCategory));
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

    let list_height = list_area.height.saturating_sub(1) as usize;
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
        let indent = "  ".repeat(row.depth as usize);
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

        let style = if idx == app.tree.cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(Line::from(line)).style(style));

        let y = list_area.y + (idx - app.tree.scroll) as u16;
        app.click_regions.register(
            Rect::new(list_area.x, y, list_area.width, 1),
            ClickTarget::ScopeFileRow(idx),
        );
    }

    frame.render_widget(List::new(items), list_area);
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
    let row = format!("{mark} Show selected categories only [v]");
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
        Span::styled(
            fit_to_width("Category", left_width),
            Theme::accent_text(),
        ),
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
        let line = Line::from(vec![
            Span::raw(fit_to_width(&category.name, left_width)),
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

    frame.render_widget(
        List::new(items),
        sections[1],
    );
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
        return if node.expanded { "\u{f07c}" } else { "\u{f07b}" };
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
    draw_naming_right(frame, app, top[1]);
    draw_command_box(frame, app, chunks[1]);
    app.click_regions
        .register(top[0], ClickTarget::NamingPane(FocusPane::NamingTable));
    app.click_regions
        .register(top[1], ClickTarget::NamingPane(FocusPane::NamingRight));
    app.click_regions
        .register(chunks[1], ClickTarget::NamingPane(FocusPane::NamingCommand));
}

fn draw_naming_table(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let block = focus_block("Current -> Proposed", app.focus == FocusPane::NamingTable);
    frame.render_widget(block, area);
    let inner = inner_rect(area);

    if app.rename_rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No files selected/matched yet. Use Scope > Files or filters.")
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
        let line = format!(
            "{mark} G{} {} -> {}{conflict}",
            row.group_id, row.current_name, row.proposed_name
        );
        let style = if idx == app.rename_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(line).style(style));

        let y = inner.y + (idx - app.rename_scroll) as u16;
        app.click_regions.register(
            Rect::new(inner.x, y, inner.width, 1),
            ClickTarget::NamingRow(idx),
        );
    }

    frame.render_widget(List::new(items), inner);
}

fn draw_naming_right(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let title = match app.naming_tab {
        NamingTab::Suggestions => "Suggestions",
        NamingTab::Style => "Style Controls",
    };
    let block = focus_block(title, app.focus == FocusPane::NamingRight);
    frame.render_widget(block, area);
    let inner = inner_rect(area);

    match app.naming_tab {
        NamingTab::Suggestions => {
            let mut items = Vec::new();
            for (idx, item) in app.suggestions.iter().enumerate() {
                let style = if idx == app.suggestion_cursor {
                    Theme::selected_row()
                } else {
                    Theme::panel()
                };
                items.push(ListItem::new(item.label.clone()).style(style));

                let y = inner.y + idx as u16;
                if y < inner.y + inner.height {
                    app.click_regions.register(
                        Rect::new(inner.x, y, inner.width, 1),
                        ClickTarget::NamingSuggestion(idx),
                    );
                }
            }
            frame.render_widget(List::new(items), inner);
        }
        NamingTab::Style => {
            let rows = vec![
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

            let mut items = Vec::new();
            for (idx, line) in rows.into_iter().enumerate() {
                let style = if idx == app.style_cursor {
                    Theme::selected_row()
                } else {
                    Theme::panel()
                };
                items.push(ListItem::new(line).style(style));

                let y = inner.y + idx as u16;
                if y < inner.y + inner.height {
                    app.click_regions.register(
                        Rect::new(inner.x, y, inner.width, 1),
                        ClickTarget::NamingStyleRow(idx),
                    );
                }
            }

            frame.render_widget(List::new(items), inner);
        }
    }
}

fn draw_command_box(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let block = focus_block(
        "Natural style command (/)",
        app.focus == FocusPane::NamingCommand,
    );
    frame.render_widget(block, area);
    let inner = inner_rect(area);

    let placeholder = "Example: short title dash no-colon keep-ext";
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
        Stage::Scope => "Scope: arrows/space, left/right expand, ctrl+arrows deep expand, t subtab",
        Stage::Naming => {
            "Naming: space select row, e edit, g assign group, / style command, t subtab"
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

fn format_length(length: NameLength) -> &'static str {
    match length {
        NameLength::Long => "long",
        NameLength::Medium => "medium",
        NameLength::Short => "short",
    }
}

fn format_caps(caps: Capitalization) -> &'static str {
    match caps {
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
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

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
}
