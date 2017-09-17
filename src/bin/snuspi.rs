extern crate snusp;

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process::{self, Command};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// Portable non-blocking stdin is a tricky thing to do, so (for now) we emulate
// it by spawning
// a worker thread, solely dedicated to blocking reads on stdin.
fn spawn_stdin_worker() -> Box<snusp::Stdin> {
    let (tx, rx) = mpsc::channel();

    let mut iter = io::stdin().bytes();
    thread::spawn(move || {
        // TODO? could do system(/bin/stty raw) here to get rid of those
        // enter-presses...

        while tx.send(iter.next().unwrap()).is_ok() { /* */ }
    });

    Box::new(move || match rx.try_recv() {
                 Ok(x) => x,
                 _ => Err(io::ErrorKind::WouldBlock.into()),
             })
}

fn stdout(b: u8) -> io::Result<()> {
    let mut out = io::stdout();
    out.write(&[b])?;
    let ret = out.flush();

    // TODO? make duration configurable
    thread::sleep(Duration::from_millis(100));

    ret
}

fn main() {
    let path = env::args()
        .skip(1)
        .next()
        .expect("Usage: snuspi [file]");

    let mut source = String::new();
    {
        let mut file = fs::File::open(path).expect("Fatal error while opening file:");
        file.read_to_string(&mut source)
            .expect("Fatal error while reading file:");
    }
    let code: snusp::CodeGrid = source
        .parse()
        .expect("Fatal error while parsing source:");
    let mut program = snusp::Program::new(code);

    let mut stdin = spawn_stdin_worker();
    loop {
        match program.step(&mut *stdin, &mut stdout) {
            Ok(Some(exit)) => {
                // print newline (snusp programs don't normally do this), flush stdout
                println!("");

                // TODO should be toggleable by a cmd-flag
                // println!("EXIT: {}", exit);

                // TODO what do if exit is -1?
                process::exit(exit as i32);
            },
            Ok(None) => {},
            Err(err) => {
                eprintln!("Fatal error during runtime: {}", err);
                process::exit(-1)
            },
        }

        // TODO make duration configurable
        thread::sleep(Duration::from_millis(1));
    }
}
