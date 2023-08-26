use crate::marshal::*;
use std::fs;
use std::io;
use std::net::{IpAddr, TcpStream};
use std::thread::sleep;
use std::time::Duration;

/*
IP management
*/

/// Get's all mixes IP's sorted, and my mix's index
pub fn init_mix_ips() -> io::Result<(Vec<IpAddr>, u16)> {
    let my_ip = write_my_ip_to_file()?;
    let mut ips = get_all_ips_from_files()?;

    ips.sort();
    let index = ips.iter().position(|&r| r == my_ip).unwrap();

    debug!("All IPs: {:?}", ips);
    debug!("My ID: {}", index);

    Ok((ips, index as u16))
}

pub fn get_all_ips_from_files() -> std::io::Result<Vec<IpAddr>> {
    let mut ips = get_cur_ip_files()?;
    while ips.len() as u16 != *NUM_MIXES {
        let missing_ips = format!("Could only find: {:?}", ips);
        warn!("{:?}", missing_ips);
        sleep(Duration::from_millis(10));
        ips = get_cur_ip_files()?;
    }
    Ok(ips)
}

pub fn write_my_ip_to_file() -> io::Result<IpAddr> {
    let my_ip = get_my_ip()?;
    let filename = format!("{}{}", *IPS_FOLDER, my_ip);
    serialize_data_to_file::<IpAddr>(&my_ip, &filename).unwrap();
    Ok(my_ip)
}

pub fn get_my_ip() -> io::Result<IpAddr> {
    // Connect to a public server to discover our external IP address
    let socket = TcpStream::connect("google.com:80")?;
    Ok(socket.local_addr()?.ip())
}

fn get_cur_ip_files() -> std::io::Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = Vec::new();
    for entry in fs::read_dir(format!("{}{}", *BASE_FOLDER, *IPS_FOLDER))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name() {
                if let Some(filename_str) = filename.to_str() {
                    let ip = filename_str.parse::<IpAddr>();
                    if ip.is_ok() {
                        ips.push(ip.unwrap());
                    }
                }
            }
        }
    }
    Ok(ips)
}
