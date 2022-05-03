/**
 * Copyright 2019-2022, Benjamin Vaisvil and the zenith contributors
 */
use super::{split_left_right_pane, FileSystemDisplay, Render, ZBackend};
use crate::float_to_byte_string;
use crate::histogram::{HistogramKind, View};
use crate::metrics::*;
use byte_unit::{Byte, ByteUnit};
use std::borrow::Cow;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline};
use tui::Frame;

pub fn render_disk(
    app: &CPUTimeApp,
    layout: Rect,
    f: &mut Frame<'_, ZBackend>,
    view: View,
    border_style: Style,
    file_system_index: &usize,
    file_system_display: &FileSystemDisplay,
) {
    let (disk_layout, view) = split_left_right_pane("Disk", layout, f, view, border_style);
    let area = Layout::default()
        .margin(1)
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(disk_layout[1]);

    if *file_system_display == FileSystemDisplay::Activity {
        disk_activity_histogram(app, f, view, &area);
    } else {
        disk_usage(app, f, view, &area, file_system_index);
    }

    let disks: Vec<_> = app
        .disks
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let style = if d.get_perc_free_space() < 10.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            };
            if *file_system_index == i {
                Span::styled(
                    Cow::Owned(format!(
                        "→{:3.0}%: {}",
                        d.get_perc_free_space(),
                        d.mount_point.display()
                    )),
                    style,
                )
            } else {
                Span::styled(
                    Cow::Owned(format!(
                        " {:3.0}%: {}",
                        d.get_perc_free_space(),
                        d.mount_point.display()
                    )),
                    style,
                )
            }
        })
        .map(ListItem::new)
        .collect();
    List::new(disks)
        .block(
            Block::default()
                .title(Span::styled(
                    "File Systems [(a)ctivity/usage]",
                    border_style,
                ))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .render(f, disk_layout[0]);
}
fn disk_activity_histogram(
    app: &CPUTimeApp,
    f: &mut Frame<'_, ZBackend>,
    view: View,
    area: &[Rect],
) {
    let read_up = float_to_byte_string!(app.disk_read as f64, ByteUnit::B);
    let h_read = match app.histogram_map.get_zoomed(&HistogramKind::IoRead, &view) {
        Some(h) => h,
        None => return,
    };

    let read_max: u64 = match h_read.data().iter().max() {
        Some(x) => *x,
        None => 1,
    };
    let read_max_bytes = float_to_byte_string!(read_max as f64, ByteUnit::B);

    let top_reader = match app.top_disk_reader_pid {
        Some(pid) => match app.process_map.get(&pid) {
            Some(p) => format!("[{:} - {:} - {:}]", p.pid, p.name, p.user_name),
            None => String::from(""),
        },
        None => String::from(""),
    };

    let write_down = float_to_byte_string!(app.disk_write as f64, ByteUnit::B);
    let h_write = match app.histogram_map.get_zoomed(&HistogramKind::IoWrite, &view) {
        Some(h) => h,
        None => return,
    };

    let write_max: u64 = match h_write.data().iter().max() {
        Some(x) => *x,
        None => 1,
    };
    let write_max_bytes = float_to_byte_string!(write_max as f64, ByteUnit::B);

    let top_writer = match app.top_disk_writer_pid {
        Some(pid) => match app.process_map.get(&pid) {
            Some(p) => format!("[{:} - {:} - {:}]", p.pid, p.name, p.user_name),
            None => String::from(""),
        },
        None => String::from(""),
    };
    Sparkline::default()
        .block(
            Block::default().title(
                format!(
                    "R [{:^10}/s] Max [{:^10}/s] {:}",
                    read_up, read_max_bytes, top_reader
                )
                .as_str(),
            ),
        )
        .data(h_read.data())
        .style(Style::default().fg(Color::LightYellow))
        .max(read_max)
        .render(f, area[0]);

    Sparkline::default()
        .block(
            Block::default().title(
                format!(
                    "W [{:^10}/s] Max [{:^10}/s] {:}",
                    write_down, write_max_bytes, top_writer
                )
                .as_str(),
            ),
        )
        .data(h_write.data())
        .style(Style::default().fg(Color::LightMagenta))
        .max(write_max)
        .render(f, area[1]);
}

fn disk_usage(
    app: &CPUTimeApp,
    f: &mut Frame<'_, ZBackend>,
    view: View,
    area: &[Rect],
    file_system_index: &usize,
) {
    if let Some(fs) = app.disks.get(*file_system_index) {
        let h_used = match app
            .histogram_map
            .get_zoomed(&HistogramKind::FileSystemUsedSpace(fs.name.clone()), &view)
        {
            Some(h) => h,
            None => return,
        };
        let free = float_to_byte_string!(fs.available_bytes as f64, ByteUnit::B);
        let used = float_to_byte_string!(fs.get_used_bytes() as f64, ByteUnit::B);
        let size = float_to_byte_string!(fs.size_bytes as f64, ByteUnit::B);
        Sparkline::default()
            .block(
                Block::default().title(
                    format!(
                        "{}  ↓Used [{:^10} ({:.1}%)] Free [{:^10} ({:.1}%)] Size [{:^10}]",
                        fs.name,
                        used,
                        fs.get_perc_used_space(),
                        free,
                        fs.get_perc_free_space(),
                        size
                    )
                    .as_str(),
                ),
            )
            .data(h_used.data())
            .style(Style::default().fg(Color::LightYellow))
            .max(fs.size_bytes)
            .render(f, area[0]);
        let columns = Layout::default()
            .margin(1)
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(area[1]);
        let rhs_style = Style::default().fg(Color::Green);
        let text = vec![
            Spans::from(vec![
                Span::raw("Name:                  ".to_string()),
                Span::styled(fs.name.to_string(), rhs_style),
            ]),
            Spans::from(vec![
                Span::raw("File System            ".to_string()),
                Span::styled(fs.file_system.to_string(), rhs_style),
            ]),
            Spans::from(vec![
                Span::raw("Mount Point:           ".to_string()),
                Span::styled(fs.mount_point.to_string_lossy(), rhs_style),
            ]),
        ];
        Paragraph::new(text).render(f, columns[0]);
        let text = vec![
            Spans::from(vec![
                Span::raw("Size:                  ".to_string()),
                Span::styled(size, rhs_style),
            ]),
            Spans::from(vec![
                Span::raw("Used                   ".to_string()),
                Span::styled(used, rhs_style),
            ]),
            Spans::from(vec![
                Span::raw("Free:                  ".to_string()),
                Span::styled(free, rhs_style),
            ]),
        ];
        Paragraph::new(text).render(f, columns[1]);
    }
}