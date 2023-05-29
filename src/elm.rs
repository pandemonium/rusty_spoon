use std::{io, net::TcpStream};

use crossterm::terminal;

/* I want to be able to subscribe. */
pub enum Cmd<Msg: Clone> {
    None,
    Suspend(Box<dyn FnOnce() -> io::Result<Msg>>),
    Dispatch(Msg),
    AndThen(Box<Cmd<Msg>>, Box<Cmd<Msg>>),
    Gtfo,
}

impl <Msg: Clone> Cmd<Msg> {
    pub fn none() -> Self { Cmd::None }

    pub fn suspend<F>(effect: F) -> Cmd<Msg> 
    where
        F: FnOnce() -> io::Result<Msg> + Sized + 'static,
    {
        Cmd::Suspend(Box::new(effect))
    }

    pub fn dispatch(message: Msg) -> Cmd<Msg> { Cmd::Dispatch(message) }

    pub fn and_then(self, then: Cmd<Msg>) -> Cmd<Msg> {
        Cmd::AndThen(Box::new(then), Box::new(self))
    }

    pub fn gtfo() -> Self { Cmd::Gtfo }
}

pub trait Application: Sized {
    type Msg: Clone;
    type View;

    fn init() -> (Self, Cmd<Self::Msg>);

    fn update(&mut self, msg: &Self::Msg) -> Cmd<Self::Msg>;

    fn view(&self, out: &Self::View) -> io::Result<()>;
}

pub fn request_size<F, Msg: Clone>(to_msg: F) -> Cmd<Msg> 
where
    F: FnOnce(u16, u16) -> Msg + 'static
{
    Cmd::suspend(|| {
        let (width, height) = terminal::size()?;
        Ok(to_msg(width, height))
    })
}

#[derive(Clone, Debug)]
pub enum Resource<A> {
    Unknown,
    Present(A),
    Failed(String),
}

impl <A> Resource<A> {
    pub fn fetch<F, G, Msg>(effect: F, as_msg: G) -> Cmd<Msg> 
    where 
        F: FnOnce() -> io::Result<A> + Sized + 'static,
        G: FnOnce(Self) -> Msg + 'static,
        Msg: Clone,
    {
        Cmd::suspend(||
            match effect() {
                Ok(a)  => Ok(as_msg(Resource::Present(a))),
                Err(e) => Ok(as_msg(Resource::Failed(e.to_string()))),
            }
        )
    }

    fn present(&self) -> Option<&A> {
        match self {
            Self::Present(x) => Some(x),
            _otherwise       => None,
        }
    }
}

impl <A> Default for Resource<A> {
    fn default() -> Self { Self::Unknown }
}

pub trait Host {
    type Event;
    type Display;

    fn poll_events(&self) -> io::Result<Self::Event>;

    fn commit_screen_buffer(&self, buffer: &Self::Display) -> io::Result<()>;

    fn get_screen_buffer(&self) -> &Self::Display;

    fn run_automat<App>(&self) -> io::Result<()>
    where 
        App: Application<View = Self::Display>,
        App::Msg: From<Self::Event>
    {
        let (mut model, mut cmd) = App::init();
        let mut cmd_stack = vec![];

        /* The trio of .get_screen_buffer, .view, and .commit_xxx
           could probably be summed up with CommandBuffer to make 
           it more principled. */
        let screen = self.get_screen_buffer();

        loop {
            model.view(&screen)?;
            self.commit_screen_buffer(&screen)?;

            cmd = match cmd {
                Cmd::Suspend(effect)     => model.update(&effect()?),
                Cmd::Dispatch(msg)       => model.update(&msg),
                Cmd::Gtfo                => break Ok(()),
                Cmd::AndThen(this, that) => {
                    cmd_stack.push(this);
                    *that
                }
                Cmd::None => {
                    if let Some(cmd) = cmd_stack.pop() { *cmd } else {
                        /* Some of these events are interesting on this level; resize,
                           for instance, must update Screen.dimensions.

                           Focus gained and lost are probably also interesting. */
                        model.update(&self.poll_events().map(&App::Msg::from)?)
                    }
                }
            };
        }
    }
}