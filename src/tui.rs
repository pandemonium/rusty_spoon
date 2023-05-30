use std::{io, cell::RefCell, cell::RefMut};
use std::io::Write;
use std::time;

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

pub struct Screen {
    inner: Box<RefCell<dyn io::Write>>,
}

impl Screen {
    pub fn attach<W: Write + 'static>(out: W) -> io::Result<Self> {
        Ok(Self {
            inner: Box::new(RefCell::new(out)),
        })
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