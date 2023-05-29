use std::io;

mod elm;
use elm::Host;

mod tui;

/* Make a crossterm prelude for the elm module? */
use crossterm::{cursor, event, event::{KeyCode, KeyModifiers}, style, QueueableCommand, terminal};


struct Editor {
    name: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            name: "Unnamed".to_owned(),
        }
    }
}

#[derive(Clone)]
enum Message {
    SetName(String),
    ExternalEvent(event::Event),
}

impl elm::Application for Editor {
    type Msg = Message;
    type View = tui::Screen;

    fn init() -> (Self, elm::Cmd<Message>) {
        (Editor::default(), elm::Cmd::none())
    }

    fn update(&mut self, message: &Message) -> elm::Cmd<Message> {
        fn process_key(key: &event::KeyEvent) -> elm::Cmd<Message> {
            match key {
                event::KeyEvent {
                    code:      KeyCode::Char('q'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }          => elm::Cmd::gtfo(),
                _otherwise => elm::Cmd::none(),
            }
        }

        fn handle_event(event: &event::Event) -> elm::Cmd<Message> {
            match event {
                event::Event::Key(key) => process_key(key),
                _otherwise             => elm::Cmd::none(),
            }
        }

        match message {
            Message::SetName(new_name) => {
                self.name = new_name.clone();
                elm::Cmd::none()
            }

            Message::ExternalEvent(event) => 
                handle_event(event),
        }
    }

    fn view(&self, display: &Self::View) -> Result<(), std::io::Error> {
        let dim = display.dimensions();

        let mut buffer = display.command_buffer();

        /* Atleast consider putting the draw methods behind some
           trait to cut down on the amount of code clutter. */

        buffer
            .queue(terminal::Clear(terminal::ClearType::All))?
            .queue(cursor::MoveTo(0, 0))?;

        for i in 0..dim.height {
            buffer.queue(style::Print("~"))?;
            if i < dim.height - 1 {
                buffer.queue(style::Print("\r\n"))?;
            }
        }

        let message = format!("Hello, world [{}]", dim);

        buffer
            .queue(cursor::MoveTo(5, 10))?
            .queue(style::Print(message))?
            .queue(cursor::MoveTo(0, 0))?;

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
