use crate::marshal::*;
use chrono::NaiveTime;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead};
use std::path::PathBuf;

use super::ips::get_cur_ip_files;

/// Initializes a logger that outputs everything to both stdout and the file at file_path
pub fn init_logger(file_path: &str) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}]{}",
                chrono::Local::now().format("%H:%M:%S%.3f"),
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

/// Rename IP logs to Mix ID logs
pub fn rename_ip_logs() {
    // Get all IP file paths
    let ips = get_cur_ip_files(&*LOGS_FOLDER).unwrap();

    // enumerate through file_ips
    for (ip_index, ip) in ips.iter().enumerate() {
        let ip_str = format!("{}{}{}", *BASE_FOLDER, *LOGS_FOLDER, ip.to_string());
        let mix_str = format!("{}{}mix {}", *BASE_FOLDER, *LOGS_FOLDER, ip_index);
        fs::rename(ip_str, mix_str).unwrap();
    }
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

fn extract_timestamp(line: &str) -> NaiveTime {
    NaiveTime::parse_from_str(&line[1..13], "%H:%M:%S%.3f").unwrap()
}

/// Merge all log files into one
pub fn merge_log_files() -> io::Result<()> {
    let dir = format!("{}{}", *BASE_FOLDER, *LOGS_FOLDER);
    let entries: Vec<_> = fs::read_dir(&dir)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<_, _>>()?;

    let merged_data: Vec<_> = entries
        .iter()
        .filter(|path| !is_final_log(path))
        .flat_map(|path| {
            let file = File::open(path).unwrap();
            let filename = path.file_name().unwrap().to_string_lossy().into_owned();
            io::BufReader::new(file).lines().filter_map(move |line| {
                line.ok()
                    .map(|l| (extract_timestamp(&l), format!("<{}>{}", filename, l)))
            })
        })
        .collect();

    let mut sorted_data = merged_data;
    sorted_data.sort_by_key(|k| k.0);

    // Collect all lines into a single string
    let content: String = sorted_data
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<&str>>()
        .join("\n");

    // Create a new log file name with the current date
    let timestamp = chrono::Local::now().format("%d.%m_%H.%M").to_string();
    let path_str = format!("{}{}{}", &dir, "log_", &timestamp);

    // Write the content to the new file
    let mut file = OpenOptions::new()
        .append(true)
        .write(true)
        .create(true)
        .open(path_str)
        .expect("cannot open file");

    // Add config data to file
    let json_config = serde_json::to_string_pretty::<ConfigInfo>(&*CONFIG_INFO)?;
    file.write_all(format!("{}{}", json_config, "\n").as_bytes())?;

    // Write logs to file
    file.write_all(content.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod test {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    #[test]
    fn test_logs() {
        // Write random IP's as files to logs folder
        let mut rand_ips: Vec<IpAddr> = Vec::new();
        for _ in 0..((*NUM_MIXES - 1) as usize) {
            let r = rand::random();
            let ip = Ipv4Addr::new(r, r, r, r);
            rand_ips.push(IpAddr::V4(ip));
        }
        for ip in rand_ips.iter() {
            let filename = format!("{}{}", *LOGS_FOLDER, ip);
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let data_line = format!("[{}]{}", timestamp, &ip);
            serialize_data_to_file::<String>(&data_line, &filename).unwrap();
        }
        println!("{:?}", get_cur_ip_files(&*LOGS_FOLDER).unwrap());

        rename_ip_logs();

        // Merge all log files
        merge_log_files().unwrap();
    }
}
