use crate::marshal::*;
use std::fs;
use std::io;
use std::net::{IpAddr, TcpStream};
use std::thread::sleep;
use std::time::Duration;

/// Get's all mixes IP's sorted, and my mix's index
pub fn init_mix_ips() -> io::Result<(Vec<IpAddr>, u16)> {
    let my_id = std::env::var("SLURM_PROCID").unwrap();
    let my_ip = write_my_ip_to_file(&my_id)?;
    let ips = get_all_ips_from_files()?;

    let index = my_id.parse::<u16>().unwrap();

    assert!(ips[index as usize] == my_ip);
    println!("All IPs: {:?}", ips);
    println!("My ID: {}", index);

    Ok((ips, index))
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
    Ok(ips)
}

pub fn write_my_ip_to_file(my_id: &String) -> io::Result<IpAddr> {
    let my_ip = get_my_ip()?;
    let filename = format!("{}{}", *IPS_FOLDER, my_id);
    serialize_data_to_file::<IpAddr>(&my_ip, &filename).unwrap();
    Ok(my_ip)
}

pub fn get_my_ip() -> io::Result<IpAddr> {
    // Connect to a public server to discover our external IP address
    let socket = TcpStream::connect("google.com:80")?;
    Ok(socket.local_addr()?.ip())
}

pub fn get_cur_ip_files(dir: &str) -> std::io::Result<Vec<IpAddr>> {
    let mut ips: Vec<(IpAddr, u16)> = Vec::new();
    for entry in fs::read_dir(format!("{}{}", *BASE_FOLDER, dir))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().unwrap().to_str().unwrap();
            let id = filename.parse::<u16>().unwrap();
            let ips_filename = format!("{}{}", *IPS_FOLDER, filename);
            let ip = deserialize_data_from_file::<IpAddr>(&ips_filename).unwrap();
            ips.push((ip, id));
        }
    }
    // sort ips based on the id value
    ips.sort_by_key(|k| k.1);
    let ordered_ips: Vec<IpAddr> = ips.iter().map(|(ip, _)| *ip).collect();
    Ok(ordered_ips)
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

        for _ in 0..(*NUM_MIXES as usize) {
            let r = rand::random();
            let ip = Ipv4Addr::new(r, r, r, r);
            rand_ips.push(IpAddr::V4(ip));
        }

        for i in 0..rand_ips.len() {
            let filename = format!("{}{}", *IPS_FOLDER, i);
            serialize_data_to_file::<IpAddr>(&rand_ips[i], &filename).unwrap();
        }

        // Get all IP's from the IP folder
        let ips = get_all_ips_from_files().unwrap();
        assert_eq!(ips.len(), *NUM_MIXES as usize);

        println!("All IPs: {:?}", ips);

        // Empty IP folder
        delete_ip_files();
    }
}
