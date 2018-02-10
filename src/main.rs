extern crate failure;
extern crate nix;
extern crate pty;
extern crate termion;

use failure::Error;
use pty::fork::Fork;
use std::io::{self, Read, Write};
use std::process::Command;
use termion::raw::IntoRawMode;

const DEFAULT_BUF_SIZE: usize = 64 * 1024; // Same as io::DEFAULT_BUF_SIZE

fn main() {
    if let Err(err) = do_main() {
        eprintln!("{}", err);
    }
}

fn do_main() -> Result<(), Error> {
    let fork = Fork::from_ptmx()?;
    let parent = fork.is_parent();
    if parent.is_err() {
        Command::new("tmux").arg("new").arg("dino").status()?;
        return Ok(());
    }
    let mut master = parent.unwrap();

    let stdout = io::stdout();
    stdout.lock();
    let mut stdout = stdout.into_raw_mode()?;

    let mut stdin = termion::async_stdin();

    let mut escape = None;
    let mut player = None;
    let mut obstacle = false;
    let mut line = 0;

    let mut buf = [0; DEFAULT_BUF_SIZE];
    loop {
        let len = stdin.read(&mut buf)?;
        if len > 0 {
            master.write_all(&buf[..len])?;
            master.flush()?;
        }
        let len = match master.read(&mut buf)? {
            0   => break,
            len => len,
        };
        stdout.write_all(&buf[..len])?;
        stdout.flush()?;

        for (col, byte) in buf.iter().enumerate() {
            let byte = *byte;
            if byte == b'\x1b' {
                escape = Some((col, Vec::with_capacity(4)));
            } else if let Some(&mut (col, ref mut escape)) = escape.as_mut() {
                escape.push(byte);
                if byte == b'm' {
                    if escape == b"[47m" {
                        player = Some(col);
                    } else if !obstacle && player.is_some() && escape == b"[41m" {
                        if let Some(dist) = col.checked_sub(player.unwrap()) {
                            if dist <= 30 {
                                master.write_all(&*if line == 16 {
                                    b"\x1b[A"
                                } else {
                                    b"\x1b[B"
                                })?;
                                master.flush()?;
                            }
                            obstacle = true;
                        }
                    }
                } else if byte == b'H' {
                    let line_string = escape.iter()
                            .skip(1)
                            .take_while(|b| **b != b';')
                            .map(|b| *b as char)
                            .filter(|c| c.is_digit(10))
                            .collect::<String>();
                    if !line_string.is_empty() {
                        line = line_string.parse().unwrap();
                    }
                    player = None;
                    obstacle = false;
                }
            }
            if byte == b'm' || byte == b'H' || byte == b'f' {
                escape = None;
            }
            if byte == b'g' { // game over
                master.write_all(&*b"q")?;
                master.flush()?;
            }
        }
    }
    fork.wait()?;
    Ok(())
}
