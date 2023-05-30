use std::{io, fmt};

mod elm;
use elm::Host;

mod tui;

/* Make a crossterm prelude for the elm module? */
use crossterm::{cursor, event, event::{KeyCode, KeyModifiers}, style, QueueableCommand, terminal};

#[derive(Clone, Debug)]
struct Size {
    width:  u16,
    height: u16,
}

impl Size {
    fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    fn request() -> elm::Cmd<Message> {
        elm::request_size(|width, height|
            Message::SizedChanged((width, height).into())
        )
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Default for Size {
    fn default() -> Self {
        Self { width: Default::default(), height: Default::default() }
    }
}

impl From<(u16, u16)> for Size {
    fn from(value: (u16, u16)) -> Self {
        Size::new(value.0, value.1)
    }
}

struct EditorContents {
    contents: String,
}

impl EditorContents {
    fn push_letter(&mut self, letter: char) {
        self.contents.push(letter);
    }

    fn push_string(&mut self, text: &str) {
        self.contents.push_str(text);
    }
}

impl Default for EditorContents {
    fn default() -> Self {
        Self { contents: Default::default() }
    }
}

struct CursorController {
    column: u16, row: u16,
    screen_bounds: Size,
}

impl CursorController {
    fn move_intended(&mut self, direction: &KeyCode) {
        match direction {
            KeyCode::Up    => self.row    -= 1,
            KeyCode::Left  => self.column -= 1,
            KeyCode::Down  => self.row    += 1,
            KeyCode::Right => self.column += 1,
            _              => unimplemented!(),
        }
    }

    fn bounds_changed(&mut self, new_size: Size) -> elm::Cmd<Message> {
        self.screen_bounds = new_size;
        elm::Cmd::none()
    }
}

impl Default for CursorController {
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
    contents:    EditorContents,
    cursor:      CursorController,
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
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            buffer_name: "Unnamed".to_owned(),
            contents: EditorContents::default(),
            cursor: CursorController::default(),
        }
    }
}

#[derive(Clone)]
enum Message {
    SetBufferName(String),
    ExternalEvent(event::Event),
    SizedChanged(Size),
}

impl elm::Application for Editor {
    type Msg = Message;
    type View = tui::Screen;

    fn init() -> (Self, elm::Cmd<Message>) {
        (Editor::default(), Size::request())
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

    fn view(&self, display: &Self::View) -> Result<(), io::Error> {
        /* This is sub-par and requires more thought. */
        let cursor_bounds = &self.cursor.screen_bounds;

        let mut buffer = display.draw_buffer();

        /* At least consider putting the draw methods behind some
           trait to cut down on the amount of code clutter. */

        buffer
            .queue(cursor::Hide)?
            .queue(cursor::MoveTo(0, 0))?;

        for i in 0..cursor_bounds.height  {
            buffer.queue(style::Print("~"))?
                  .queue(terminal::Clear(terminal::ClearType::UntilNewLine))?;

            if i < cursor_bounds.height - 1 {
                buffer.queue(style::Print("\r\n"))?;
            }
        }

        let message = format!("Cursor bounds: {:?}", cursor_bounds);

        buffer
            .queue(cursor::MoveTo(5, 10))?
            .queue(style::Print(message))?
            .queue(cursor::MoveTo(self.cursor.column, self.cursor.row))?
            .queue(cursor::Show)?;

        Ok(())
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
    let out = io::BufWriter::new(io::stdout());
    tui::Screen::attach(out)?
        .enter_raw_mode()?
        .run_automat::<Editor>()
}
