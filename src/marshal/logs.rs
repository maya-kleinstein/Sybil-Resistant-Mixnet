use crate::marshal::*;
use std::fs::{self};
use std::path::PathBuf;

use super::ips::get_cur_ip_files;

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
pub fn rename_ip_logs() {
    // Get all IP file paths
    let ips = get_cur_ip_files(&*LOGS_FOLDER).unwrap();

    // enumerate through file_ips
    for (ip_index, ip) in ips.iter().enumerate() {
        let ip_str = format!("{}{}{}", *BASE_FOLDER, *LOGS_FOLDER, ip.to_string());
        let mix_str = format!("{}{}{}", *BASE_FOLDER, *LOGS_FOLDER, ip_index);
        fs::rename(ip_str, mix_str).unwrap();
    }
}

/// Merge all log files into one
pub fn merge_log_files() {
    // Get all current log file paths
}

/// Delete all files in data\logs that aren't relevant
pub fn delete_old_log_files() {
    let paths = fs::read_dir(format!("{}{}", *BASE_FOLDER, *LOGS_FOLDER)).unwrap();
    for path in paths {
        let path = path.unwrap().path();
        // check if path is old file
        if !is_final_log(&path) {
            fs::remove_file(path).unwrap();
        }
    }
}

fn is_final_log(path: &PathBuf) -> bool {
    let filename = path.file_name().unwrap().to_str().unwrap();
    filename.contains("log_")
}

#[cfg(test)]
mod test {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    #[test]
    fn test_rename_ip_logs() {
        // Write random IP's as files to logs folder
        let mut rand_ips: Vec<IpAddr> = Vec::new();
        for _ in 0..((*NUM_MIXES - 1) as usize) {
            let r = rand::random();
            let ip = Ipv4Addr::new(r, r, r, r);
            rand_ips.push(IpAddr::V4(ip));
        }
        for ip in rand_ips.iter() {
            let filename = format!("{}{}", *LOGS_FOLDER, ip);
            serialize_data_to_file::<IpAddr>(&ip, &filename).unwrap();
        }
        println!("{:?}", get_cur_ip_files(&*LOGS_FOLDER).unwrap());

        rename_ip_logs();
    }
}
