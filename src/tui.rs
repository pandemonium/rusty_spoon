use std::{io, cell::RefCell, cell::RefMut};
use std::io::Write;
use std::{time, fmt};

use crossterm::{Command, event, terminal, QueueableCommand};

use crate::elm;


impl elm::Host for Screen {
    type Event = event::Event;
    type Display = Self;

    /* I dunno, man. */
    fn get_screen_buffer(&self) -> &Self::Display { &self }

    fn poll_events(&self) -> io::Result<Self::Event> {
        if event::poll(time::Duration::from_millis(5427))? {
            event::read()
        } else {
            Err(io::Error::new(io::ErrorKind::TimedOut, "Timed out waiting for the world."))
        }
    }

    fn commit_screen_buffer(&self, buffer: &Self::Display) -> io::Result<()> {
        buffer.flush()
    }
}

pub struct CommandBuffer<'a>(RefMut<'a, dyn io::Write>);

impl <'a> CommandBuffer<'a> {
    fn new(cell: &'a RefCell<dyn io::Write>) -> Self {
        Self(cell.borrow_mut())
    }

    pub fn queue(&mut self, command: impl Command) -> io::Result<&mut (dyn io::Write + 'a)> {
        self.0.queue(command)
    }
}


#[derive(Clone, Debug)]
pub struct Size {
    pub width:  u16,
    pub height: u16,
}

impl Size {
    fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

pub struct Screen {
    inner: Box<RefCell<dyn io::Write>>,
    dimensions: Size,
}

impl Screen {
    pub fn attach<W: Write + 'static>(out: W) -> io::Result<Self> {
        Ok(Self {
            inner: Box::new(RefCell::new(out)),
            dimensions: Self::get_terminal_size()?,
        })
    }

    fn get_terminal_size() -> io::Result<Size> {
        let (columns, rows) = terminal::size()?;
        Ok(Size::new(columns, rows))
    }

    pub fn dimensions(&self) -> &Size {
        &self.dimensions
    }

    pub fn enter_raw_mode(self) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(self)
    }

    pub fn draw_buffer(&self) -> CommandBuffer {
        CommandBuffer::new(&self.inner)
    }

    pub fn flush(&self) -> io::Result<()> {
        self.inner.borrow_mut().flush()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable!")
    }
}