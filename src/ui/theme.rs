use ratatui::style::{Color, Modifier, Style};

#[allow(dead_code)]
pub struct Theme {
    pub header: Style,
    pub footer: Style,
    pub selected: Style,
    pub dir: Style,
    pub file: Style,
    pub graph: Style,
    pub shortcut: Style,
    pub border: Style,
}

pub fn get_theme(name: &str) -> Theme {
    match name {
        "dark" => Theme {
            header: Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            footer: Style::default().bg(Color::Blue).fg(Color::White),
            selected: Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            dir: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            file: Style::default().fg(Color::White),
            graph: Style::default().fg(Color::Green),
            shortcut: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::Gray),
        },
        "dark-bg" => Theme {
            header: Style::default()
                .bg(Color::Black)
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            footer: Style::default().bg(Color::Black).fg(Color::Cyan),
            selected: Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            dir: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            file: Style::default().fg(Color::Gray),
            graph: Style::default().fg(Color::Cyan),
            shortcut: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::DarkGray),
        },
        _ => {
            // off (default monochrome style)
            Theme {
                header: Style::default().add_modifier(Modifier::REVERSED),
                footer: Style::default().add_modifier(Modifier::REVERSED),
                selected: Style::default().add_modifier(Modifier::REVERSED),
                dir: Style::default().add_modifier(Modifier::BOLD),
                file: Style::default(),
                graph: Style::default(),
                shortcut: Style::default().add_modifier(Modifier::UNDERLINED),
                border: Style::default(),
            }
        }
    }
}
