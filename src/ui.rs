use chrono::DateTime;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Screen};

const SURFACE: Color = Color::Rgb(36, 37, 58);
const HIGHLIGHT: Color = Color::Rgb(59, 66, 97);
const TEXT: Color = Color::Rgb(192, 202, 245);
const SUBTEXT: Color = Color::Rgb(86, 95, 137);
const ACCENT: Color = Color::Rgb(1, 120, 212);
const ERROR: Color = Color::Rgb(247, 118, 142);

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
    ]);
    let rows = layout.split(area);
    let header_area = rows[0];
    let search_area = rows[1];
    let list_area = rows[2];
    let footer_area = rows[3];

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
    ]))
    .style(Style::new().bg(Color::Rgb(36, 47, 56)));
    frame.render_widget(footer, footer_area);
}

fn render_editor(frame: &mut Frame, app: &App) {
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

    let title_style = if app.editor_focus_title {
        Style::new().fg(TEXT).bg(SURFACE)
    } else {
        Style::new().fg(SUBTEXT).bg(SURFACE)
    };
    let title_border = if app.editor_focus_title {
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
    let title_display = if app.editor_title.is_empty() {
        "Note title...".to_string()
    } else {
        app.editor_title.clone()
    };
    let title_widget = Paragraph::new(title_display)
        .style(title_style)
        .block(title_border);
    frame.render_widget(title_widget, title_area);

    let content_border = if !app.editor_focus_title {
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
    let content_style = if !app.editor_focus_title {
        Style::new().fg(TEXT).bg(SURFACE)
    } else {
        Style::new().fg(SUBTEXT).bg(SURFACE)
    };
    let content_widget = Paragraph::new(app.editor_content.as_str())
        .style(content_style)
        .block(content_border)
        .wrap(Wrap { trim: false });
    frame.render_widget(content_widget, content_area);

    let footer = if matches!(&app.screen, Screen::Editor(Some(_))) {
        Paragraph::new(Line::from(vec![
            Span::styled(" [Ctrl+S] Save ", Style::new().fg(ACCENT)),
            Span::styled(" [Esc] Cancel ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Ctrl+D] Delete ", Style::new().fg(ERROR)),
            Span::styled(" [Tab] Switch field ", Style::new().fg(SUBTEXT)),
        ]))
        .style(Style::new().bg(Color::Rgb(36, 47, 56)))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled(" [Ctrl+S] Save ", Style::new().fg(ACCENT)),
            Span::styled(" [Esc] Cancel ", Style::new().fg(SUBTEXT)),
            Span::styled(" [Tab] Switch field ", Style::new().fg(SUBTEXT)),
        ]))
        .style(Style::new().bg(Color::Rgb(36, 47, 56)))
    };
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

pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::List => render_list(frame, app),
        Screen::Editor(_) => render_editor(frame, app),
        Screen::ConfirmDelete(_) => render_confirm(frame, app),
    }
}
