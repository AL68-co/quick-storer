#![warn(missing_docs)]

//! Defines the way our files are stored

use brotli::enc::BrotliEncoderParams;
use lazy_static::lazy_static;
use mla::{config::ArchiveWriterConfig, ArchiveReader, ArchiveWriter, Layers};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
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
    #[structopt(parse(from_os_str))]
    input: PathBuf,
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
    let input = CLI_OPTIONS.input.clone();
    if CLI_OPTIONS.verbose {
        println!("{}", input.display());
    }
    if input.is_dir() {
        store_dir(input.as_path())?;
    } else if !input.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The argument is not a file nor a directory",
        )
        .into());
    } else {
        match input.extension() {
            Some(ext) => {
                if ext == OsString::from(config::FILE_EXTENSION).as_os_str() {
                    load_file(&input)?;
                } else if ext == OsString::from(config::DIR_EXTENSION).as_os_str() {
                    load_dir(&input)?;
                } else {
                    store_file(&input)?;
                }
            }
            None => store_file(&input)?,
        };
    }

    Ok(())
}

fn load_file(input: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut input_file = BufReader::new(File::open(&input)?);
    let output = input.with_extension("");
    if CLI_OPTIONS.verbose {
        println!("{}", output.display());
    }
    let mut output_file = BufWriter::new(
        OpenOptions::new()
            .create_new(!CLI_OPTIONS.force)
            .create(true)
            .truncate(true)
            .write(true)
            .open(&output)?,
    );
    brotli::BrotliDecompress(&mut input_file, &mut output_file)?;
    output_file.flush()?;
    Ok(())
}

fn store_file(input: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut input_file = BufReader::new(File::open(&input)?);
    let output = PathBuf::from({
        let mut output = input.as_os_str().to_owned();
        output.push(format!(".{}", config::FILE_EXTENSION));
        output
    });
    if CLI_OPTIONS.verbose {
        println!("{}", output.display());
    }
    let mut output_file = BufWriter::new(
        OpenOptions::new()
            .create_new(!CLI_OPTIONS.force)
            .create(true)
            .truncate(true)
            .write(true)
            .open(&output)?,
    );
    brotli::BrotliCompress(
        &mut input_file,
        &mut output_file,
        &BrotliEncoderParams::default(),
    )?;
    output_file.flush()?;
    Ok(())
}

fn load_dir(input: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = BufReader::new(File::open(input)?);
    let mut archive_reader = ArchiveReader::new(input_file)?;
    let iter = archive_reader.list_files()?.cloned().collect::<Vec<_>>();
    let nb_of_files = iter.len();
    for (i, file_string) in iter.into_iter().enumerate() {
        let file_path = {
            let mut bd = input.to_path_buf();
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
        std::fs::create_dir_all(file_path.parent().unwrap())?;
        let mut output_file = BufWriter::new(
            OpenOptions::new()
                .create_new(!CLI_OPTIONS.force)
                .create(true)
                .truncate(true)
                .write(true)
                .open(&file_path)?,
        );
        let mut file = archive_reader.get_file(file_string.clone())?.unwrap().data;
        io::copy(&mut file, &mut output_file)?;
    }
    Ok(())
}

fn store_dir(input: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from({
        let mut output = input.as_os_str().to_owned();
        output.push(format!(".{}", config::DIR_EXTENSION));
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
    traverse_and_store_dir(input, &PathBuf::from(""), &mut archive_writer)?;
    archive_writer.finalize()?;
    archive_writer.flush()?;
    Ok(())
}

fn traverse_and_store_dir<W: Write>(
    current_dir: &Path,
    root: &Path,
    archive_writer: &mut ArchiveWriter<W>,
) -> Result<(), Box<dyn std::error::Error>> {
    let iter: Vec<_> = (current_dir.read_dir()?).flatten().enumerate().collect();
    let len = iter.len();
    for (i, file) in iter {
        if file.path().is_dir() {
            let mut new_root = root.as_os_str().to_owned();
            new_root.push("/");
            new_root.push(current_dir.file_name().unwrap());
            traverse_and_store_dir(&file.path(), &PathBuf::from(new_root), archive_writer)?;
        } else if file.path().is_file() {
            let input_file_handler = BufReader::new(File::open(file.path())?);
            let file_path = {
                let mut p = root.as_os_str().to_owned();
                p.push("/");
                p.push(current_dir.file_name().unwrap());
                p.push("/");
                p.push(file.path().file_name().unwrap());
                p
            };
            println!(
                "Found file to store at {}, storing it at path {} (file {} of {} in this directory)",
                file.path().display(),
                file_path.to_string_lossy(),
                i+1,
                len
            );
            archive_writer.add_file(
                &file_path.to_string_lossy(),
                file.metadata()?.len(),
                input_file_handler,
            )?;
        };
    }
    Ok(())
}
