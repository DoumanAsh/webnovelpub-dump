#![no_main]

use std::{io, fs, path, thread};
use core::time;

mod cli;
mod parser;

c_ffi::c_main!(rust_main);

fn construct_file_path(dir: &str, name: &str) -> path::PathBuf {
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

    let chapters = match parser::ChapterList::new(args.novel) {
        Ok(chapters) => chapters,
        Err(error) => {
            eprint!("{error}");
            return false;
        }
    };
    println!("Title: {}", chapters.proper_title);

    let mut file = match fs::File::create(construct_file_path(".", &chapters.proper_title.replace(' ', "_"))) {
        Ok(file) => io::BufWriter::new(file),
        Err(error) => {
            eprintln!("Failed to create file to store content. Error: {}", error);
            return false;
        },
    };

    if let Err(error) = io::Write::write_fmt(&mut file, format_args!("# {}\n\nOriginal: https://www.webnovelpub.com/novel/{}\n\n", chapters.proper_title, chapters.iter.title)) {
        eprintln!("Unable to write file: {}", error);
        return false;
    }

    'chapters: for chapter in chapters.iter {
        print!(">>>{}: Downloading...", chapter.title);

        if let Err(error) = chapter.write_chapter(&mut file) {
            println!("ERR\n{error}");
            match error {
                parser::WriteError::Http(_) => {
                    println!("Retry in 5s...");
                    thread::sleep(time::Duration::from_secs(5));
                    continue 'chapters;
                },
                _ => break 'chapters,
            }
        }
        println!("OK");
    }

    let _ = io::Write::flush(&mut file);

    true
}
