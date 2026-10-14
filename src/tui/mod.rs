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
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::{collections::HashSet, fs, io, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::mpsc;

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
#[derive(Clone, Copy)]
enum Focus {
    Explorer,
    Target,
}

struct App {
    directory: PathBuf,
    entries: Vec<PathBuf>,
    selected: usize,
    marked: HashSet<PathBuf>,
    target: String,
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
                KeyCode::Down => {
                    app.selected = (app.selected + 1).min(app.entries.len().saturating_sub(1))
                }
                KeyCode::Up => app.selected = app.selected.saturating_sub(1),
                KeyCode::Tab => {
                    app.focus = match app.focus {
                        Focus::Explorer => Focus::Target,
                        Focus::Target => Focus::Explorer,
                    }
                }
                KeyCode::Backspace => {
                    app.go_parent()?;
                }
                KeyCode::Enter => match app.focus {
                    Focus::Explorer => app.open_selected()?,
                    Focus::Target => app.enqueue_conversion(),
                },
                KeyCode::Char(' ') => app.toggle_mark(),
                KeyCode::Char('p' | 'P') => app.target = "pdf".into(),
                KeyCode::Char('t' | 'T') => app.target = "txt".into(),
                KeyCode::Char('d' | 'D') => app.target = "docx".into(),
                _ => {}
            },
            Event::Mouse(mouse)
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) =>
            {
                // İç çerçevenin sol panelinde tıklanan satırı seçer; sağdaki format kartları hedefi değiştirir.
                let size = terminal.size()?;
                let split = (size.width as u32 * 40 / 100) as u16;
                if mouse.column < split && mouse.row >= 3 {
                    let index = (mouse.row - 3) as usize;
                    if index < app.entries.len() {
                        app.selected = index;
                        app.focus = Focus::Explorer;
                    }
                } else if mouse.column >= split && mouse.row >= 8 && mouse.row <= 10 {
                    app.target = if mouse.column < split + 17 {
                        "pdf"
                    } else if mouse.column < split + 35 {
                        "txt"
                    } else {
                        "docx"
                    }
                    .into();
                    app.focus = Focus::Target;
                }
            }
            _ => {}
        }
    }
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let root = frame.size();
    let outer = Block::default()
        .title(" DocVert TUI v0.1.0 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(outer, root);
    let inner = Rect::new(
        root.x + 1,
        root.y + 1,
        root.width.saturating_sub(2),
        root.height.saturating_sub(2),
    );
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
        .split(inner);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(vertical[0]);

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
    let explorer_title = format!(" Klasör gezgini — {} ", app.directory.display());
    let list = List::new(items)
        .block(
            Block::default()
                .title(explorer_title)
                .borders(Borders::ALL)
                .border_style(focus_style(app, Focus::Explorer)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    frame.render_stateful_widget(list, top[0], &mut state);

    let selected = app.selected_path();
    let name = selected
        .and_then(|p| p.file_name())
        .map(|v| v.to_string_lossy())
        .unwrap_or_else(|| "—".into());
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
    let drivers = app.registry.driver_names().join("\n🟢 ");
    let detail = vec![
        Line::from(vec![
            Span::styled("Dosya adı: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(name),
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
        Line::from(format!(
            "[{} PDF]  [{} TXT]  [{} DOCX]",
            radio(&app.target, "pdf"),
            radio(&app.target, "txt"),
            radio(&app.target, "docx")
        )),
        Line::from(""),
        Line::from(Span::styled(
            "🛠 SÜRÜCÜ DURUMU",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("🟢 {drivers}")),
        Line::from(""),
        Line::from("Enter: dönüştür  |  P/T/D: format seç"),
    ];
    let preview = Paragraph::new(detail)
        .block(
            Block::default()
                .title(" Dosya önizleme & ayarlar ")
                .borders(Borders::ALL)
                .border_style(focus_style(app, Focus::Target)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(preview, top[1]);

    let queue_block = Block::default()
        .title(" Dönüşüm kuyruğu ve ilerleme ")
        .borders(Borders::ALL);
    frame.render_widget(queue_block, vertical[1]);
    let usable = Rect::new(
        vertical[1].x + 1,
        vertical[1].y + 1,
        vertical[1].width.saturating_sub(2),
        vertical[1].height.saturating_sub(2),
    );
    if app.queue.is_empty() {
        frame.render_widget(Paragraph::new("Kuyruk boş. Space ile dosyaları seçin, Enter ile dönüştürün.\nKısayollar: Tab panel | Backspace üst klasör | Q çıkış"), usable);
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                app.queue
                    .iter()
                    .map(|_| Constraint::Length(2))
                    .collect::<Vec<_>>(),
            )
            .split(usable);
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
}

fn focus_style(app: &App, panel: Focus) -> Style {
    let active = matches!(
        (app.focus, panel),
        (Focus::Explorer, Focus::Explorer) | (Focus::Target, Focus::Target)
    );
    Style::default().fg(if active { Color::Cyan } else { Color::DarkGray })
}
fn radio(current: &str, value: &str) -> &'static str {
    if current == value {
        "●"
    } else {
        "○"
    }
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
