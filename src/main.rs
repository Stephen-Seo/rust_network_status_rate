use std::env::var;
use std::fs::File;
use std::io::{Read, Write};
use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};
use structopt::StructOpt;

const INTERVAL_SECONDS: u64 = 5;

const USE_XDG_RUNTIME_DIR: bool = true;
const ALTERNATE_PREFIX_DIR: &str = "/tmp";

const SEND_TOTAL_FILENAME: &str = "rust_send_total";
const RECV_TOTAL_FILENAME: &str = "rust_recv_total";
const SEND_INTERVAL_FILENAME: &str = "rust_send_interval";
const RECV_INTERVAL_FILENAME: &str = "rust_recv_interval";

#[derive(StructOpt, Debug, Clone)]
struct Opt {
    net_dev: String,
    #[structopt(
        short = "s",
        long = "disable-scaling",
        help = "Disables byte scaling into interval files"
    )]
    disable_byte_scaling: bool,
}

#[derive(Copy, Clone, Debug)]
struct ByteState {
    pub recv: u64,
    pub send: u64,
}

impl Sub for ByteState {
    type Output = ByteState;

    fn sub(self, rhs: Self) -> Self::Output {
        let mut result_byte_state = ByteState { recv: 0, send: 0 };

        if self.recv >= rhs.recv {
            result_byte_state.recv = self.recv - rhs.recv;
        }
        if self.send >= rhs.send {
            result_byte_state.send = self.send - rhs.send;
        }

        result_byte_state
    }
}

fn read_proc_net_dev(net_device: &str) -> Result<ByteState, String> {
    let mut byte_state = ByteState { recv: 0, send: 0 };

    let mut file_string = String::new();
    {
        let mut file = File::open("/proc/net/dev")
            .map_err(|_| String::from("Failed to open \"/proc/net/dev\""))?;
        file.read_to_string(&mut file_string)
            .map_err(|_| String::from("Failed to read from \"/proc/net/dev\""))?;
    }

    for line in file_string.lines() {
        if line.trim().starts_with(net_device) {
            for (count, word) in line.split_ascii_whitespace().enumerate() {
                if count == 1 {
                    byte_state.recv = word.parse::<u64>().map_err(|_| {
                        String::from("Failed to parse recv bytes from \"/proc/net/dev\"")
                    })?;
                } else if count == 9 {
                    byte_state.send = word.parse::<u64>().map_err(|_| {
                        String::from("Failed to parse send bytes from \"/proc/net/dev\"")
                    })?;
                    return Ok(byte_state);
                }
            }
            return Err(String::from(
                "Failed to parse from \"/proc/net/dev\", too few words?",
            ));
        }
    }
    Err(String::from(
        "Failed to parse from \"/proc/net/dev\", missing device?",
    ))
}

fn write_compare_state(
    net_device: &str,
    send_filename: &Path,
    recv_filename: &Path,
) -> Result<ByteState, String> {
    let mut prev_byte_state = ByteState { send: 0, recv: 0 };

    {
        let mut temp_string: String = String::new();
        let send_file_open_result = File::open(send_filename);
        if let Ok(mut send_file) = send_file_open_result {
            let read_result = send_file.read_to_string(&mut temp_string);
            if read_result.is_ok() {
                let int_parse_result = temp_string.trim().parse::<u64>();
                if let Ok(i) = int_parse_result {
                    prev_byte_state.send = i;
                }
            }
        }
    }
    {
        let mut temp_string: String = String::new();
        let recv_file_open_result = File::open(recv_filename);
        if let Ok(mut recv_file) = recv_file_open_result {
            let read_result = recv_file.read_to_string(&mut temp_string);
            if read_result.is_ok() {
                let int_parse_result = temp_string.trim().parse::<u64>();
                if let Ok(i) = int_parse_result {
                    prev_byte_state.recv = i;
                }
            }
        }
    }

    let byte_state = read_proc_net_dev(net_device)?;

    {
        let mut send_file = File::create(send_filename)
            .map_err(|_| format!("Failed to create \"{:?}\"", send_filename))?;
        send_file
            .write_all(byte_state.send.to_string().as_bytes())
            .map_err(|_| format!("Failed to write into \"{:?}\"", send_filename))?;
    }

    {
        let mut recv_file = File::create(recv_filename)
            .map_err(|_| format!("Failed to create \"{:?}\"", recv_filename))?;
        recv_file
            .write_all(byte_state.recv.to_string().as_bytes())
            .map_err(|_| format!("Failed to write into \"{:?}\"", recv_filename))?;
    }

    Ok(byte_state - prev_byte_state)
}

fn do_set_states(
    net_device: &str,
    disable_byte_scalaing: bool,
    send_interval_filename: &Path,
    recv_interval_filename: &Path,
    send_total_filename: &Path,
    recv_total_filename: &Path,
) -> Result<(), String> {
    let state = write_compare_state(net_device, send_total_filename, recv_total_filename)?;

    if disable_byte_scalaing {
        {
            let mut send_interval_file = File::create(send_interval_filename)
                .map_err(|_| format!("Failed to create \"{:?}\"", send_interval_filename))?;
            send_interval_file
                .write_all(state.send.to_string().as_bytes())
                .map_err(|_| format!("Failed to write into \"{:?}\"", send_interval_filename))?;
        }
        {
            let mut recv_interval_file = File::create(recv_interval_filename)
                .map_err(|_| format!("Failed to create \"{:?}\"", recv_interval_filename))?;
            recv_interval_file
                .write_all(state.recv.to_string().as_bytes())
                .map_err(|_| format!("Failed to write into \"{:?}\"", recv_interval_filename))?;
        }
    } else {
        {
            let mut send_string = String::new();
            if state.send > 1024 * 1024 {
                send_string.push_str(&(state.send as f64 / 1024.0 / 1024.0).to_string());
                let decimal_location_opt = send_string.find('.');
                if let Some(location) = decimal_location_opt {
                    send_string.truncate(location + 2);
                }
                send_string.push_str("MB");
            } else if state.send > 1024 {
                send_string.push_str(&(state.send as f64 / 1024.0).to_string());
                let decimal_location_opt = send_string.find('.');
                if let Some(location) = decimal_location_opt {
                    send_string.truncate(location + 2);
                }
                send_string.push_str("KB");
            } else {
                send_string.push_str(&state.send.to_string());
                send_string.push('B');
            }
            let mut send_interval_file = File::create(send_interval_filename)
                .map_err(|_| format!("Failed to create \"{:?}\"", send_interval_filename))?;
            send_interval_file
                .write_all(send_string.as_bytes())
                .map_err(|_| format!("Failed to write into \"{:?}\"", send_interval_filename))?;
        }
        {
            let mut recv_string = String::new();
            if state.recv > 1024 * 1024 {
                recv_string.push_str(&(state.recv as f64 / 1024.0 / 1024.0).to_string());
                let decimal_location_opt = recv_string.find('.');
                if let Some(location) = decimal_location_opt {
                    recv_string.truncate(location + 2);
                }
                recv_string.push_str("MB");
            } else if state.recv > 1024 {
                recv_string.push_str(&(state.recv as f64 / 1024.0).to_string());
                let decimal_location_opt = recv_string.find('.');
                if let Some(location) = decimal_location_opt {
                    recv_string.truncate(location + 2);
                }
                recv_string.push_str("KB");
            } else {
                recv_string.push_str(&state.recv.to_string());
                recv_string.push('B');
            }
            let mut recv_interval_file = File::create(recv_interval_filename)
                .map_err(|_| format!("Failed to create \"{:?}\"", recv_interval_filename))?;
            recv_interval_file
                .write_all(recv_string.as_bytes())
                .map_err(|_| format!("Failed to write into \"{:?}\"", recv_interval_filename))?;
        }
    }

    Ok(())
}

fn timer_execute<F>(func: F, sleep_seconds: u64) -> Result<(), String>
where
    F: std::ops::Fn() -> Result<(), String>,
{
    let mut instant = Instant::now() - Duration::from_secs(sleep_seconds);
    let diff_duration = Duration::from_secs(sleep_seconds * 2);
    let mut duration: Duration;
    loop {
        func()?;
        let newer_instant = Instant::now();
        duration = diff_duration - (newer_instant - instant);
        instant = newer_instant;
        sleep(duration);
    }
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();

    println!("Using net_dev == \"{}\"", opt.net_dev);

    let prefix_dir: String;
    if USE_XDG_RUNTIME_DIR {
        prefix_dir = var("XDG_RUNTIME_DIR").map_err(|e| format!("{}", e))?;
    } else {
        prefix_dir = ALTERNATE_PREFIX_DIR.to_string();
    }

    let mut send_total_path = PathBuf::new();
    send_total_path.push(&prefix_dir);
    send_total_path.push(SEND_TOTAL_FILENAME);
    let send_total_path = send_total_path;

    let mut recv_total_path = PathBuf::new();
    recv_total_path.push(&prefix_dir);
    recv_total_path.push(RECV_TOTAL_FILENAME);
    let recv_total_path = recv_total_path;

    let mut send_interval_path = PathBuf::new();
    send_interval_path.push(&prefix_dir);
    send_interval_path.push(SEND_INTERVAL_FILENAME);
    let send_interval_path = send_interval_path;

    let mut recv_interval_path = PathBuf::new();
    recv_interval_path.push(&prefix_dir);
    recv_interval_path.push(RECV_INTERVAL_FILENAME);
    let recv_interval_path = recv_interval_path;

    timer_execute(
        move || {
            do_set_states(
                &opt.net_dev,
                opt.disable_byte_scaling,
                &send_interval_path,
                &recv_interval_path,
                &send_total_path,
                &recv_total_path,
            )
        },
        INTERVAL_SECONDS,
    )?;

    Ok(())
}
