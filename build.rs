use std::{
    env,
    error::Error,
    fs::File,
    io::{BufWriter, Write},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut config_file =
        BufWriter::new(File::create(format!("{}/config.rs", env::var("OUT_DIR")?))?);
    write!(
        config_file,
        "pub const FILE_EXTENSION: &str = \"{}\";\n\
pub const DIR_EXTENSION: &str = \"{}\";",
        String::from("csfile"),
        String::from("csdir")
    )?;
    #[cfg(windows)]
    windows_part()?;

    Ok(())
}

#[cfg(windows)]
fn windows_part() -> Result<(), Box<dyn Error>> {
    use std::{path::PathBuf, str::FromStr};

    let mut resources = winres::WindowsResource::new();

    println!("cargo:rerun-if-changed=resources/icon.png");

    if PathBuf::from_str("resources/icon.png")?.exists() {
        let image = image::open("resources/icon.png")?;
        image
            .thumbnail(256, 256)
            .save(format!("{}/icon.ico", env::var("OUT_DIR")?))?;

        resources.set_icon(&format!("{}/icon.ico", env::var("OUT_DIR")?));
    };
    resources.compile()?;
    Ok(())
}
