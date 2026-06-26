/*
 * ROS Kernel
 *
 * Copyright (c) 2026 Saulo Henrique Santos Dorotéio
 *
 * This file is part of ROS.
 * See the LICENSE file in the project root for licensing information.
 */

use std::fs;
use std::path::Path;
use std::collections::BTreeMap;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};

const MAX_LINE_LENGTH: usize = 100;
const MAX_FUNCTION_LINES: usize = 50;
const MAX_NESTING_DEPTH: usize = 4;
const DUPLICATE_MIN_LINES: usize = 5;

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum Severity { Error, Warning, Info }

#[derive(Clone)]
struct Issue {
    line: usize,
    severity: Severity,
    kind: &'static str,
    message: String,
}

#[derive(Clone)]
struct FileReport {
    path: String,
    issues: Vec<Issue>,
    warnings: usize,
    infos: usize,
}

// ── Scanners ────────────────────────────────────────────────

fn emit(issues: &mut Vec<Issue>, l: usize, s: Severity, k: &'static str, m: String) {
    issues.push(Issue { line: l, severity: s, kind: k, message: m });
}

fn walk_rs_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name == "target" || name == ".git" || name.starts_with('.') { continue; }
                files.extend(walk_rs_files(&path));
            } else if path.extension().map_or(false, |e| e == "rs") {
                files.push(path);
            }
        }
    }
    files
}

fn is_comment_or_attr(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with("#[") || t.starts_with("#!") || t.starts_with('*')
}
fn is_use_line(line: &str) -> bool { line.trim().starts_with("use ") }
fn is_pub_const_or_type(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("pub const ") || t.starts_with("const ") || t.starts_with("pub type ") || t.starts_with("type ")
}

fn scan_file(content: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    trailing_whitespace(content, &mut issues);
    line_length(content, &mut issues);
    eof_newline(content, &mut issues);
    todo_fixme(content, &mut issues);
    unsafe_blocks(content, &mut issues);
    silent_unwrap(content, &mut issues);
    missing_docs(content, &mut issues);
    deep_nesting(content, &mut issues);
    magic_numbers(content, &mut issues);
    long_functions(content, &mut issues);
    issues
}

fn trailing_whitespace(c: &str, issues: &mut Vec<Issue>) {
    for (i, line) in c.lines().enumerate() {
        if line.len() > line.trim_end().len() {
            emit(issues, i + 1, Severity::Warning, "trailing-whitespace", String::new());
        }
    }
}

fn line_length(c: &str, issues: &mut Vec<Issue>) {
    for (i, line) in c.lines().enumerate() {
        let len = line.len();
        if len > MAX_LINE_LENGTH {
            emit(issues, i + 1, Severity::Warning, "line-too-long", format!("{} chars", len));
        }
    }
}

fn eof_newline(c: &str, issues: &mut Vec<Issue>) {
    if !c.ends_with('\n') {
        emit(issues, c.lines().count(), Severity::Warning, "no-eof-newline", String::new());
    }
}

fn todo_fixme(c: &str, issues: &mut Vec<Issue>) {
    for (i, line) in c.lines().enumerate() {
        let t = line.trim();
        if !t.starts_with("//") && !t.starts_with("/*") && !t.starts_with("*") { continue; }
        let lower = t.to_lowercase();
        if lower.contains("todo") || lower.contains("fixme") {
            emit(issues, i + 1, Severity::Info, "todo/fixme", t.to_string());
        }
    }
}

fn unsafe_blocks(c: &str, issues: &mut Vec<Issue>) {
    for (i, line) in c.lines().enumerate() {
        if line.trim() == "unsafe {" || line.trim().starts_with("unsafe {") {
            emit(issues, i + 1, Severity::Info, "unsafe", String::new());
        }
    }
}

fn silent_unwrap(c: &str, issues: &mut Vec<Issue>) {
    for (i, line) in c.lines().enumerate() {
        let t = line.trim();
        if t.contains(".unwrap()") && !t.starts_with("//") {
            emit(issues, i + 1, Severity::Warning, "unwrap", t.to_string());
        }
    }
}

fn missing_docs(c: &str, issues: &mut Vec<Issue>) {
    let lines: Vec<&str> = c.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        let is_pub = t.starts_with("pub fn") || t.starts_with("pub struct")
            || t.starts_with("pub enum") || t.starts_with("pub trait")
            || t.starts_with("pub type") || t.starts_with("pub const")
            || t.starts_with("pub static") || t.starts_with("pub unsafe");
        if is_pub {
            let has_doc = i > 0 && (lines[i - 1].trim().starts_with("///") || lines[i - 1].trim().starts_with("//!"));
            if !has_doc {
                let name = t.split('(').next().unwrap_or(t).trim().to_string();
                emit(issues, i + 1, Severity::Info, "missing-doc", name);
            }
        }
    }
}

fn deep_nesting(c: &str, issues: &mut Vec<Issue>) {
    let mut depth: i32 = 0;
    let mut max_depth: i32 = 0;
    let mut max_line = 0;
    for (i, line) in c.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('}') { depth = depth.saturating_sub(1); }
        for ch in t.chars() {
            match ch { '{' => depth += 1, '}' => depth = depth.saturating_sub(1), _ => {} }
        }
        if depth > max_depth { max_depth = depth; max_line = i + 1; }
    }
    if max_depth > MAX_NESTING_DEPTH as i32 {
        emit(issues, max_line, Severity::Warning, "deep-nesting", format!("depth {}", max_depth));
    }
}

fn magic_numbers(c: &str, issues: &mut Vec<Issue>) {
    for (i, raw) in c.lines().enumerate() {
        let t = raw.trim();
        if is_comment_or_attr(t) || is_use_line(t) || is_pub_const_or_type(t)
            || t.starts_with("pub struct ") || t.starts_with("pub enum ")
            || t.starts_with("struct ") || t.starts_with("enum ")
            || t.starts_with("pub fn ") || t.starts_with("fn ")
            || t.starts_with("pub unsafe fn ") || t.starts_with("unsafe fn ")
            || t.starts_with("impl ") || t.starts_with("pub mod ") || t.starts_with("mod ")
            || t.starts_with("static ") || t.starts_with("pub static ")
            || t.starts_with("let ") || t.starts_with("pub use ")
            || t.starts_with("extern ") || t.starts_with("use ")
        { continue; }
        let s = t.to_string();
        let chars: Vec<char> = s.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j].is_ascii_digit() && j + 1 < chars.len() && chars[j + 1].is_ascii_digit() {
                if j > 0 && (chars[j - 1].is_alphanumeric() || chars[j - 1] == '_' || chars[j - 1] == ':' || chars[j - 1] == '.')
                { j += 1; continue; }
                let start = j;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') { j += 1; }
                let token: String = chars[start..j].iter().collect();
                if token.starts_with("0x") || token.starts_with("0b") || token.starts_with("0o") { continue; }
                let cleaned = token.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '_');
                if let Ok(val) = cleaned.replace('_', "").parse::<i64>() {
                    if val > 1 { emit(issues, i + 1, Severity::Info, "magic-number", t.chars().take(60).collect()); break; }
                }
            } else { j += 1; }
        }
    }
}

fn long_functions(c: &str, issues: &mut Vec<Issue>) {
    let lines: Vec<&str> = c.lines().collect();
    let mut fn_start: Option<(usize, String)> = None;
    let mut brace_depth = 0i32;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if let Some((start, ref name)) = fn_start {
            for ch in t.chars() {
                match ch { '{' => brace_depth += 1, '}' => brace_depth -= 1, _ => {} }
            }
            if brace_depth == 0 {
                let count = i + 1 - start;
                if count > MAX_FUNCTION_LINES {
                    emit(issues, start, Severity::Warning, "long-fn", format!("{} lines: {}", count, name));
                }
                fn_start = None;
            }
        } else if t.starts_with("fn ") || t.starts_with("pub fn ") || t.starts_with("unsafe fn ") || t.starts_with("pub unsafe fn ") {
            let brace = t.find('{');
            let semi = t.find(';');
            if semi.is_some_and(|s| brace.map_or(true, |b| b > s)) { continue; }
            if let Some(_) = brace {
                fn_start = Some((i + 1, t.split('(').next().unwrap_or(t).trim().to_string()));
                brace_depth = 1;
                brace_depth += t.matches('{').count() as i32 - t.matches('}').count() as i32;
                if brace_depth == 0 { fn_start = None; }
            }
        }
    }
}

fn dead_code(all_files: &[std::path::PathBuf], root: &Path) -> Vec<(String, usize, String)> {
    let mut result = Vec::new();
    let mut file_fns: Vec<(String, Vec<(usize, String)>)> = Vec::new();
    let mut all_source = String::new();
    for filepath in all_files {
        let content = match fs::read_to_string(filepath) { Ok(c) => c, _ => continue };
        let rel = filepath.strip_prefix(root).unwrap_or(filepath).display().to_string();
        all_source.push_str(&content); all_source.push('\n');
        file_fns.push((rel, extract_fns(&content)));
    }
    for (file, fns) in &file_fns {
        for &(line, ref name) in fns.iter() {
            if name == "main" { continue; }
            if all_source.matches(&format!("fn {}", name)).count() <= 1 && all_source.matches(&format!("{}(", name)).count() == 0 {
                result.push((file.clone(), line, name.clone()));
            }
        }
    }
    result
}

fn extract_fns(content: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut fns = Vec::new();
    let mut in_trait = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("impl") && t.contains("for") && !t.contains('{') { in_trait = true; continue; }
        if in_trait { if t.starts_with('}') || t.starts_with("impl") { in_trait = false; } continue; }
        if (t.starts_with("fn ") || t.starts_with("pub fn ") || t.starts_with("unsafe fn ") || t.starts_with("pub unsafe fn ")) && t.contains('{') && !t.contains(';') {
            if let Some(name) = t.split('(').next().and_then(|s| s.split_whitespace().last()) {
                if !name.is_empty() && name != "main" { fns.push((i + 1, name.to_string())); }
            }
        }
    }
    fns
}

fn duplicate_code(all_files: &[std::path::PathBuf], root: &Path) -> Vec<(String, usize, String, usize)> {
    let mut result = Vec::new();
    fn is_header(l: &str) -> bool {
        let t = l.trim(); t.is_empty() || t.starts_with("//") || t.starts_with("/*") || t.starts_with("*") || t.starts_with("*/") || t == "{"
    }
    let blocks: Vec<(String, Vec<String>)> = all_files.iter().filter_map(|fp| {
        let content = fs::read_to_string(fp).ok()?;
        let rel = fp.strip_prefix(root).unwrap_or(fp).display().to_string();
        let lines: Vec<String> = content.lines().map(|l| l.trim().to_string()).filter(|l| !is_header(l)).collect();
        Some((rel, lines))
    }).collect();
    for i in 0..blocks.len() {
        for j in (i + 1)..blocks.len() {
            let (f1, l1) = &blocks[i]; let (f2, l2) = &blocks[j];
            if l1.len() < DUPLICATE_MIN_LINES || l2.len() < DUPLICATE_MIN_LINES { continue; }
            for s1 in 0..=l1.len().saturating_sub(DUPLICATE_MIN_LINES) {
                for s2 in 0..=l2.len().saturating_sub(DUPLICATE_MIN_LINES) {
                    let mut len = 0;
                    while s1 + len < l1.len() && s2 + len < l2.len() && l1[s1 + len] == l2[s2 + len] { len += 1; }
                    if len >= DUPLICATE_MIN_LINES {
                        result.push((f1.clone(), s1 + 1, f2.clone(), len));
                        break;
                    }
                }
            }
        }
    }
    result
}

// ── TUI ─────────────────────────────────────────────────────

const TABS: &[&str] = &[" Summary ", " Warnings ", " Infos "];

struct App {
    display: Vec<FileReport>,
    list_state: ListState,
    active_tab: usize,
    file_count: usize,
    total_w: usize,
    total_i: usize,
}

fn sev_color(s: &Severity) -> Color {
    match s { Severity::Error => Color::Red, Severity::Warning => Color::Yellow, Severity::Info => Color::Cyan }
}
fn sev_label(s: &Severity) -> &'static str {
    match s { Severity::Error => "ERR", Severity::Warning => "WARN", Severity::Info => "INFO" }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let args: Vec<String> = std::env::args().collect();
    let rs_files: Vec<std::path::PathBuf> = if args.len() > 1 {
        args[1..].iter().map(|a| root.join(a)).collect()
    } else {
        let mut files = walk_rs_files(&root);
        files.retain(|p| p.strip_prefix(&root).map(|r| r.to_string_lossy().starts_with("kernel")).unwrap_or(false));
        files
    };
    let mut reports: Vec<FileReport> = Vec::new();
    let dead = dead_code(&rs_files, &root);
    let dup = duplicate_code(&rs_files, &root);

    for filepath in &rs_files {
        let content = match fs::read_to_string(filepath) { Ok(c) => c, _ => continue };
        let relative = filepath.strip_prefix(&root).unwrap_or(filepath).display().to_string();
        let mut issues = scan_file(&content);
        for (f, l, name) in &dead { if *f == relative { emit(&mut issues, *l, Severity::Warning, "dead-code", name.clone()); } }
        for (f, l, other, len) in &dup { if *f == relative { emit(&mut issues, *l, Severity::Info, "duplicate", format!("{} lines match {}", len, other)); } }
        let warnings = issues.iter().filter(|i| matches!(i.severity, Severity::Warning)).count();
        let infos = issues.iter().filter(|i| matches!(i.severity, Severity::Info)).count();
        reports.push(FileReport { path: relative, issues, warnings, infos });
    }
    reports.sort_by(|a, b| b.warnings.cmp(&a.warnings).then(b.infos.cmp(&a.infos)).then(a.path.cmp(&b.path)));

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;

    let total_w: usize = reports.iter().map(|r| r.warnings).sum();
    let total_i: usize = reports.iter().map(|r| r.infos).sum();
    let mut app = App {
        display: reports.into_iter().filter(|r| r.warnings > 0 || r.infos > 0).collect(),
        list_state: { let mut s = ListState::default(); s.select(Some(0)); s },
        active_tab: 0,
        file_count: rs_files.len(),
        total_w,
        total_i,
    };

    let res = run_app(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    if let Err(e) = res { eprintln!("Error: {}", e); }
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = app.list_state.selected().unwrap_or(0);
                        if i > 0 { app.list_state.select(Some(i - 1)); }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = app.list_state.selected().unwrap_or(0);
                        if i + 1 < app.display.len() { app.list_state.select(Some(i + 1)); }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        app.active_tab = app.active_tab.saturating_sub(1);
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if app.active_tab + 1 < TABS.len() { app.active_tab += 1; }
                    }
                    KeyCode::Char('1') => app.active_tab = 0,
                    KeyCode::Char('2') => app.active_tab = 1,
                    KeyCode::Char('3') => app.active_tab = 2,
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // ── Header ──
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" CodeCheck  {} files scanned, {} with issues  {}W  {}I    [1-3 tabs  j/k nav  q quit]", app.file_count, app.display.len(), app.total_w, app.total_i),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ))).style(Style::default().bg(Color::from_u32(0x222222))),
        chunks[0],
    );

    // ── File List ──
    let items: Vec<ListItem> = app.display.iter().map(|r| {
        let path_display = if r.path.len() > 48 { format!("...{}", &r.path[r.path.len()-45..]) } else { r.path.clone() };
        let w_part = if r.warnings > 0 { format!(" {}W", r.warnings) } else { "  ".to_string() };
        let i_part = if r.infos > 0 { format!(" {}I", r.infos) } else { "  ".to_string() };
        ListItem::new(Line::from(vec![
            Span::styled(path_display, Style::default().fg(Color::White)),
            Span::raw(" ".repeat(55usize.saturating_sub(r.path.len().min(48)))),
            Span::styled(w_part, if r.warnings > 0 { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::DarkGray) }),
            Span::styled(i_part, if r.infos > 0 { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) }),
        ]))
    }).collect();

    let file_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Files "))
        .highlight_style(Style::default().bg(Color::from_u32(0x444444)).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_stateful_widget(file_list, main[0], &mut app.list_state);

    // ── Detail Panel ──
    let detail_area = main[1];
    if let Some(sel) = app.list_state.selected() {
        if sel < app.display.len() {
            let r = &app.display[sel];

            // Tab bar
            let tab_titles: Vec<Line> = TABS.iter().map(|t| {
                Line::from(Span::styled(*t, Style::default().fg(Color::White)))
            }).collect();
            let tabs = Tabs::new(tab_titles)
                .block(Block::default().borders(Borders::ALL))
                .select(app.active_tab)
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
            let tab_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(detail_area);
            f.render_widget(tabs, tab_chunks[0]);

            // Tab content
            let content = match app.active_tab {
                0 => render_summary(r),
                1 => render_issues(&r.issues, Severity::Warning),
                2 => render_issues(&r.issues, Severity::Info),
                _ => vec![],
            };
            let paragraph = Paragraph::new(content)
                .block(Block::default().borders(Borders::ALL).title(TABS[app.active_tab].trim()))
                .wrap(Wrap { trim: false });
            f.render_widget(paragraph, tab_chunks[1]);
            return;
        }
    }
    let empty = Paragraph::new(Line::from(Span::styled(" Select a file to view details ", Style::default().fg(Color::DarkGray))))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(empty, detail_area);
}

fn render_summary(r: &FileReport) -> Vec<Line<'static>> {
    let path = r.path.clone();
    let mut lines = vec![
        Line::from(Span::styled(path, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(Span::raw(format!("  {} warnings, {} infos\n", r.warnings, r.infos))),
    ];
    if r.issues.is_empty() {
        lines.push(Line::from(Span::styled("  No issues found", Style::default().fg(Color::Green))));
        return lines;
    }
    // Group by kind
    let mut warn_groups: BTreeMap<&str, usize> = BTreeMap::new();
    let mut info_groups: BTreeMap<&str, usize> = BTreeMap::new();
    for i in &r.issues {
        match i.severity {
            Severity::Warning => *warn_groups.entry(i.kind).or_insert(0) += 1,
            Severity::Info => *info_groups.entry(i.kind).or_insert(0) += 1,
            _ => {}
        }
    }
    if !warn_groups.is_empty() {
        lines.push(Line::from(Span::styled(" Warnings by type:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
        for (kind, count) in &warn_groups {
            lines.push(Line::from(Span::raw(format!("   {}  {}", count, kind))));
        }
        lines.push(Line::from(Span::raw("")));
    }
    if !info_groups.is_empty() {
        lines.push(Line::from(Span::styled(" Infos by type:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
        for (kind, count) in &info_groups {
            lines.push(Line::from(Span::raw(format!("   {}  {}", count, kind))));
        }
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(" [Tab 2: Warnings]  [Tab 3: Infos]  [← → or 1-3 to switch]",
        Style::default().fg(Color::DarkGray))));
    lines
}

fn render_issues(issues: &[Issue], sev: Severity) -> Vec<Line<'static>> {
    let filtered: Vec<&Issue> = issues.iter().filter(|i| i.severity as u8 == sev as u8).collect();
    if filtered.is_empty() {
        return vec![Line::from(Span::styled("  No issues", Style::default().fg(Color::Green)))];
    }
    let mut lines = Vec::new();
    for issue in filtered.iter().take(100) {
        let color = sev_color(&issue.severity);
        let slabel = sev_label(&issue.severity);
        lines.push(Line::from(vec![
            Span::styled(format!("{:>3} ", issue.line), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("[{}] ", slabel), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}  {}", issue.kind.replace('-', " "), issue.message), color),
        ]));
    }
    if filtered.len() > 100 {
        lines.push(Line::from(Span::styled(format!("  ... {} more issues", filtered.len() - 100), Style::default().fg(Color::DarkGray))));
    }
    lines
}
