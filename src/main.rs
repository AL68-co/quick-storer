#![warn(missing_docs)]

//! Defines the way our files are stored

use lazy_static::lazy_static;
use mla::{config::ArchiveWriterConfig, ArchiveReader, ArchiveWriter, Layers};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, ErrorKind, Write},
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "QuickStorer")]
struct CLIOptions {
    /// Increases verbosity of the program
    #[structopt(short, long)]
    verbose: bool,
    /// The input file
    #[structopt(parse(from_os_str), min_values(1), required(true))]
    input: Vec<PathBuf>,
    /// Forces the rewrite of files
    #[structopt(short, long)]
    force: bool,
}

lazy_static! {
    static ref CLI_OPTIONS: CLIOptions = CLIOptions::from_args();
}

mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut to_load = vec![];
    let mut to_store = vec![];
    for input in &CLI_OPTIONS.input {
        if CLI_OPTIONS.verbose {
            println!("{}", input.display());
        }
        if input.is_dir() {
            to_store.push(input);
        } else if !input.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The argument is not a file nor a directory",
            )
            .into());
        } else {
            match input.extension() {
                Some(ext) => {
                    if ext == OsString::from(config::EXTENSION).as_os_str() {
                        to_load.push(input);
                    } else {
                        to_store.push(input);
                    }
                }
                None => to_store.push(input),
            };
        }
    }

    load(to_load)?;
    store(to_store)?;

    Ok(())
}

fn load(input_vec: Vec<impl AsRef<Path>>) -> Result<(), Box<dyn std::error::Error>> {
    for input in input_vec {
        let input_file = BufReader::new(File::open(input.as_ref())?);
        let mut archive_reader = ArchiveReader::new(input_file)?;
        let iter = archive_reader.list_files()?.cloned().collect::<Vec<_>>();
        let nb_of_files = iter.len();
        for (i, file_string) in iter.into_iter().enumerate() {
            let file_path = {
                let mut bd = input.as_ref().to_path_buf();
                bd.pop();
                let mut path = bd.as_os_str().to_owned();
                path.push(&file_string);
                PathBuf::from(path)
            };
            println!(
                "File stored at {} extracted to {} (file {} of {})",
                &file_string,
                file_path.display(),
                i + 1,
                nb_of_files
            );
            let openoptions = {
                let mut o = OpenOptions::new();
                o.create_new(!CLI_OPTIONS.force)
                    .create(true)
                    .truncate(true)
                    .write(true);
                o
            };

            let mut output_file = BufWriter::new(match openoptions.open(&file_path) {
                Ok(v) => v,
                Err(e) => {
                    if e.kind() == ErrorKind::NotFound {
                        std::fs::create_dir_all(file_path.parent().unwrap())?;
                        openoptions.open(&file_path)?
                    } else {
                        return Err(e.into());
                    }
                }
            });
            let mut file = archive_reader.get_file(file_string.clone())?.unwrap().data;
            io::copy(&mut file, &mut output_file)?;
        }
    }
    Ok(())
}

fn store(input_vec: Vec<impl AsRef<Path>>) -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from({
        let mut output = input_vec[0].as_ref().as_os_str().to_owned();
        output.push(format!(".{}", config::EXTENSION));
        output
    });
    println!("{}", output.display());
    let output_file = BufWriter::new(
        OpenOptions::new()
            .create_new(!CLI_OPTIONS.force)
            .create(true)
            .truncate(true)
            .write(true)
            .open(&output)?,
    );
    let archive_writer_config = {
        let mut awc = ArchiveWriterConfig::new();
        awc.set_layers(Layers::COMPRESS)
            .with_compression_level(11)?;
        awc
    };
    let mut archive_writer = ArchiveWriter::from_config(output_file, archive_writer_config)?;
    for input in input_vec {
        traverse_dir_and_store(input, &PathBuf::from(""), &mut archive_writer)?;
    }

    archive_writer.finalize()?;
    archive_writer.flush()?;
    Ok(())
}

fn store_file<W: Write>(
    path: impl AsRef<Path>,
    root: impl AsRef<Path>,
    archive_writer: &mut ArchiveWriter<W>,
) -> Result<(), Box<dyn std::error::Error>> {
    let input_file_handler = BufReader::new(File::open(&path)?);
    let file_path = {
        let mut p = root.as_ref().as_os_str().to_owned();
        p.push("/");
        p.push(path.as_ref().file_name().unwrap());
        p
    };
    archive_writer.add_file(
        &file_path.to_string_lossy(),
        path.as_ref().metadata()?.len(),
        input_file_handler,
    )?;
    Ok(())
}

fn traverse_dir_and_store<W: Write>(
    path: impl AsRef<Path>,
    root: impl AsRef<Path>,
    archive_writer: &mut ArchiveWriter<W>,
) -> Result<(), Box<dyn std::error::Error>> {
    let iter = match path.as_ref().read_dir() {
        Ok(i) => i.flatten().collect::<Vec<_>>(),
        Err(_) => {
            return store_file(path, root, archive_writer);
        }
    };
    for file in iter {
        if file.path().is_dir() {
            let mut new_root = root.as_ref().as_os_str().to_owned();
            new_root.push("/");
            new_root.push(path.as_ref().file_name().unwrap());
            traverse_dir_and_store(&file.path(), &PathBuf::from(new_root), archive_writer)?;
        } else if file.path().is_file() {
            store_file(file.path(), &root, archive_writer)?;
        };
    }
    Ok(())
}
