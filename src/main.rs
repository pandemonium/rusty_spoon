use std::{cmp, fmt::{self, Display}, fs, io, path, ops::Range};

mod elm;
use elm::Host;

mod tui;

/* Make a crossterm prelude for the elm module? */
use crossterm::{cursor, event, event::{KeyCode, KeyModifiers}, style, QueueableCommand, terminal};
use tui::RenderingBuffer;

#[derive(Clone, Debug)]
struct ScreenSize {
    columns: usize,
    rows:    usize,
}

impl ScreenSize {
    fn new(columns: usize, rows: usize) -> Self {
        Self { columns, rows }
    }

    fn request() -> elm::Cmd<Message> {
        tui::request_terminal_size(|width, height|
            Message::SizedChanged((width, height).into())
        )
    }
}

impl fmt::Display for ScreenSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.columns, self.rows)
    }
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self { columns: Default::default(), rows: Default::default() }
    }
}

impl From<(u16, u16)> for ScreenSize {
    fn from(value: (u16, u16)) -> Self {
        ScreenSize::new(value.0 as usize, value.1 as usize)
    }
}

#[derive(Debug)]
struct EditingViewport {
    row_offset:   usize,
    column_offset: usize,
}

impl EditingViewport {
    fn select_and_clip<'a>(
        &self,
        line_index: usize, 
        width:      usize, 
        lines:      &'a[String]
    ) -> Option<&'a str> {
        let effective_line_index = self.row_offset + line_index;
        if effective_line_index < lines.len() {
            let line = &lines[effective_line_index];

            if self.column_offset < line.len() {
                let len = cmp::min(width, line.len().saturating_sub(self.column_offset));
                let end = self.column_offset + len;
                let start = self.column_offset;
                let slice = start..end;
                Some(&line[slice])
            } else if !line.is_empty() {
                Some(&"«")
            } else {
                Some(&"")
            }
        } else {
            None
        }
    }

    fn scroll_up(&mut self, by: usize) {
        self.row_offset = self.row_offset.saturating_sub(by);
    }

    fn scroll_down(&mut self, by: usize) {
        self.row_offset += by;
    }

    fn scroll_left(&mut self, by: usize) {
        self.column_offset = self.column_offset.saturating_sub(by);
    }

    fn scroll_right(&mut self, by: usize) {
        self.column_offset += by;
    }
}

impl Default for EditingViewport {
    fn default() -> Self {
        Self { row_offset: Default::default(), column_offset: Default::default() }
    }
}

struct EditingModel {
    lines: Vec<String>,
}

impl EditingModel {
    fn new() -> Self {
        Self {
            lines: vec![
                "hi, mom".into(),
                "Hello, world".into(),
            ],
        }
    }

    fn with_lines(lines: &[String]) -> Self {
        Self { lines: lines.to_vec(), }
    }

    fn from_file(file_path: &path::Path) -> io::Result<Self> {
        let file_contents = fs::read_to_string(file_path)?;
        let lines = file_contents.lines()
            .map(|line| line.to_owned())
            .collect::<Vec<_>>();
        Ok(Self::with_lines(&lines))
    }

    fn line_count(&self) -> usize { self.lines.len() }

    fn line_slice(&self, line_index: usize, range: Range<usize>) -> Option<&str> {
        self.lines.get(line_index).map(|line| &line[range])
    }
}

impl Default for EditingModel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct Position {
    column:      usize,
    row:         usize,
}

impl Position {
    fn move_up(&mut self, by: usize)    { self.row = self.row.saturating_sub(by)      }
    fn move_down(&mut self, by: usize)  { self.row += by  /* no! */                            }
    fn move_left(&mut self)             { self.column = self.column.saturating_sub(1) }
    fn move_right(&mut self)            { self.column += 1 /* No! */                           }
}

impl Default for Position {
    fn default() -> Self {
        Self { column: Default::default(), row: Default::default() }
    }
}

struct NavigationModel {
    cursor:      Position,
    screen_size: ScreenSize,
    viewport:    EditingViewport,
}

impl NavigationModel {
    fn is_topmost(&self)    -> bool { self.cursor.row == 0                               }
    fn is_bottommost(&self) -> bool { self.cursor.row == self.screen_size.rows - 1       }
    fn is_leftmost(&self)   -> bool { self.cursor.column == 0                            }
    fn is_rightmost(&self)  -> bool { self.cursor.column == self.screen_size.columns - 1 }

    fn is_recognized(direction: &KeyCode) -> bool {
        matches!(
            direction, 
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right |
            KeyCode::PageUp | KeyCode::PageDown
        )
    }

    fn move_intended(&mut self, direction: &KeyCode) {
        match direction {
            KeyCode::Up    => {
                if self.is_topmost() {
                    self.viewport.scroll_up(1)
                } else {
                    self.cursor.move_up(1)
                }
            }

            KeyCode::Down  => {
                if self.is_bottommost() {
                    self.viewport.scroll_down(1)
                } else {
                    self.cursor.move_down(1)
                }
            }

            KeyCode::Left  => {
                if self.is_leftmost() {
                    self.viewport.scroll_left(1)
                } else {
                    self.cursor.move_left()
                }
            }

            KeyCode::Right => {
                if self.is_rightmost() {
                    self.viewport.scroll_right(1)
                } else {
                    self.cursor.move_right()
                }
            }

            KeyCode::PageUp => {
                let page = self.screen_size.rows;
                let scroll_by = page.saturating_sub(self.cursor.row);
                self.cursor.move_up(page);
                self.viewport.scroll_up(scroll_by);
            }

            KeyCode::PageDown => {
                let page = self.screen_size.rows;
                let scroll_by = self.cursor.row;
                self.viewport.scroll_down(scroll_by);
                let move_by = page.saturating_sub(self.cursor.row);
                self.cursor.move_down(move_by);
            }

            _otherwise => unimplemented!(),
        }
    }

    fn screen_size_changed(&mut self, new_size: ScreenSize) -> elm::Cmd<Message> {
        self.screen_size = new_size;
        elm::Cmd::none()
    }
}

impl Default for NavigationModel {
    fn default() -> Self {
        Self {
            cursor:      Default::default(), 
            screen_size: Default::default(),
            viewport:    Default::default(),
        }
    }
}

struct KeyEvent(event::KeyEvent);

impl From<&event::KeyEvent> for KeyEvent {
    fn from(event: &event::KeyEvent) -> Self {
        Self(event.clone())
    }
}

impl Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} [{:?}]", self.0.code, self.0.modifiers)
    }
}

struct KeyHistory {
    events: Vec<KeyEvent>,
    horizon: usize,
}

impl KeyHistory {
    fn record(&mut self, event: &event::KeyEvent) {
        self.events.push(event.into());
        if self.events.len() > self.horizon {
            self.events.remove(0);
        }
    }
}

impl Display for KeyHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for event in &self.events {
            write!(f, "{},", event)?;
        }
        write!(f, "]")
    }
}

impl Default for KeyHistory {
    fn default() -> Self {
        Self { events: Default::default(), horizon: 3 }
    }
}

struct Editor {
    buffer_name: String,
    contents:    EditingModel,
    navigation:  NavigationModel,
    key_history: KeyHistory,
}

impl Editor {
    fn key_typed(&mut self, key: &event::KeyEvent) -> elm::Cmd<Message> {
        match key {
            event::KeyEvent {
                code:      KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => 
                elm::Cmd::gtfo(),

            event::KeyEvent {
                code:      direction,
                modifiers: KeyModifiers::NONE,
                ..
            } if NavigationModel::is_recognized(direction) => {
                self.navigation.move_intended(direction);
                elm::Cmd::none()
            }

            ev @ event::KeyEvent { .. } =>
                self.record_key_event(ev),
        }
    }

    fn record_key_event(&mut self, ev: &event::KeyEvent) -> elm::Cmd<Message> {
        self.key_history.record(ev);
        elm::Cmd::none()
    }

    fn event_occurred(&mut self, event: &event::Event) -> elm::Cmd<Message> {
        match event {
            event::Event::Key(key) =>
                self.key_typed(key),
            event::Event::Resize(width, height) =>
                self.navigation.screen_size_changed((*width, *height).into()),
            _otherwise =>
                elm::Cmd::none(),
        }
    }

    fn render(&self, buffer: &mut RenderingBuffer) -> io::Result<()> {
        let cursor_bounds = &self.navigation.screen_size;

        /* At least consider putting the draw methods behind some
           trait to cut down on the amount of code clutter. */

        buffer
           .queue(cursor::Hide)?
           .queue(cursor::MoveTo(0, 0))?;

        self.render_contents(buffer)?;

        let navigation_message = format!(
            "size: {:?}, cursor: {:?}, view: {:?}",
            cursor_bounds,
            self.navigation.cursor,
            self.navigation.viewport,
        );

        let key_message = format!("History: {}", self.key_history);

        buffer
            .queue(cursor::MoveTo(5, 10))?
            .queue(style::Print(navigation_message))?
            .queue(cursor::MoveTo(5, 15))?
            .queue(style::Print(key_message))?
            .queue(cursor::MoveTo(
                self.navigation.cursor.column as u16,
                self.navigation.cursor.row as u16,
            ))?
            .queue(cursor::Show)?;

        Ok(())
    }

    fn render_contents(&self, buffer: &mut RenderingBuffer) -> io::Result<()> {
        let cursor_bounds = &self.navigation.screen_size;
        for i in 0..cursor_bounds.rows  {
            let line = self.render_line(i as usize);

            buffer.queue(style::Print(line))?
                  .queue(terminal::Clear(terminal::ClearType::UntilNewLine))?;

            if i < cursor_bounds.rows - 1 {
                buffer.queue(style::Print("\r\n"))?;
            }
        }

        Ok(())
    }

    fn render_line(&self, viewport_line_index: usize) -> &str {
        let width = self.navigation.screen_size.columns as usize;
        self.navigation.viewport
            .select_and_clip(viewport_line_index, width, &self.contents.lines)
            .unwrap_or("~")
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            buffer_name: "Unnamed".to_owned(),
            contents:    EditingModel::from_file(path::Path::new("src/main.rs")).unwrap(),
            navigation:  NavigationModel::default(),
            key_history: Default::default(),
        }
    }
}

#[derive(Clone)]
enum Message {
    SetBufferName(String),
    ExternalEvent(event::Event),
    SizedChanged(ScreenSize),
}

impl Message {
    
}

impl elm::Application for Editor {
    type Msg  = Message;
    type View = tui::Screen;

    fn init() -> (Self, elm::Cmd<Message>) {
        (Editor::default(), ScreenSize::request())
    }

    fn update(&mut self, message: &Message) -> elm::Cmd<Message> {
        match message {
            Message::SetBufferName(new_name) => {
                self.buffer_name = new_name.clone();
                elm::Cmd::none()
            }

            Message::ExternalEvent(event) =>
                self.event_occurred(event),

            Message::SizedChanged(size) =>
                self.navigation.screen_size_changed(size.clone()),
        }
    }

    fn view(&self, display: &Self::View) -> io::Result<()> {
        self.render(&mut display.rendering_buffer())
    }

}

impl From<event::Event> for Message {
    /* This thing could be smarter; it could re-map the key-events to something
       more easily processable. */
    fn from(value: event::Event) -> Self {
        Message::ExternalEvent(value)
    }
}

fn main() -> io::Result<()> {
    let args = std::env::args();
    println!("Args: {:?}", args);

    let out = io::BufWriter::with_capacity(16384, io::stdout());
    tui::Screen::attach(out)?
        .enter_raw_mode()?
        .run_automat::<Editor>()
}
