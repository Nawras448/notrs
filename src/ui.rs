use chrono::DateTime;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Screen};
use crate::md;
use crate::storage;

const SURFACE: Color = Color::Rgb(36, 37, 58);
const HIGHLIGHT: Color = Color::Rgb(59, 66, 97);
const TEXT: Color = Color::Rgb(192, 202, 245);
const SUBTEXT: Color = Color::Rgb(86, 95, 137);
const ACCENT: Color = Color::Rgb(1, 120, 212);
const ERROR: Color = Color::Rgb(247, 118, 142);
const H2: Color = Color::Rgb(101, 197, 120);
const LIST: Color = Color::Rgb(169, 130, 255);

fn styled_line(chars: &[char], cursor: Option<usize>, style: Style) -> Line<'static> {
    if let Some(cur) = cursor {
        if cur >= chars.len() {
            let mut spans: Vec<Span> = chars
                .iter()
                .map(|&c| Span::styled(c.to_string(), style))
                .collect();
            spans.push(Span::styled(" ", Style::new().bg(TEXT).fg(SURFACE)));
            Line::from(spans)
        } else {
            let before: String = chars[..cur].iter().collect();
            let at = chars[cur];
            let after: String = chars[cur + 1..].iter().collect();
            Line::from(vec![
                Span::styled(before, style),
                Span::styled(at.to_string(), Style::new().bg(TEXT).fg(SURFACE)),
                Span::styled(after, style),
            ])
        }
    } else {
        Line::from(
            chars
                .iter()
                .map(|&c| Span::styled(c.to_string(), style))
                .collect::<Vec<_>>(),
        )
    }
}

fn line_style(line: &[char]) -> Style {
    match md::classify(line) {
        md::BlockKind::Heading(1) => Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
        md::BlockKind::Heading(_) => Style::new().fg(H2).add_modifier(Modifier::BOLD),
        md::BlockKind::Quote => Style::new().fg(SUBTEXT).add_modifier(Modifier::ITALIC),
        md::BlockKind::Divider => Style::new().fg(SUBTEXT),
        md::BlockKind::Bullet | md::BlockKind::Checkbox(_) | md::BlockKind::Ordered => {
            Style::new().fg(LIST)
        }
        _ => Style::new().fg(TEXT),
    }
}

fn format_date(rfc: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(rfc) {
        dt.format("%Y-%m-%d").to_string()
    } else {
        rfc[..10].to_string()
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::vertical([
        Constraint::Length((r.height.saturating_sub(percent_y)) / 2),
        Constraint::Length(percent_y),
        Constraint::Length((r.height.saturating_sub(percent_y)) / 2),
    ]);
    let horz = Layout::horizontal([
        Constraint::Length((r.width.saturating_sub(percent_x)) / 2),
        Constraint::Length(percent_x),
        Constraint::Length((r.width.saturating_sub(percent_x)) / 2),
    ]);
    horz.split(vert.split(r)[1])[1]
}

fn render_list(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);
    let rows = layout.split(area);
    let header_area = rows[0];
    let search_area = rows[1];
    let list_area = rows[2];
    let message_area = rows[3];
    let footer_area = rows[4];

    let header = Paragraph::new("Notes")
        .style(Style::new().fg(TEXT).bg(Color::Rgb(36, 47, 56)))
        .bold()
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::new().fg(Color::Rgb(73, 82, 89))),
        )
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(header, header_area);

    let search_text = if app.search_query.is_empty() {
        "Search...".to_string()
    } else {
        app.search_query.clone()
    };
    let search_style = if app.search_query.is_empty() {
        Style::new().fg(SUBTEXT).bg(SURFACE)
    } else {
        Style::new().fg(TEXT).bg(SURFACE)
    };
    let search = Paragraph::new(search_text)
        .style(search_style)
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(ACCENT)),
        );
    frame.render_widget(search, search_area);

    if app.filtered_notes.is_empty() {
        let msg = if app.search_query.is_empty() {
            "No notes yet. Press 'n' to create one."
        } else {
            "No notes match your search."
        };
        let empty = Paragraph::new(Text::from(msg))
            .style(Style::new().fg(SUBTEXT))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty, list_area);
    } else {
        let items: Vec<ListItem> = app
            .filtered_notes
            .iter()
            .map(|note| {
                let date = format_date(&note.updated_at);
                ListItem::new(Line::from(vec![
                    Span::styled(&note.title, Style::new().bold().fg(TEXT)),
                    Span::raw("  "),
                    Span::styled(date, Style::new().fg(SUBTEXT)),
                ]))
            })
            .collect();
        let list = List::new(items)
            .block(Block::bordered().border_style(Style::new().fg(Color::Rgb(59, 66, 97))))
            .highlight_style(Style::new().bg(HIGHLIGHT).fg(TEXT))
            .highlight_symbol("> ");
        frame.render_stateful_widget(
            list,
            list_area,
            &mut ratatui::widgets::ListState::default().with_selected(Some(app.selected_index)),
        );
    }

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [q] Quit ", Style::new().fg(ACCENT)),
        Span::styled(" [/] Search ", Style::new().fg(SUBTEXT)),
        Span::styled(" [n] New ", Style::new().fg(ACCENT)),
        Span::styled(" [Enter] Edit ", Style::new().fg(SUBTEXT)),
        Span::styled(" [d] Delete ", Style::new().fg(ERROR)),
        Span::styled(" [Ctrl+R] Sort ", Style::new().fg(ACCENT)),
        Span::styled(" [Ctrl+S] Settings ", Style::new().fg(ACCENT)),
    ]))
    .style(Style::new().bg(Color::Rgb(36, 47, 56)));
    frame.render_widget(footer, footer_area);

    if let Some((text, color)) = match (&app.error_message, &app.status_message) {
        (Some(e), _) => Some((e.clone(), ERROR)),
        (None, Some(s)) => Some((s.clone(), SUBTEXT)),
        _ => None,
    } {
        let message = Paragraph::new(text)
            .style(Style::new().fg(color).bg(Color::Rgb(36, 47, 56)));
        frame.render_widget(message, message_area);
    }
}

fn render_editor(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ]);
    let rows = layout.split(area);
    let header_area = rows[0];
    let title_area = rows[1];
    let content_area = rows[2];
    let footer_area = rows[3];

    let mode = match &app.screen {
        Screen::Editor(Some(_)) => "Edit Note",
        _ => "New Note",
    };
    let header = Paragraph::new(mode)
        .style(Style::new().fg(TEXT).bg(Color::Rgb(36, 47, 56)))
        .bold()
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::new().fg(Color::Rgb(73, 82, 89))),
        );
    frame.render_widget(header, header_area);

    // --- Title ---

    let title_style = if app.editor.focus_title {
        Style::new().fg(TEXT).bg(SURFACE)
    } else {
        Style::new().fg(SUBTEXT).bg(SURFACE)
    };
    let title_border = if app.editor.focus_title {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" Title ")
            .border_style(Style::new().fg(ACCENT))
    } else {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" Title ")
            .border_style(Style::new().fg(Color::Rgb(59, 66, 97)))
    };
    let mut visual_cursor_x: Option<usize> = None;
    let title_inner_width = title_area.width.saturating_sub(2) as usize;
    let title_text = if app.editor.title.is_empty() {
        if app.editor.focus_title {
            visual_cursor_x = Some("Note title...".chars().count());
            Text::from(Line::from(vec![
                Span::raw("Note title..."),
                Span::styled(" ", Style::new().bg(TEXT).fg(SURFACE)),
            ]))
        } else {
            Text::from("Note title...")
        }
    } else {
        let cursor = if app.editor.focus_title {
            Some(app.editor.title_cursor)
        } else {
            None
        };
        let visual = md::line_to_visual(&app.editor.title, cursor, None, title_inner_width);
        if app.editor.focus_title {
            visual_cursor_x = visual.cursor;
        }
        let fg = if app.editor.focus_title { TEXT } else { SUBTEXT };
        Text::from(styled_line(&visual.chars, visual.cursor, Style::new().fg(fg)))
    };
    let title_widget = Paragraph::new(title_text)
        .style(title_style)
        .block(title_border);
    frame.render_widget(title_widget, title_area);

    // --- Content ---

    let content_border = if !app.editor.focus_title {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" Content ")
            .border_style(Style::new().fg(ACCENT))
    } else {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" Content ")
            .border_style(Style::new().fg(Color::Rgb(59, 66, 97)))
    };
    let content_style = if !app.editor.focus_title {
        Style::new().fg(TEXT).bg(SURFACE)
    } else {
        Style::new().fg(SUBTEXT).bg(SURFACE)
    };
    let content_inner_width = content_area.width.saturating_sub(2) as usize;
    let content_text = if !app.editor.focus_title {
        let mut out_lines: Vec<Line> = Vec::with_capacity(app.editor.lines.len());
        let mut i = 0;
        while i < app.editor.lines.len() {
            if md::is_table_row(&app.editor.lines[i]) {
                let mut end = i;
                while end + 1 < app.editor.lines.len()
                    && md::is_table_row(&app.editor.lines[end + 1])
                {
                    end += 1;
                }
                let block: Vec<&[char]> = app.editor.lines[i..=end]
                    .iter()
                    .map(|v| v.as_slice())
                    .collect();
                let widths = md::table_col_widths(&block);
                for (k, rl) in app.editor.lines[i..=end].iter().enumerate() {
                    let is_cur = i + k == app.editor.cursor_line;
                    let cursor = if is_cur {
                        Some(app.editor.cursor_col)
                    } else {
                        None
                    };
                    let visual = md::table_visual(rl, &widths, cursor);
                    if is_cur {
                        visual_cursor_x = visual.cursor;
                    }
                    let style = if md::is_separator_row(rl) {
                        Style::new().fg(SUBTEXT)
                    } else {
                        Style::new().fg(TEXT)
                    };
                    out_lines.push(styled_line(&visual.chars, visual.cursor, style));
                }
                i = end + 1;
                continue;
            }
            let is_cur = i == app.editor.cursor_line;
            let cursor = if is_cur {
                Some(app.editor.cursor_col)
            } else {
                None
            };
            let visual = md::line_to_visual(&app.editor.lines[i], cursor, None, content_inner_width);
            if is_cur {
                visual_cursor_x = visual.cursor;
            }
            out_lines.push(styled_line(&visual.chars, visual.cursor, line_style(&app.editor.lines[i])));
            i += 1;
        }
        Text::from(out_lines)
    } else {
        Text::from(
            app.editor.lines
                .iter()
                .map(|line| line.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    };
    let content_widget = Paragraph::new(content_text)
        .style(content_style)
        .block(content_border)
        .wrap(Wrap { trim: false });
    frame.render_widget(content_widget, content_area);

    // --- Cursor & scroll ---

    let visible_lines = (content_area.height.saturating_sub(2)) as usize;
    if app.editor.cursor_line < app.editor.scroll_offset {
        app.editor.scroll_offset = app.editor.cursor_line;
    }
    if visible_lines > 0 && app.editor.cursor_line >= app.editor.scroll_offset + visible_lines {
        app.editor.scroll_offset = app.editor.cursor_line.saturating_add(1).saturating_sub(visible_lines);
    }

    if app.editor.focus_title {
        let cursor_x = title_area.x + 1 + visual_cursor_x.unwrap_or(app.editor.title_cursor) as u16;
        let cursor_y = title_area.y + 1;
        if cursor_x < title_area.x + title_area.width - 1 {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    } else {
        let cursor_screen_line = app.editor.cursor_line.saturating_sub(app.editor.scroll_offset);
        let cursor_x = content_area.x + 1 + visual_cursor_x.unwrap_or(app.editor.cursor_col) as u16;
        let cursor_y = content_area.y + 1 + cursor_screen_line as u16;
        if cursor_screen_line < visible_lines
            && cursor_x < content_area.x + content_area.width - 1
        {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    // --- Footer ---

    let footer = if matches!(&app.screen, Screen::Editor(Some(_))) {
        Paragraph::new(Line::from(vec![
            Span::styled(" [Ctrl+S] Save ", Style::new().fg(ACCENT)),
            Span::styled(" [Esc] Cancel ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Ctrl+D] Delete ", Style::new().fg(ERROR)),
            Span::styled(" [Ctrl+1-6] Heading ", Style::new().fg(ACCENT)),
            Span::styled(" [Ctrl+U] Bullet ", Style::new().fg(ACCENT)),
            Span::styled(" [Ctrl+O] Numbered ", Style::new().fg(ACCENT)),
            Span::styled(" [Ctrl+T] Table ", Style::new().fg(ACCENT)),
            Span::styled(" [Alt+↑/↓] Move ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Tab] Switch ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Mouse] Click ", Style::new().fg(SUBTEXT)),
        ]))
        .style(Style::new().bg(Color::Rgb(36, 47, 56)))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(" [Ctrl+S] Save ", Style::new().fg(ACCENT)),
            Span::styled(" [Esc] Cancel ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Ctrl+1-6] Heading ", Style::new().fg(ACCENT)),
            Span::styled(" [Ctrl+U] Bullet ", Style::new().fg(ACCENT)),
            Span::styled(" [Ctrl+O] Numbered ", Style::new().fg(ACCENT)),
            Span::styled(" [Ctrl+T] Table ", Style::new().fg(ACCENT)),
            Span::styled(" [Tab] Switch ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Mouse] Click ", Style::new().fg(SUBTEXT)),
        ]))
        .style(Style::new().bg(Color::Rgb(36, 47, 56)))
    };
    frame.render_widget(footer, footer_area);
}

fn render_settings(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ]);
    let rows = layout.split(area);
    let header_area = rows[0];
    let path_area = rows[1];
    let hint_area = rows[2];
    let footer_area = rows[4];

    let header = Paragraph::new("Settings")
        .style(Style::new().fg(TEXT).bg(Color::Rgb(36, 47, 56)))
        .bold()
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::new().fg(Color::Rgb(73, 82, 89))),
        );
    frame.render_widget(header, header_area);

    let path_text = if app.settings.input.is_empty() {
        Text::from(Line::from(vec![
            Span::raw("(current folder) notes.json"),
            Span::styled(" ", Style::new().bg(TEXT).fg(SURFACE)),
        ]))
    } else {
        let before: String = app.settings.input[..app.settings.cursor].iter().collect();
        let at = app.settings.input.get(app.settings.cursor);
        let after: String = app
            .settings.input
            .get(app.settings.cursor + 1..)
            .unwrap_or(&[])
            .iter()
            .collect();
        let mut spans = vec![Span::raw(before)];
        match at {
            Some(c) => {
                spans.push(Span::styled(c.to_string(), Style::new().bg(TEXT).fg(SURFACE)));
            }
            None => {
                spans.push(Span::styled(" ", Style::new().bg(TEXT).fg(SURFACE)));
            }
        }
        spans.push(Span::raw(after));
        Text::from(Line::from(spans))
    };
    let path_widget = Paragraph::new(path_text)
        .style(Style::new().fg(TEXT).bg(SURFACE))
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(" Notes file ")
                .border_style(Style::new().fg(ACCENT)),
        );
    frame.render_widget(path_widget, path_area);

    let current = storage::display_notes_path();
    let hint_lines = vec![
        Line::from(vec![Span::styled(format!(" Current: {current}"), Style::new().fg(SUBTEXT))]),
        if let Some(err) = &app.settings.error {
            Line::from(vec![Span::styled(format!(" {err}"), Style::new().fg(ERROR))])
        } else {
            Line::from(vec![Span::styled(
                " Full file path (e.g. ~/notes.json). A folder gets notes.json added inside.",
                Style::new().fg(SUBTEXT),
            )])
        },
    ];
    let hint = Paragraph::new(Text::from(hint_lines)).style(Style::new().bg(SURFACE));
    frame.render_widget(hint, hint_area);

    let cursor_x = path_area.x + 1 + app.settings.cursor as u16;
    let cursor_y = path_area.y + 1;
    if cursor_x < path_area.x + path_area.width - 1 {
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [Ctrl+S] Save ", Style::new().fg(ACCENT)),
        Span::styled(" [Enter] Save ", Style::new().fg(SUBTEXT)),
        Span::styled(" [Esc] Cancel ", Style::new().fg(SUBTEXT)),
    ]))
    .style(Style::new().bg(Color::Rgb(36, 47, 56)));
    frame.render_widget(footer, footer_area);
}

fn render_confirm(frame: &mut Frame, app: &App) {
    render_list(frame, app);

    let title = match &app.screen {
        Screen::ConfirmDelete(t) => t.clone(),
        _ => String::new(),
    };

    let dialog_area = centered_rect(50, 6, frame.area());
    let dialog = Paragraph::new(Text::from(vec![
        Line::from(vec![Span::styled(
            format!(" Delete '{}'? ", title),
            Style::new().bold().fg(TEXT),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Y] Yes  ", Style::new().fg(ERROR).bold()),
            Span::styled("  [N] No  ", Style::new().fg(SUBTEXT)),
        ]),
    ]))
    .style(Style::new().bg(Color::Rgb(30, 31, 48)))
    .alignment(ratatui::layout::Alignment::Center)
    .block(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(ERROR)),
    );
    frame.render_widget(dialog, dialog_area);
}

pub fn render(frame: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::List => render_list(frame, app),
        Screen::Editor(_) => render_editor(frame, app),
        Screen::ConfirmDelete(_) => render_confirm(frame, app),
        Screen::Settings => render_settings(frame, app),
    }
}
