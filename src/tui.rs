use std::{io, cell::RefCell, cell::RefMut};
use std::io::Write;
use std::time;

use crossterm::{Command, event, terminal, QueueableCommand};

use crate::elm;


pub fn request_terminal_size<F, Msg: Clone>(to_msg: F) -> elm::Cmd<Msg> 
where
    F: FnOnce(u16, u16) -> Msg + 'static
{
    elm::Cmd::suspend(|| {
        let (width, height) = terminal::size()?;
        Ok(to_msg(width, height))
    })
}

impl elm::Host for Screen {
    type Event = event::Event;
    type Display = Self;

    /* I dunno, man. */
    fn get_display(&self) -> &Self::Display { &self }

    fn poll_events(&self) -> io::Result<Self::Event> {
        if event::poll(time::Duration::from_millis(5427))? {
            event::read()
        } else {
            Err(io::Error::new(io::ErrorKind::TimedOut, "Timed out waiting for the world."))
        }
    }

    fn flush(&self, display: &Self::Display) -> io::Result<()> {
        display.commit()
    }
}

pub struct RenderingBuffer<'a>(RefMut<'a, dyn io::Write>);

impl <'a> RenderingBuffer<'a> {
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

    pub fn rendering_buffer(&self) -> RenderingBuffer {
        RenderingBuffer::new(&self.inner)
    }

    pub fn commit(&self) -> io::Result<()> {
        self.inner.borrow_mut().flush()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable!")
    }
}