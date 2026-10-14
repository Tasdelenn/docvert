//! Fare/klavye destekli DocVert terminal arayüzü.
use crate::core::registry::ConverterRegistry;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::{collections::HashSet, fs, io, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::mpsc;

/// Sağ panelde seçilebilen hedef formatlar (uzantı, etiket). Sıra, sekme
/// gezinmesiyle aynıdır.
const TARGET_FORMATS: [(&str, &str); 3] = [("pdf", "PDF"), ("txt", "TXT"), ("docx", "DOCX")];

#[derive(Clone, Debug)]
enum JobState {
    Waiting,
    Running,
    Done,
    Failed(String),
}
#[derive(Clone, Debug)]
struct QueueJob {
    input: PathBuf,
    output: PathBuf,
    state: JobState,
}
#[derive(Clone, Debug)]
struct JobUpdate {
    input: PathBuf,
    output: PathBuf,
    state: JobState,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Explorer,
    Target,
}

/// Çizim ve fare isabet testi tarafından paylaşılan tek yerleşim kaynağı.
/// Böylece tıklama koordinatları ile gerçek widget koordinatları kaymaz.
#[derive(Clone, Copy)]
struct LayoutAreas {
    explorer: Rect,
    explorer_list: Rect,
    explorer_info: Rect,
    preview: Rect,
    preview_details: Rect,
    target_cells: [Rect; 3],
    preview_drivers: Rect,
    queue: Rect,
    queue_inner: Rect,
    help: Rect,
}

struct App {
    directory: PathBuf,
    entries: Vec<PathBuf>,
    selected: usize,
    marked: HashSet<PathBuf>,
    target: String,
    target_index: usize,
    hovered_target: Option<usize>,
    focus: Focus,
    queue: Vec<QueueJob>,
    registry: Arc<ConverterRegistry>,
    sender: mpsc::UnboundedSender<JobUpdate>,
    receiver: mpsc::UnboundedReceiver<JobUpdate>,
}

impl App {
    fn new() -> io::Result<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut app = Self {
            directory: std::env::current_dir()?,
            entries: Vec::new(),
            selected: 0,
            marked: HashSet::new(),
            target: "pdf".into(),
            target_index: 0,
            hovered_target: None,
            focus: Focus::Explorer,
            queue: Vec::new(),
            registry: Arc::new(ConverterRegistry::with_detected_drivers()),
            sender,
            receiver,
        };
        app.refresh()?;
        Ok(app)
    }

    fn refresh(&mut self) -> io::Result<()> {
        let mut entries: Vec<_> = fs::read_dir(&self.directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        entries.sort_by_key(|path| (!path.is_dir(), path.file_name().map(|s| s.to_os_string())));
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        Ok(())
    }

    fn selected_path(&self) -> Option<&PathBuf> {
        self.entries.get(self.selected)
    }

    fn toggle_mark(&mut self) {
        if let Some(path) = self.selected_path().cloned().filter(|p| p.is_file()) {
            if !self.marked.insert(path.clone()) {
                self.marked.remove(&path);
            }
        }
    }

    /// Hedef formatı ve buna karşılık gelen sekme indeksini birlikte günceller.
    fn set_target(&mut self, value: &str) {
        self.target = value.to_string();
        self.target_index = target_index_of(value);
    }

    fn open_selected(&mut self) -> io::Result<()> {
        let Some(path) = self.selected_path().cloned() else {
            return Ok(());
        };
        if path.is_dir() {
            self.directory = path;
            self.refresh()?;
        }
        Ok(())
    }

    fn go_parent(&mut self) -> io::Result<()> {
        if let Some(parent) = self.directory.parent().map(PathBuf::from) {
            self.directory = parent;
            self.refresh()?;
        }
        Ok(())
    }

    fn enqueue_conversion(&mut self) {
        let inputs: Vec<PathBuf> = if self.marked.is_empty() {
            self.selected_path()
                .filter(|p| p.is_file())
                .cloned()
                .into_iter()
                .collect()
        } else {
            self.marked.iter().cloned().collect()
        };
        for input in inputs {
            let output = input.with_extension(&self.target);
            self.queue.push(QueueJob {
                input: input.clone(),
                output: output.clone(),
                state: JobState::Waiting,
            });
            let registry = Arc::clone(&self.registry);
            let sender = self.sender.clone();
            rayon::spawn(move || {
                let _ = sender.send(JobUpdate {
                    input: input.clone(),
                    output: output.clone(),
                    state: JobState::Running,
                });
                let state = match registry.convert(&input, &output) {
                    Ok(()) => JobState::Done,
                    Err(error) => JobState::Failed(error.to_string()),
                };
                let _ = sender.send(JobUpdate {
                    input,
                    output,
                    state,
                });
            });
        }
        self.marked.clear();
    }

    fn consume_updates(&mut self) {
        while let Ok(update) = self.receiver.try_recv() {
            if let Some(job) = self
                .queue
                .iter_mut()
                .find(|job| job.input == update.input && job.output == update.output)
            {
                job.state = update.state;
            }
        }
    }
}

fn target_index_of(value: &str) -> usize {
    TARGET_FORMATS
        .iter()
        .position(|(extension, _)| *extension == value)
        .unwrap_or(0)
}

pub fn run() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_loop(&mut terminal);
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new()?;
    loop {
        app.consume_updates();
        terminal.draw(|frame| draw(frame, &app))?;
        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => match key.code {
                KeyCode::Char('q' | 'Q') => return Ok(()),
                KeyCode::Down if app.focus == Focus::Explorer => {
                    app.selected = (app.selected + 1).min(app.entries.len().saturating_sub(1))
                }
                KeyCode::Up if app.focus == Focus::Explorer => {
                    app.selected = app.selected.saturating_sub(1)
                }
                // Sekme: gezginden hedef panele geçer, sonra formatlar arasında
                // ilerleyip seçer; son formattan sonra tekrar gezgine döner.
                KeyCode::Tab => match app.focus {
                    Focus::Explorer => {
                        app.focus = Focus::Target;
                        app.target_index = target_index_of(&app.target);
                    }
                    Focus::Target => {
                        if app.target_index + 1 < TARGET_FORMATS.len() {
                            app.target_index += 1;
                            app.set_target(TARGET_FORMATS[app.target_index].0);
                        } else {
                            app.focus = Focus::Explorer;
                        }
                    }
                },
                KeyCode::Left if app.focus == Focus::Target => {
                    app.target_index = app.target_index.saturating_sub(1);
                    app.set_target(TARGET_FORMATS[app.target_index].0);
                }
                KeyCode::Right if app.focus == Focus::Target => {
                    app.target_index = (app.target_index + 1).min(TARGET_FORMATS.len() - 1);
                    app.set_target(TARGET_FORMATS[app.target_index].0);
                }
                KeyCode::Backspace => {
                    app.go_parent()?;
                }
                KeyCode::Enter => match app.focus {
                    Focus::Explorer => app.open_selected()?,
                    Focus::Target => app.enqueue_conversion(),
                },
                KeyCode::Char(' ') => app.toggle_mark(),
                KeyCode::Char('p' | 'P') => app.set_target("pdf"),
                KeyCode::Char('t' | 'T') => app.set_target("txt"),
                KeyCode::Char('d' | 'D') => app.set_target("docx"),
                _ => {}
            },
            Event::Mouse(mouse) => {
                let size = terminal.size()?;
                let areas = compute_layout(Rect::new(0, 0, size.width, size.height));
                match mouse.kind {
                    MouseEventKind::Moved => {
                        app.hovered_target = hit_target(&areas, mouse.column, mouse.row);
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(index) = hit_explorer_row(&areas, mouse.column, mouse.row) {
                            if index < app.entries.len() {
                                app.selected = index;
                                app.focus = Focus::Explorer;
                            }
                        } else if let Some(index) = hit_target(&areas, mouse.column, mouse.row) {
                            app.set_target(TARGET_FORMATS[index].0);
                            app.focus = Focus::Target;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// Terminal alanından tüm bölümlerin gerçek koordinatlarını üretir.
fn compute_layout(area: Rect) -> LayoutAreas {
    let inner = shrink(area, 1);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(72),
            Constraint::Percentage(24),
            Constraint::Length(1),
        ])
        .split(inner);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(vertical[0]);

    // Sol panel: liste + seçili dosya bilgisi (ad ve yol) alt satırları.
    let explorer_inner = shrink(top[0], 1);
    let explorer_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(explorer_inner);

    // Sağ panel: detaylar, hedef format kartları, sürücüler.
    let preview_inner = shrink(top[1], 1);
    let preview_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .split(preview_inner);
    let target_cells = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(preview_split[1]);

    LayoutAreas {
        explorer: top[0],
        explorer_list: explorer_split[0],
        explorer_info: explorer_split[1],
        preview: top[1],
        preview_details: preview_split[0],
        target_cells: [target_cells[0], target_cells[1], target_cells[2]],
        preview_drivers: preview_split[2],
        queue: vertical[1],
        queue_inner: shrink(vertical[1], 1),
        help: vertical[2],
    }
}

fn shrink(rect: Rect, by: u16) -> Rect {
    Rect::new(
        rect.x.saturating_add(by),
        rect.y.saturating_add(by),
        rect.width.saturating_sub(by.saturating_mul(2)),
        rect.height.saturating_sub(by.saturating_mul(2)),
    )
}

fn contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// Fare koordinatını gezgin listesindeki satır indeksine çevirir; liste
/// dışındaysa `None` döner.
fn hit_explorer_row(areas: &LayoutAreas, column: u16, row: u16) -> Option<usize> {
    if contains(areas.explorer_list, column, row) {
        Some((row - areas.explorer_list.y) as usize)
    } else {
        None
    }
}

/// Fare koordinatının hangi hedef format kartına denk geldiğini döndürür.
fn hit_target(areas: &LayoutAreas, column: u16, row: u16) -> Option<usize> {
    areas
        .target_cells
        .iter()
        .position(|cell| contains(*cell, column, row))
}

/// Uzun metni, sonu (daha anlamlı kısım) görünecek biçimde kısaltır.
fn fit_end(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= width {
        return text.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let tail: String = characters[characters.len() - (width - 1)..]
        .iter()
        .collect();
    format!("…{tail}")
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let areas = compute_layout(frame.size());

    let outer = Block::default()
        .title(" DocVert TUI v0.1.0 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(outer, frame.size());

    // ---- Sol panel: klasör gezgini ----
    let directory = app.directory.display().to_string();
    let explorer = Block::default()
        .title(format!(
            " Klasör gezgini — {} ",
            fit_end(&directory, areas.explorer.width.saturating_sub(22) as usize)
        ))
        .borders(Borders::ALL)
        .border_style(focus_style(app, Focus::Explorer));
    frame.render_widget(explorer, areas.explorer);

    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|path| {
            let icon = if path.is_dir() { "📁" } else { "📄" };
            let mark = if app.marked.contains(path) {
                "✓ "
            } else {
                "  "
            };
            ListItem::new(format!(
                "{mark}{icon} {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.selected));
    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    frame.render_stateful_widget(list, areas.explorer_list, &mut state);

    // Sol panelin altında seçili öğenin adı ve tam yolu, okunaklı biçimde.
    let selected = app.selected_path();
    let selected_name = selected
        .and_then(|p| p.file_name())
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_else(|| "—".into());
    let selected_path = selected
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "—".into());
    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Seçili: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                selected_name.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            fit_end(&selected_path, areas.explorer_info.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
    ]);
    frame.render_widget(info, areas.explorer_info);

    // ---- Sağ panel: dosya önizleme & ayarlar ----
    let preview_block = Block::default()
        .title(" Dosya önizleme & ayarlar ")
        .borders(Borders::ALL)
        .border_style(focus_style(app, Focus::Target));
    frame.render_widget(preview_block, areas.preview);

    let size = selected
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| human_size(m.len()))
        .unwrap_or_else(|| "—".into());
    let kind = selected
        .map(|p| {
            if p.is_dir() {
                "Klasör"
            } else {
                p.extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("Bilinmeyen")
            }
        })
        .unwrap_or("—");
    let detail = vec![
        Line::from(vec![
            Span::styled("Dosya adı: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(selected_name.clone()),
        ]),
        Line::from(vec![
            Span::styled("Yol: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                fit_end(
                    &selected_path,
                    areas.preview_details.width.saturating_sub(5) as usize,
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(format!("Boyut: {size}")),
        Line::from(format!("Tür: {kind}")),
        Line::from(""),
        Line::from(Span::styled(
            "⚙ HEDEF FORMAT",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Tab / ◄► ile seç, Enter ile dönüştür",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(detail), areas.preview_details);

    // Hedef format kartları: seçili, odaklı ve fareyle üzerine gelinen hâli
    // farklı renklerle vurgular.
    for (index, (value, label)) in TARGET_FORMATS.iter().enumerate() {
        let is_selected = app.target == *value;
        let is_focused = app.focus == Focus::Target && app.target_index == index;
        let is_hovered = app.hovered_target == Some(index);
        let accent = if is_hovered {
            Color::Magenta
        } else if is_focused {
            Color::Cyan
        } else if is_selected {
            Color::Green
        } else {
            Color::DarkGray
        };
        let mut title_style = Style::default().fg(accent);
        if is_hovered || is_focused || is_selected {
            title_style = title_style.add_modifier(Modifier::BOLD);
        }
        let marker = if is_selected { "●" } else { "○" };
        let card = Block::default()
            .title(Line::from(Span::styled(
                format!(" {marker} {label} "),
                title_style,
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let body = if is_selected { "seçili" } else { "" };
        frame.render_widget(
            Paragraph::new(body)
                .alignment(Alignment::Center)
                .block(card),
            areas.target_cells[index],
        );
    }

    let drivers = app.registry.driver_names().join("  ·  🟢 ");
    let drivers_para = Paragraph::new(vec![
        Line::from(Span::styled(
            "🛠 SÜRÜCÜ DURUMU",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("🟢 {drivers}")),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(drivers_para, areas.preview_drivers);

    // ---- Alt panel: dönüşüm kuyruğu ----
    let queue_block = Block::default()
        .title(" Dönüşüm kuyruğu ve ilerleme ")
        .borders(Borders::ALL);
    frame.render_widget(queue_block, areas.queue);
    if app.queue.is_empty() {
        frame.render_widget(
            Paragraph::new("Kuyruk boş. Space ile dosyaları seçin, Enter ile dönüştürün."),
            areas.queue_inner,
        );
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                app.queue
                    .iter()
                    .map(|_| Constraint::Length(2))
                    .collect::<Vec<_>>(),
            )
            .split(areas.queue_inner);
        for (job, area) in app
            .queue
            .iter()
            .rev()
            .take(rows.len())
            .zip(rows.iter().copied())
        {
            let (label, ratio, color) = match &job.state {
                JobState::Waiting => ("Bekliyor", 0.0, Color::DarkGray),
                JobState::Running => ("İşleniyor", 0.55, Color::Yellow),
                JobState::Done => ("Tamamlandı", 1.0, Color::Green),
                JobState::Failed(error) => (error.as_str(), 1.0, Color::Red),
            };
            let title = format!(
                "{} → {}  ({label})",
                job.input.file_name().unwrap_or_default().to_string_lossy(),
                job.output.file_name().unwrap_or_default().to_string_lossy()
            );
            frame.render_widget(
                Gauge::default()
                    .block(Block::default().title(title))
                    .gauge_style(Style::default().fg(color))
                    .ratio(ratio),
                area,
            );
        }
    }

    // ---- En alt satır: kısayol yardımı ----
    let help = Paragraph::new(Line::from(vec![
        Span::styled("Kısayollar: ", Style::default().fg(Color::DarkGray)),
        key_hint("↑/↓"),
        Span::raw(" gezin  "),
        key_hint("Tab"),
        Span::raw(" panel/format  "),
        key_hint("Space"),
        Span::raw(" seç  "),
        key_hint("Enter"),
        Span::raw(" dönüştür  "),
        key_hint("Backspace"),
        Span::raw(" üst klasör  "),
        key_hint("P/T/D"),
        Span::raw(" format  "),
        key_hint("Q"),
        Span::raw(" çıkış"),
    ]));
    frame.render_widget(help, areas.help);
}

fn key_hint(text: &'static str) -> Span<'static> {
    Span::styled(
        text,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn focus_style(app: &App, panel: Focus) -> Style {
    let active = matches!(
        (app.focus, panel),
        (Focus::Explorer, Focus::Explorer) | (Focus::Target, Focus::Target)
    );
    Style::default().fg(if active { Color::Cyan } else { Color::DarkGray })
}
fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Çizim ve fare isabet testi aynı yerleşimi kullanmalı: gezginin ilk
    /// satırına tıklamak 0. indeksi vermeli, bir üst satır liste dışı olmalı.
    #[test]
    fn explorer_click_maps_row_without_offset() {
        let areas = compute_layout(Rect::new(0, 0, 120, 40));
        let list = areas.explorer_list;
        assert!(list.height > 0, "gezgin listesi görünür olmalı");

        let column = list.x;
        assert_eq!(hit_explorer_row(&areas, column, list.y), Some(0));
        assert_eq!(
            hit_explorer_row(&areas, column, list.y + list.height - 1),
            Some(list.height as usize - 1)
        );
        // Başlık/çerçeve satırına tıklamak liste dışı olmalı.
        assert_eq!(hit_explorer_row(&areas, column, list.y - 1), None);
        assert_eq!(hit_explorer_row(&areas, column, list.y + list.height), None);
    }

    /// Her hedef kartının merkezine tıklamak doğru formatı seçmeli; kartlar
    /// soldan sağa sıralı ve çakışmasız olmalı.
    #[test]
    fn target_cells_are_ordered_and_clickable() {
        let areas = compute_layout(Rect::new(0, 0, 120, 40));
        for index in 0..TARGET_FORMATS.len() {
            let cell = areas.target_cells[index];
            assert!(
                cell.width > 0 && cell.height > 0,
                "kart {index} görünür olmalı"
            );
            let center = (cell.x + cell.width / 2, cell.y + cell.height / 2);
            assert_eq!(hit_target(&areas, center.0, center.1), Some(index));
            if index > 0 {
                let previous = areas.target_cells[index - 1];
                assert!(previous.x + previous.width <= cell.x, "kartlar çakışmamalı");
            }
        }
    }

    /// Bölümler dikey sırayla ve terminal alanı içinde yerleşmeli.
    #[test]
    fn layout_sections_are_stacked_in_order() {
        let areas = compute_layout(Rect::new(0, 0, 120, 40));
        assert!(areas.preview_details.y < areas.target_cells[0].y);
        assert!(areas.target_cells[0].y < areas.preview_drivers.y);
        assert!(areas.queue.y > areas.explorer.y);
        assert!(areas.help.y > areas.queue.y);
        assert!(areas.help.y + areas.help.height <= 40);
    }

    /// Küçük terminallerde de çökmeden bir yerleşim üretilmeli.
    #[test]
    fn layout_survives_tiny_terminal() {
        let areas = compute_layout(Rect::new(0, 0, 8, 3));
        assert!(areas.help.height <= 1);
        assert!(hit_target(&areas, 0, 0).is_none());
    }

    /// Uzun metin kısaltılırken sonu korunur (yol için daha okunaklı).
    #[test]
    fn fit_end_keeps_tail() {
        assert_eq!(fit_end("/a/b/c.pdf", 20), "/a/b/c.pdf");
        assert_eq!(fit_end("/a/b/c.pdf", 6), "…c.pdf");
        assert_eq!(fit_end("/a/b/c.pdf", 1), "…");
        assert_eq!(fit_end("/a/b/c.pdf", 0), "");
    }

    #[test]
    fn target_index_lookup_defaults_to_first() {
        assert_eq!(target_index_of("pdf"), 0);
        assert_eq!(target_index_of("txt"), 1);
        assert_eq!(target_index_of("docx"), 2);
        assert_eq!(target_index_of("yok"), 0);
    }
}
