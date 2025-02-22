use crate::config::*;
use tracing::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::MAIN_SEPARATOR;

use self::ips::{delete_ip_files, create_all_shutdown_files};
use self::logs::{delete_old_log_files, merge_log_files};

pub mod info;
pub mod ips;
pub mod logs;

/*
This file contains all functions related to marshalling data to and from files.
This includes: INFO, IPS, and LOGS files.
INFO: all pre-computed data
IPS: all IP addresses for initial setup
LOGS: all logs from runs
*/

lazy_static! {
    /// The base folder for all files
    pub static ref BASE_FOLDER: String = format!("data{}", MAIN_SEPARATOR);
    /// The folder for all IP files
    pub static ref IPS_FOLDER: String = format!("ips{}", MAIN_SEPARATOR);
    /// The folder for all info files
    pub static ref INFO_FOLDER: String = format!("info{}", MAIN_SEPARATOR);
    /// The folder for all logs
    pub static ref LOGS_FOLDER: String = format!("logs{}", MAIN_SEPARATOR);
    /// The path to the shutdown file
    pub static ref SHUTDOWN_FILE: String = format!("{}{}", *BASE_FOLDER, "shutdown");
}

pub fn serialize_data_to_file<T: Serialize>(
    data: &T,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = format!("{}{}", *BASE_FOLDER, filename);
    let json = serde_json::to_string::<T>(data)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn deserialize_data_from_file<T: for<'a> Deserialize<'a>>(
    filename: &str,
) -> Result<T, serde_json::Error> {
    let path = format!("{}{}", *BASE_FOLDER, filename);
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let result: Result<T, serde_json::Error> = serde_json::from_str::<T>(&(contents.as_str()));
    return result;
}

/// Runs all function needed to manage \data dir. at the end of a run.
pub fn manage_files() {
    merge_log_files().unwrap();
    delete_ip_files();
    delete_old_log_files();
    create_all_shutdown_files();
}
