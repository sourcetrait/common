//! Touch a file
//!
//! Performs a coreutils touch on a file, creating it if necessary, and setting
//! its modified time if provided.
use std::fs::{self, FileTimes};
use std::io::{self, Write};
use std::path::Path;
use chrono::{DateTime, Utc};

/// Touch a file, creating it if necessary, and optionally setting its contents,
/// and modified / accessed times.
pub fn touch_file(path: &Path, datetime: Option<chrono::DateTime<Utc>>, contents: Option<&str>) -> io::Result<()> {
    touch_file_impl(path, datetime, contents)
}

/// Touch a file, creating it if necessary, and optionally setting its contents,
/// and modified / accessed times.
pub fn touch_file_bytes(path: &Path, contents: &[u8], datetime: Option<chrono::DateTime<Utc>>) -> io::Result<()> {
    touch_file_impl(path, datetime, Some(contents))
}

fn touch_file_impl<C: AsRef<[u8]>>(path: &Path, datetime: Option<DateTime<Utc>>, contents: Option<C>) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(contents.is_some())
        .open(path)?;
    
    if let Some(contents) = contents {
        file.write_all(contents.as_ref())?;
    }
    
    let _ = file.flush();
    
    if let Some(datetime) = datetime {
        let filetimes = FileTimes::new().set_modified(datetime.into());
        file.set_times(filetimes)?;
    }
    
    Ok(())
}
