use std::{cmp, fmt, io};

mod elm;
use elm::Host;

mod tui;

/* Make a crossterm prelude for the elm module? */
use crossterm::{cursor, event, event::{KeyCode, KeyModifiers}, style, QueueableCommand, terminal};
use tui::RenderingBuffer;

#[derive(Clone, Debug)]
struct ScreenSize {
    columns: u16,
    rows:    u16,
}

impl ScreenSize {
    fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }

    fn request() -> elm::Cmd<Message> {
        elm::request_size(|width, height|
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
        ScreenSize::new(value.0, value.1)
    }
}

struct EditingViewport {
    line_offset:   usize,
    column_offset: usize,
}

impl EditingViewport {
    fn select_and_clip<'a>(
        &self,
        line_index: usize, 
        width:      usize, 
        lines:      &'a[String]
    ) -> Option<&'a str> {
        let effective_line_index = self.line_offset + line_index;
        if effective_line_index < lines.len() {
            let line = &lines[effective_line_index];
            let effective_width = cmp::min(width, line.len());
            let slice = self.column_offset..self.column_offset + effective_width;
            Some(&line[slice])
        } else {
            None
        }
    }
}

impl Default for EditingViewport {
    fn default() -> Self {
        Self { line_offset: Default::default(), column_offset: Default::default() }
    }
}

struct EditingModel {
    lines: Vec<String>,
    viewport: EditingViewport,
}

impl EditingModel {
    fn new() -> Self {
        Self {
            lines: vec![
                "hi, mom".into(),
                "Hello, world".into(),
            ],
            viewport: Default::default(),
        }
    }

    fn line_count(&self) -> usize { self.lines.len() }

    fn line_at(&self, index: usize, width: usize) -> Option<&str> {
        self.viewport.select_and_clip(index, width, self.lines.as_slice())
    }
}

impl Default for EditingModel {
    fn default() -> Self {
        Self::new()
    }
}

struct CursorModel {
    column: u16, row: u16,
    screen_bounds: ScreenSize,
}

impl CursorModel {
    fn move_intended(&mut self, direction: &KeyCode) {
        match direction {
            KeyCode::Up    => self.row    = self.row.saturating_sub(1),
            KeyCode::Left  => self.column = self.column.saturating_sub(1),
            KeyCode::Down  => self.row    += 1,
            KeyCode::Right => self.column += 1,
            _              => unimplemented!(),
        }
    }

    fn bounds_changed(&mut self, new_size: ScreenSize) -> elm::Cmd<Message> {
        self.screen_bounds = new_size;
        elm::Cmd::none()
    }
}

impl Default for CursorModel {
    fn default() -> Self {
        Self {
            column: Default::default(), 
            row: Default::default(),
            screen_bounds: Default::default(),
        }
    }
}

struct Editor {
    buffer_name: String,
    contents:    EditingModel,
    cursor:      CursorModel,
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
                code:      direction @ (KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.cursor.move_intended(direction);
                elm::Cmd::none()
            }

            _otherwise => elm::Cmd::none(),
        }
    }

    fn handle_event(&mut self, event: &event::Event) -> elm::Cmd<Message> {
        match event {
            event::Event::Key(key) =>
                self.key_typed(key),
            event::Event::Resize(width, height) =>
                self.cursor.bounds_changed((*width, *height).into()),
            _otherwise =>
                elm::Cmd::none(),
        }
    }

    fn render(&self, buffer: &mut RenderingBuffer) -> io::Result<()> {
        let cursor_bounds = &self.cursor.screen_bounds;

        /* At least consider putting the draw methods behind some
           trait to cut down on the amount of code clutter. */

        buffer
           .queue(cursor::Hide)?
           .queue(cursor::MoveTo(0, 0))?;

        self.render_contents(buffer)?;

        let message = format!("Cursor bounds: {:?}", cursor_bounds);

        buffer
            .queue(cursor::MoveTo(5, 10))?
            .queue(style::Print(message))?
            .queue(cursor::MoveTo(self.cursor.column, self.cursor.row))?
            .queue(cursor::Show)?;

        Ok(())
    }

    fn render_contents(&self, buffer: &mut RenderingBuffer) -> io::Result<()> {
        let cursor_bounds = &self.cursor.screen_bounds;
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

    fn render_line(&self, i: usize) -> &str {
        let line_width = self.cursor.screen_bounds.columns as usize;
        self.contents.line_at(i, line_width).unwrap_or("~")
    }


}

impl Default for Editor {
    fn default() -> Self {
        Self {
            buffer_name: "Unnamed".to_owned(),
            contents: EditingModel::default(),
            cursor: CursorModel::default(),
        }
    }
}

#[derive(Clone)]
enum Message {
    SetBufferName(String),
    ExternalEvent(event::Event),
    SizedChanged(ScreenSize),
}

impl elm::Application for Editor {
    type Msg = Message;
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
                self.handle_event(event),

            Message::SizedChanged(size) =>
                self.cursor.bounds_changed(size.clone()),
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
    let out = io::BufWriter::with_capacity(16384, io::stdout());
    tui::Screen::attach(out)?
        .enter_raw_mode()?
        .run_automat::<Editor>()
}
