#![feature(conservative_impl_trait)]
#![feature(ascii_ctype)]
#![feature(ord_max_min)]

/// TODO(outdated)
///
/// * save & resume program state
/// * max_mem_grid_size
/// * max_call_stack_size
/// * max_threads
///
/// * what do if skip would shoot you out of bounds?
/// * what do if there's no start point?

extern crate rand;

use std::fmt::{self, Display};
use std::io;
use std::str::{self, FromStr};

pub type Coord = (usize, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Right,
    Down,
    Left,
    Up,
}

impl Default for Direction {
    fn default() -> Self { Direction::Right }
}

impl Direction {
    /// Advances a coordinate (in this direction) by a certain distance.
    ///
    /// Returns `None` if result would be left-of/above-of `(0,0)`.
    ///
    /// TODO should return Result
    pub fn advance(self, mut c: Coord, by: usize) -> Option<Coord> {
        match self {
            Direction::Right => c.0 += by,
            Direction::Down => c.1 += by,
            Direction::Left => {
                match c.0.checked_sub(by) {
                    Some(v) => c.0 = v,
                    None => return None,
                }
            },
            Direction::Up => {
                match c.1.checked_sub(by) {
                    Some(v) => c.1 = v,
                    None => return None,
                }
            },
        }
        Some(c)
    }
}


// pub enum Error {
//     NoStart,
//     InvalidChar(u8),
//     OutOfBounds,
//     OutOfMemory,
// }

// ++++++++++++++++++++ instructions ++++++++++++++++++++

pub mod inst {
    use std::ascii::AsciiExt;

    pub const BLANK: u8 = b' ';
    pub const START: u8 = b'$';

    pub const LEFT: u8 = b'<';
    pub const RIGHT: u8 = b'>';
    pub const INCR: u8 = b'+';
    pub const DECR: u8 = b'-';
    pub const READ: u8 = b',';
    pub const WRITE: u8 = b'.';
    pub const LURD: u8 = b'\\';
    pub const RULD: u8 = b'/';
    pub const SKIP: u8 = b'!';
    pub const SKIPZ: u8 = b'?';

    pub const ENTER: u8 = b'@';
    pub const LEAVE: u8 = b'#';

    pub const UP: u8 = b':';
    pub const DOWN: u8 = b';';
    pub const SPLIT: u8 = b'&';
    pub const RAND: u8 = b'%';

    pub fn is_valid(i: u8) -> bool { i == b' ' || i.is_ascii_graphic() }
}


// ++++++++++++++++++++ CodeGrid ++++++++++++++++++++

#[derive(Default, Debug, Clone)]
pub struct CodeGrid {
    rows: Vec<Vec<u8>>,
}

impl FromStr for CodeGrid {
    type Err = Box<::std::error::Error>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut width = 0;
        let mut rows = vec![];

        for line in s.lines() {
            if !line.as_bytes().iter().all(|&i| inst::is_valid(i)) {
                return Err("Invalid character".into()); // TODO
            }
            rows.push(line.as_bytes().to_owned());
            width = width.max(line.as_bytes().len());
        }

        for row in &mut rows {
            row.resize(width, inst::BLANK)
        }

        Ok(Self { rows })
    }
}

impl Display for CodeGrid {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for row in self.rows() {
            writeln!(fmt, "{}", str::from_utf8(row).unwrap())?;
        }
        Ok(())
    }
}

impl CodeGrid {
    pub fn get(&self, c: Coord) -> Option<u8> {
        self.rows.get(c.1).and_then(|r| r.get(c.0)).map(|i| *i)
    }
    pub fn set(&mut self, c: Coord, i: u8) {
        assert!(inst::is_valid(i));
        self.rows[c.1][c.0] = i;
    }

    pub fn size(&self) -> Coord {
        (self.rows.len(), self.rows.get(0).map(|r| r.len()).unwrap_or(0))
    }
    pub fn resize(&mut self, c: Coord) {
        self.rows.resize(c.1, vec![]);

        for row in &mut self.rows {
            row.resize(c.0, inst::BLANK);
        }
    }

    pub fn reset(&mut self) {
        for i in self.rows.iter_mut().flat_map(|row| row) {
            *i = inst::BLANK
        }
    }

    /// TODO should return Result
    pub fn find_start(&self) -> Option<Coord> {
        let mut start = (!0, !0);
        let mut explicit = false;

        for (y, row) in self.rows.iter().enumerate() {
            for (x, &i) in row.iter().enumerate() {
                match i {
                    inst::START => {
                        if !explicit || (x < start.0 && y < start.1) {
                            start = (x, y);
                            explicit = true;
                        }
                    },
                    inst::BLANK => {},
                    _ => {
                        if !explicit && (x < start.0 && y < start.1) {
                            start = (x, y);
                        }
                    },
                }
            }
        }

        if start != (!0, !0) { Some(start) } else { None }
    }

    pub fn rows(&self) -> &[Vec<u8>] { &self.rows }
}

// ++++++++++++++++++++ MemoryGrid ++++++++++++++++++++

/// TODO mem_limit
#[derive(Default, Debug, Clone)]
pub struct MemoryGrid {
    rows: Vec<Vec<u32>>,
}

impl MemoryGrid {
    pub fn get(&self, c: Coord) -> u32 {
        *self.rows
             .get(c.1)
             .and_then(|r| r.get(c.0))
             .unwrap_or(&0)
    }
    pub fn entry(&mut self, c: Coord) -> &mut u32 {
        if c.1 >= self.rows.len() {
            self.rows.resize(c.1 + 1, vec![])
        }
        let row = &mut self.rows[c.1];
        if c.0 >= row.len() {
            row.resize(c.0 + 1, 0)
        }
        &mut row[c.0]
    }
    pub fn set(&mut self, c: Coord, v: u32) { *self.entry(c) = v }

    pub fn reset(&mut self) { self.rows.clear() }

    pub fn rows(&self) -> &[Vec<u32>] { &self.rows }
}

// ++++++++++++++++++++ Program ++++++++++++++++++++

pub type Stdin = Fn() -> io::Result<u8>;
pub type Stdout = Fn(u8) -> io::Result<()>;

#[derive(Debug, Clone)]
pub struct Thread {
    inst_ptr: Coord,
    mem_ptr: Coord,
    call_stack: Vec<Coord>,
    dir: Direction,
}

impl Thread {
    pub fn start(inst_ptr: Coord) -> Self {
        Self {
            inst_ptr,
            mem_ptr: (0, 0),
            call_stack: vec![],
            dir: Default::default(),
        }
    }
    fn create_child(&self, inst_ptr: Coord) -> Self {
        Self {
            inst_ptr,
            mem_ptr: self.mem_ptr,
            call_stack: vec![],
            dir: self.dir,
        }
    }

    pub fn instruction_pointer(&self) -> Coord { self.inst_ptr }
    pub fn memory_pointer(&self) -> Coord { self.mem_ptr }
    pub fn call_stack(&self) -> &[Coord] { &self.call_stack }
    pub fn direction(&self) -> Direction { self.dir }

    pub fn step(
        &mut self,
        code: &CodeGrid,
        mem: &mut MemoryGrid,
        stdin: &mut Stdin,
        stdout: &mut Stdout,
        spawn: &mut FnMut(Thread),
    ) -> Result<Option<u32>, Box<::std::error::Error>> {
        let mut advance_by = 1;

        match code.get(self.inst_ptr).unwrap() {
            inst::LEFT => {
                match Direction::Left.advance(self.mem_ptr, 1) {
                    Some(c) => self.mem_ptr = c,
                    None => return Err("Out Of Bounds".into()), // TODO
                }
            },
            inst::RIGHT => self.mem_ptr.0 += 1,

            inst::INCR => {
                let v = mem.entry(self.mem_ptr);
                *v = v.wrapping_add(1);
            },
            inst::DECR => {
                let v = mem.entry(self.mem_ptr);
                *v = v.wrapping_sub(1);
            },

            inst::READ => {
                match stdin() {
                    Ok(b) => mem.set(self.mem_ptr, b as u32),
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => advance_by = 0,
                    Err(e) => return Err(e.into()),
                }
            },
            inst::WRITE => {
                let v = mem.get(self.mem_ptr);
                // TODO print a warning if value exceeds !0u8
                match stdout(v as u8) {
                    Ok(()) => {},
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => advance_by = 0,
                    Err(e) => return Err(e.into()),
                }
            },
            // inst::READ => {
            //     let b = &mut [0u8];
            //     if stdin.read(b)? == 1 {
            //         mem.set(self.mem_ptr, b[0] as u32)
            //     }
            // }
            // inst::WRITE => {
            //     let b = mem.get(self.mem_ptr) as u8;
            //     if stdout.write(&[b])? != 1 {
            //         advance_by = 0;
            //     }
            // }
            inst::LURD => {
                match self.dir {
                    Direction::Right => self.dir = Direction::Down,
                    Direction::Down => self.dir = Direction::Right,
                    Direction::Left => self.dir = Direction::Up,
                    Direction::Up => self.dir = Direction::Left,
                }
            },
            inst::RULD => {
                match self.dir {
                    Direction::Right => self.dir = Direction::Up,
                    Direction::Down => self.dir = Direction::Left,
                    Direction::Left => self.dir = Direction::Down,
                    Direction::Up => self.dir = Direction::Right,
                }
            },

            inst::SKIP => advance_by = 2,
            inst::SKIPZ => {
                if mem.get(self.mem_ptr) == 0 {
                    advance_by = 2
                }
            },

            inst::ENTER => self.call_stack.push(self.inst_ptr),
            inst::LEAVE => {
                match self.call_stack.pop() {
                    Some(c) => {
                        self.inst_ptr = c;
                        advance_by = 2
                    },
                    None => return Ok(Some(mem.get(self.mem_ptr))),
                }
            },

            inst::UP => {
                match Direction::Up.advance(self.mem_ptr, 1) {
                    Some(c) => self.mem_ptr = c,
                    None => return Err("Out Of Bounds".into()), // TODO
                }
            },
            inst::DOWN => self.mem_ptr.1 += 1,

            inst::SPLIT => {
                if let Some(c) = self.dir.advance(self.inst_ptr, 1) {
                    if code.get(c).is_some() {
                        spawn(self.create_child(c));
                        advance_by = 2;
                    }
                }
            },
            inst::RAND => *mem.entry(self.mem_ptr) = rand::random(),

            _ => {},
        }

        match self.dir.advance(self.inst_ptr, advance_by) {
            // if still within code-grid, thread goes on...
            Some(c) if code.get(c).is_some() => {
                self.inst_ptr = c;
                Ok(None)
            },
            // ...otherwise, thread is finished.
            _ => Ok(Some(mem.get(self.mem_ptr))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    code: CodeGrid,
    mem: MemoryGrid,
    threads: Vec<Thread>,
}

impl Program {
    pub fn new(code: CodeGrid) -> Self {
        Self {
            code,
            mem: Default::default(),
            threads: vec![],
        }
    }

    pub fn code(&self) -> &CodeGrid { &self.code }
    pub fn memory(&self) -> &MemoryGrid { &self.mem }
    pub fn threads(&self) -> &[Thread] { &self.threads }

    pub fn step(
        &mut self,
        stdin: &mut Stdin,
        stdout: &mut Stdout,
    ) -> Result<Option<u32>, Box<::std::error::Error>> {
        if self.threads.is_empty() {
            let start = self.code.find_start().unwrap(); // TODO what do if start not found?
            self.threads.push(Thread::start(start));
        }

        let mut new_threads: Vec<Thread> = vec![];

        for idx in (0..self.threads.len()).rev() {
            let exit = self.threads[idx]
                .step(&self.code,
                      &mut self.mem,
                      stdin,
                      stdout,
                      &mut |tc| new_threads.push(tc))?;

            if let Some(exit) = exit {
                self.threads.remove(idx);
                if self.threads.is_empty() {
                    self.mem.reset();
                    return Ok(Some(exit));
                }
            }
        }

        self.threads.extend(new_threads);

        Ok(None)
    }
}
