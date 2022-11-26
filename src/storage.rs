use crate::api::CurrentState;
use std::fs::File;
use std::io::{BufReader, BufWriter};

pub(crate) fn load_current(file_path: &String) -> Result<CurrentState, anyhow::Error> {
    let f = File::open(file_path)?;
    let reader = BufReader::new(f);
    let u = serde_json::from_reader::<BufReader<File>, CurrentState>(reader)?;
    Ok(u)
}
pub(crate) fn save_current(
    file_path: &String,
    current: &CurrentState,
) -> Result<(), anyhow::Error> {
    let f = File::options().write(true).open(file_path)?;
    let writer = BufWriter::new(f);
    serde_json::to_writer_pretty(writer, current)?;
    Ok(())
}

pub(crate) fn init_current(
    file_path: &String,
    current: &CurrentState,
) -> Result<(), anyhow::Error> {
    let file = File::create(file_path)?;
    serde_json::to_writer_pretty(file, current)?;
    Ok(())
}
