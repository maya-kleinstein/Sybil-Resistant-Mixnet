use crate::marshal::*;
use chrono::NaiveTime;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead};
use std::path::PathBuf;
use tracing_subscriber::{filter, fmt, Registry};
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_appender::non_blocking;
use tracing::level_filters::LevelFilter;
use super::ips::get_cur_ip_files;

pub const RESULTS: &str = "Results:";

struct CustomTime;

impl FormatTime for CustomTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer) -> std::fmt::Result {
        write!(w, "[{}]", chrono::Local::now().format("%H:%M:%S%.3f"))?;
        Ok(())
    }
}

/// Initializes a logger that outputs everything to both stdout and the file at file_path
pub fn init_logger(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let base_folder = PathBuf::from(BASE_FOLDER.clone()); // Replace with your actual base folder
    let logs_folder = PathBuf::from(LOGS_FOLDER.clone()); // Replace with your actual logs folder
    let log_file = base_folder.join(logs_folder).join(file_path);

    let file_appender = RollingFileAppender::new(
        Rotation::NEVER,
        log_file.parent().unwrap(),
        log_file.file_name().unwrap().to_str().unwrap(),
    );

    let (file_writer, _guard) = non_blocking(file_appender);

    // Stdout non-blocking writer
    let (stdout_writer, _stdout_guard) = non_blocking(std::io::stdout());

    // File layer
    let file_layer = fmt::layer()
        .with_timer(CustomTime)
        .with_writer(file_writer)
        .with_ansi(false)
        .with_level(true)
        .with_target(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    // Stdout layer
    let stdout_layer = fmt::layer()
        .with_timer(CustomTime)
        .with_writer(stdout_writer)
        .with_ansi(true)
        .with_level(true)
        .with_target(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    let filter = filter::Targets::new()
        .with_default(LevelFilter::OFF) // Set the default log level to INFO
        .with_target("bbs", LevelFilter::TRACE); // Set the log level for the "bbs" target to ERROR

    // Combine layers
    let subscriber = Registry::default()
        .with(file_layer)
        .with(stdout_layer)
        .with(filter);

    tracing::subscriber::set_global_default(subscriber)?;

    // Keeping _guard alive to ensure logs are flushed on exit
    std::mem::forget(_guard);
    std::mem::forget(_stdout_guard);
    
    Ok(())
}

/// Rename IP logs to Mix ID logs
pub fn rename_ip_logs() {
    // Get all IP file paths
    let ips = get_cur_ip_files(&*LOGS_FOLDER).unwrap();

    // enumerate through file_ips
    for (ip_index, ip) in ips.iter().enumerate() {
        let ip_str = format!("{}{}{}", *BASE_FOLDER, *LOGS_FOLDER, ip.to_string());
        let mix_str = format!("{}{}{}", *BASE_FOLDER, *LOGS_FOLDER, ip_index);
        debug!("Renaming {} to {}", &ip_str, &mix_str);
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

fn get_sorted_string(merged_data: Vec<(NaiveTime, String)>) -> String {
    let mut sorted_data = merged_data;
    sorted_data.sort_by_key(|k| k.0);

    // Collect all lines into a single string
    let content: String = sorted_data
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<&str>>()
        .join("\n");

    return content;
}

fn get_results_string(content: &str) -> String {
    // Get + Parse all lines that contain RESULTS
    let results: Vec<_> = content
        .lines()
        .filter(|line| line.contains(RESULTS))
        .map(|line| line.split(RESULTS).collect::<Vec<&str>>()[1])
        .collect();

    // Collect all lines into a single string
    let results_string: String = results.join("\n");

    return results_string;
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
                match line {
                    Ok(l) => Some((extract_timestamp(&l), format!("<{}>{}", filename, l))),
                    Err(e) => {
                        error!("Error reading line from {}: {}", filename, e);
                        None
                    }
                }
            })
        })
        .collect();
    
    debug!("Len merged data: {}", merged_data.len());

    let content: String = get_sorted_string(merged_data);

    let results = get_results_string(&content);

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

    // Add results to file
    file.write_all(format!("{}{}", results, "\n").as_bytes())?;

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
