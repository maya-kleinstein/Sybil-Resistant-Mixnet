use crate::data_manager::*;
use chrono::NaiveDateTime;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

/*
LOGS management
*/
/// Initializes a logger that outputs everything to both stdout and the file at file_path
pub fn init_logger(file_path: &str) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}]{}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                message
            ))
        })
        .chain(fern::log_file(format!(
            "{}{}{}",
            *BASE_FOLDER, *LOGS_FOLDER, file_path
        ))?)
        .chain(std::io::stdout())
        .level(LevelFilter::Off)
        .level_for("bbs", LevelFilter::Trace)
        .apply()?;
    Ok(())
}

/*
NOTES:
Below are functions that we'll eventually need in order to generate final logs and delete unnecessary files
at the end of a run.
Config will run them through a "manage_files()" function that will run these in the following order:
- rename_ip_logs: rename all IP logs to Mix ID logs
- merge_log_files: merge all log files into one (in order of timestamps)
- delete_ip_files: delete all files in date\ips
- delete_old_log_files: delete all log files that AREN'T merged ones (they'll have a format for there name)

    WRITE TESTS FOR THIS!!!!!
*/

/// Rename IP logs to Mix ID logs
pub fn rename_ip_logs() {}

/// Merge all log files into one
pub fn merge_log_files(filenames: Vec<&str>) {
    // Read each file, parse the timestamp, and push the items into the heap.
    for (file_index, file_path) in filenames.iter().enumerate() {
        let file = File::open(file_path).unwrap();
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.unwrap();
            let timestamp_str = &line[1..24];
            let format = "%Y-%m-%d %H:%M:%S%.3f";
            let timestamp = NaiveDateTime::parse_from_str(timestamp_str, format).unwrap();
        }
    }
}

/// Delete all files in date\ips
pub fn delete_ip_files() {
    let paths = fs::read_dir(format!("{}{}", *BASE_FOLDER, *IPS_FOLDER)).unwrap();
    for path in paths {
        let path = path.unwrap().path();
        fs::remove_file(path).unwrap();
    }
}
