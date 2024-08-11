#![no_main]

use std::{io, fs, path, thread};
use core::time;

mod cli;
mod parser;

c_ffi::c_main!(rust_main);

fn construct_file_path(dir: &str, title: &str) -> path::PathBuf {
    let mut name = String::new();
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            name.push(ch);
        } else if ch.is_whitespace() {
            name.push('_');
        } else {
            continue
        }
    }

    let mut path = path::PathBuf::from(dir);
    path.push(name);
    path.set_extension("md");

    path
}

fn rust_main(args: c_ffi::Args) -> bool {
    let args = match cli::Cli::new(args.into_iter().skip(1)) {
        Ok(args) => args,
        Err(code) => return code,
    };

    let retry_delay = time::Duration::from_secs(args.retry_delay);
    let chapters = match parser::ChapterList::new(args.novel, retry_delay) {
        Ok(chapters) => chapters,
        Err(error) => {
            eprint!("{error}");
            return false;
        }
    };
    println!("Title: {}", chapters.proper_title);

    let mut file = match fs::File::create(construct_file_path(".", &chapters.proper_title)) {
        Ok(file) => io::BufWriter::new(file),
        Err(error) => {
            eprintln!("Failed to create file to store content. Error: {}", error);
            return false;
        },
    };

    let result = io::Write::write_fmt(&mut file, format_args!("# {}\n\nOriginal: https://www.lightnovelworld.com/novel/{}\n\n", chapters.proper_title, chapters.iter.title));
    if let Err(error) = result {
        eprintln!("Unable to write file: {}", error);
        return false;
    }

    'chapters: for chapter in chapters.iter {
        print!(">>>{}: Downloading...", chapter.title);

        'write_chapter: loop {
            if let Err(error) = chapter.write_chapter(&mut file) {
                println!("ERR\n{error}");
                match error {
                    parser::WriteError::Http(_) => {
                        println!("Retry in {}s...", retry_delay.as_secs());
                        thread::sleep(retry_delay);
                        continue 'write_chapter;
                    },
                    _ => break 'chapters,
                }
            } else {
                break 'write_chapter;
            }
        }
        println!("OK");
    }

    let _ = io::Write::flush(&mut file);

    true
}
