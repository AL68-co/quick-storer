use std::{env, error::Error, fs::File, io::{BufWriter, Write}};

fn main() -> Result<(), Box<dyn Error>> {
    let mut config_file = BufWriter::new(File::create(format!("{}/config.rs", env::var("OUT_DIR")?))?);
    write!(config_file, "pub const FILE_EXTENSION: &str = \"{}\";\n\
pub const DIR_EXTENSION: &str = \"{}\";", String::from("csfile"), String::from("csdir"))?;
    
    Ok(())
}
