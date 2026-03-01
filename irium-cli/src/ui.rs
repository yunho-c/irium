use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::{
    model::{
        AppState, Capitalization, FocusPane, InputMode, NameLength, NamingTab, RowHit, ScopeTab,
        Separator, Stage, TabHit, ToastLevel, UiMap,
    },
    theme::Theme,
};

pub fn draw(frame: &mut Frame<'_>, app: &mut AppState) {
    app.ui_map = UiMap::default();

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

    app.ui_map.stage_tabs = segment_tabs(area, Stage::ALL.as_slice());
}

fn draw_subtabs(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    match app.stage {
        Stage::Scope => {
            app.ui_map.scope_tabs.clear();
        }
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
            app.ui_map.naming_tabs = segment_tabs(area, NamingTab::ALL.as_slice());
        }
        Stage::Apply => {
            let text = Paragraph::new("Review simulated changes and session undo history")
                .style(Theme::muted_text());
            frame.render_widget(text, area);
        }
    }
}

fn draw_scope(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    app.ui_map.scope_main = area;
    app.ui_map.scope_tabs.clear();
    app.ui_map.file_rows.clear();
    app.ui_map.select_rows.clear();
    app.ui_map.constraint_rows.clear();
    app.ui_map.preset_rows.clear();
    app.ui_map.marketplace_rows.clear();

    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    draw_scope_left(frame, app, split[0]);
    draw_scope_right(frame, app, split[1]);
}

fn draw_scope_left(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let focused = app.focus == FocusPane::ScopeMain
        && matches!(app.scope_tab, ScopeTab::Files | ScopeTab::Select);
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
        draw_scope_select(frame, app, sections[0]);
    }

    let files_focus = app.focus == FocusPane::ScopeMain && app.scope_tab == ScopeTab::Files;
    frame.render_widget(
        focus_block("Files", files_focus),
        sections[1],
    );
    let files_inner = inner_rect(sections[1]);
    if files_inner.height > 0 && files_inner.width > 0 {
        draw_scope_files(frame, app, files_inner);
    }
}

fn draw_scope_right(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let focused = app.focus == FocusPane::ScopeMain
        && matches!(
            app.scope_tab,
            ScopeTab::Constraint | ScopeTab::Preset | ScopeTab::Marketplace
        );
    frame.render_widget(focus_block("Options", focused), area);
    let inner = inner_rect(area);
    if inner.height < 3 {
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let right_tabs = [ScopeTab::Constraint, ScopeTab::Preset, ScopeTab::Marketplace];
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
    app.ui_map
        .scope_tabs
        .extend(segment_tabs(layout[0], right_tabs.as_slice()));

    match app.scope_right_tab {
        ScopeTab::Constraint => draw_scope_constraint(frame, app, layout[1]),
        ScopeTab::Preset => draw_scope_preset(frame, app, layout[1]),
        ScopeTab::Marketplace => draw_scope_marketplace(frame, app, layout[1]),
        ScopeTab::Files | ScopeTab::Select => draw_scope_constraint(frame, app, layout[1]),
    }
}

fn draw_scope_files(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let visible = app.tree.visible_nodes();
    if visible.is_empty() {
        let text = app
            .scan_error
            .as_ref()
            .map(|error| format!("Could not read directory tree: {error}"))
            .unwrap_or_else(|| "No files found in current directory".to_string());
        frame.render_widget(
            Paragraph::new(text)
                .style(Theme::muted_text())
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let list_height = area.height.saturating_sub(1) as usize;
    if app.tree.cursor < app.tree.scroll {
        app.tree.scroll = app.tree.cursor;
    } else if app.tree.cursor >= app.tree.scroll + list_height && list_height > 0 {
        app.tree.scroll = app.tree.cursor + 1 - list_height;
    }

    let mut items = Vec::new();
    app.ui_map.file_rows.clear();
    for (idx, row) in visible
        .iter()
        .enumerate()
        .skip(app.tree.scroll)
        .take(list_height)
    {
        let node = &app.tree.nodes[row.node_id];
        let indent = "  ".repeat(row.depth as usize);
        let branch = if node.is_dir {
            if node.expanded {
                "▾"
            } else {
                "▸"
            }
        } else {
            "•"
        };
        let mark = if node.selected { "[x]" } else { "[ ]" };
        let unreadable = if node.unreadable { " !" } else { "" };
        let line = format!("{indent}{branch} {mark} {}{unreadable}", node.name);

        let style = if idx == app.tree.cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        items.push(ListItem::new(line).style(style));

        let y = area.y + (idx - app.tree.scroll) as u16;
        app.ui_map.file_rows.push(RowHit {
            rect: Rect::new(area.x, y, area.width, 1),
            index: idx,
        });
    }

    frame.render_widget(List::new(items), area);
}

fn draw_scope_select(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(67), Constraint::Percentage(33)])
        .split(area);

    app.ui_map.select_rows.clear();
    let mut left_items = Vec::new();
    for (idx, category) in app.category_filters.iter().enumerate() {
        let checked = category
            .extensions
            .iter()
            .all(|ext| app.selected_extensions.contains(ext));
        let mark = if checked { "[x]" } else { "[ ]" };
        let preview = category.extensions.join(",");
        let line = format!("{mark} {} ({preview})", category.name);
        let style = if idx == app.select_cursor {
            Theme::selected_row()
        } else {
            Theme::panel()
        };
        left_items.push(ListItem::new(line).style(style));

        let y = cols[0].y + idx as u16;
        if y < cols[0].y + cols[0].height {
            app.ui_map.select_rows.push(RowHit {
                rect: Rect::new(cols[0].x, y, cols[0].width, 1),
                index: idx,
            });
        }
    }

    frame.render_widget(
        List::new(left_items)
            .block(Block::default().title("Category Filters").borders(Borders::ALL)),
        cols[0],
    );

    let summary = if app.selected_extensions.is_empty() {
        "No extension filters active\n(renaming uses tree selection)".to_string()
    } else {
        format!(
            "Active extensions:\n{}",
            app.selected_extensions
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    frame.render_widget(
        Paragraph::new(summary)
            .block(Block::default().title("Active").borders(Borders::ALL))
            .style(Theme::muted_text())
            .wrap(Wrap { trim: true }),
        cols[1],
    );
}

fn draw_scope_constraint(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let mut items = Vec::new();
    app.ui_map.constraint_rows.clear();
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
            app.ui_map.constraint_rows.push(RowHit {
                rect: Rect::new(area.x, y, area.width, 1),
                index,
            });
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
            app.ui_map.constraint_rows.push(RowHit {
                rect: Rect::new(area.x, y, area.width, 1),
                index: index - 1,
            });
        }
        index += 1;
    }

    frame.render_widget(
        List::new(items)
            .block(Block::default().title("Constraint options").borders(Borders::ALL)),
        area,
    );
}

fn draw_scope_preset(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let mut items = Vec::new();
    app.ui_map.preset_rows.clear();
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
            app.ui_map.preset_rows.push(RowHit {
                rect: Rect::new(area.x, y, area.width, 1),
                index: idx,
            });
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
    app.ui_map.marketplace_rows.clear();
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
            app.ui_map.marketplace_rows.push(RowHit {
                rect: Rect::new(area.x, y, area.width, 1),
                index: idx,
            });
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

    app.ui_map.naming_table = top[0];
    app.ui_map.naming_right = top[1];
    app.ui_map.naming_command = chunks[1];

    draw_naming_table(frame, app, top[0]);
    draw_naming_right(frame, app, top[1]);
    draw_command_box(frame, app, chunks[1]);
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
        app.ui_map.rename_rows.clear();
        return;
    }

    let list_height = inner.height as usize;
    if app.rename_cursor < app.rename_scroll {
        app.rename_scroll = app.rename_cursor;
    } else if app.rename_cursor >= app.rename_scroll + list_height && list_height > 0 {
        app.rename_scroll = app.rename_cursor + 1 - list_height;
    }

    let mut items = Vec::new();
    app.ui_map.rename_rows.clear();
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
        app.ui_map.rename_rows.push(RowHit {
            rect: Rect::new(inner.x, y, inner.width, 1),
            index: idx,
        });
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
            app.ui_map.suggestion_rows.clear();
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
                    app.ui_map.suggestion_rows.push(RowHit {
                        rect: Rect::new(inner.x, y, inner.width, 1),
                        index: idx,
                    });
                }
            }
            frame.render_widget(List::new(items), inner);
        }
        NamingTab::Style => {
            app.ui_map.style_rows.clear();
            let rows = vec![
                format!("Length: {}", format_length(app.style.length)),
                format!("Capitalization: {}", format_caps(app.style.capitalization)),
                format!("Separator: {}", format_separator(app.style.separator)),
                format!(
                    "Keep extension: {}",
                    if app.style.keep_extension { "yes" } else { "no" }
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
                    app.ui_map.style_rows.push(RowHit {
                        rect: Rect::new(inner.x, y, inner.width, 1),
                        index: idx,
                    });
                }
            }

            frame.render_widget(List::new(items), inner);
        }
    }
}

fn draw_command_box(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    let block = focus_block("Natural style command (/)", app.focus == FocusPane::NamingCommand);
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

    app.ui_map.apply_review = split[0];
    app.ui_map.apply_history = split[1];

    draw_apply_review(frame, app, split[0]);
    draw_apply_history(frame, app, split[1]);
}

fn draw_apply_review(frame: &mut Frame<'_>, app: &mut AppState, area: Rect) {
    frame.render_widget(
        focus_block("Ready to Apply (simulated)", app.focus == FocusPane::ApplyReview),
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
        app.ui_map.apply_rows.clear();
        return;
    }

    let mut items = Vec::new();
    app.ui_map.apply_rows.clear();
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
            app.ui_map.apply_rows.push(RowHit {
                rect: Rect::new(inner.x, y, inner.width, 1),
                index: idx,
            });
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
        app.ui_map.history_rows.clear();
        return;
    }

    let mut items = Vec::new();
    app.ui_map.history_rows.clear();
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
            app.ui_map.history_rows.push(RowHit {
                rect: Rect::new(inner.x, y, inner.width, 1),
                index: idx,
            });
        }
    }

    frame.render_widget(List::new(items), inner);
}

fn draw_footer(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let hints = match app.stage {
        Stage::Scope => "Scope: arrows/space, left/right expand, ctrl+arrows deep expand, t subtab",
        Stage::Naming => "Naming: space select row, e edit, g assign group, / style command, t subtab",
        Stage::Apply => "Apply: Enter submit+exit | Ctrl+Enter submit+stay (simulated)",
    };
    frame.render_widget(
        Paragraph::new(hints).style(Theme::muted_text()),
        area,
    );
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
            Line::from(Span::styled("Enter: save, Esc: cancel, Tab: autocomplete", Theme::muted_text())),
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
        .title(Span::styled(title, if focused { Theme::accent_text() } else { Theme::muted_text() }))
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

fn segment_tabs<T: Copy>(area: Rect, items: &[T]) -> Vec<TabHit<T>> {
    if items.is_empty() {
        return Vec::new();
    }

    let chunk_width = (area.width / items.len() as u16).max(1);
    items
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            let x = area.x + (idx as u16 * chunk_width);
            let width = if idx == items.len() - 1 {
                area.x + area.width - x
            } else {
                chunk_width
            };
            TabHit::new(Rect::new(x, area.y, width, area.height.max(1)), *value)
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
