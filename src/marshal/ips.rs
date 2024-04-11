use crate::marshal::*;
use std::fs;
use std::io;
use std::net::{IpAddr, TcpStream};
use std::thread::sleep;
use std::time::Duration;

/// Get's all mixes IP's sorted, and my mix's index
pub fn init_mix_ips() -> io::Result<(Vec<IpAddr>, u16)> {
    let my_ip = write_my_ip_to_file()?;
    let ips = get_all_ips_from_files()?;

    let index = ips.iter().position(|&r| r == my_ip).unwrap();

    debug!("All IPs: {:?}", ips);
    debug!("My ID: {}", index);

    Ok((ips, index as u16))
}

// Get all mixes IP's from files, sorted
pub fn get_all_ips_from_files() -> std::io::Result<Vec<IpAddr>> {
    let mut ips = get_cur_ip_files(&*IPS_FOLDER)?;
    while ips.len() as u16 != *NUM_MIXES {
        let missing_ips = format!("Could only find: {:?}", ips);
        warn!("{:?}", missing_ips);
        sleep(Duration::from_secs(1));
        ips = get_cur_ip_files(&*IPS_FOLDER)?;
    }
    ips.sort();
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

pub fn get_cur_ip_files(dir: &str) -> std::io::Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = Vec::new();
    for entry in fs::read_dir(format!("{}{}", *BASE_FOLDER, dir))? {
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

pub fn delete_ip_files() {
    let paths = fs::read_dir(format!("{}{}", *BASE_FOLDER, *IPS_FOLDER)).unwrap();
    for path in paths {
        let path = path.unwrap().path();
        fs::remove_file(path).unwrap();
    }
}

pub fn create_all_shutdown_files(){
    for i in 0..*NUM_MIXES {
        let _file = File::create(format!("{}{}", *SHUTDOWN_FILE, i)).unwrap();
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn ips_test() {
        // Write random IP's as files to IP folder
        let mut rand_ips: Vec<IpAddr> = Vec::new();
        for _ in 0..((*NUM_MIXES - 1) as usize) {
            let r = rand::random();
            let ip = Ipv4Addr::new(r, r, r, r);
            rand_ips.push(IpAddr::V4(ip));
        }
        for ip in rand_ips.iter() {
            let filename = format!("{}{}", *IPS_FOLDER, ip);
            serialize_data_to_file::<IpAddr>(&ip, &filename).unwrap();
        }

        // Write my IP to the IP folder
        let my_ip = get_my_ip().unwrap();
        let filename = format!("{}{}", *IPS_FOLDER, my_ip);
        serialize_data_to_file::<IpAddr>(&my_ip, &filename).unwrap();

        // Get all IP's from the IP folder
        let mut ips = get_all_ips_from_files().unwrap();
        assert_eq!(ips.len(), *NUM_MIXES as usize);

        ips.sort();
        let index = ips.iter().position(|&r| r == my_ip).unwrap();
        assert_eq!(ips[index], my_ip);

        println!("All IPs: {:?}", ips);
        println!("My IP's index: {}", index);

        // Empty IP folder
        delete_ip_files();
    }
}
