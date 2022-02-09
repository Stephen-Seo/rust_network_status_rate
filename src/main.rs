use std::env::var;
use std::fs::File;
use std::io::{Read, Write};
use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};
use structopt::StructOpt;

const ALTERNATE_PREFIX_DIR: &str = "/tmp";

const DEFAULT_SEND_TOTAL_FILENAME: &str = "rust_send_total";
const DEFAULT_RECV_TOTAL_FILENAME: &str = "rust_recv_total";
const DEFAULT_SEND_INTERVAL_FILENAME: &str = "rust_send_interval";
const DEFAULT_RECV_INTERVAL_FILENAME: &str = "rust_recv_interval";

const DEFAULT_PID_FILENAME: &str = "rust_network_rate_pid";

#[derive(StructOpt, Debug, Clone)]
struct Opt {
    net_dev: String,
    #[structopt(
        short = "c",
        long = "disable-scaling",
        help = "Disables byte scaling into interval files"
    )]
    disable_byte_scaling: bool,
    #[structopt(
        short = "e",
        long = "enable-alt-prefix",
        help = "Enable use of alternate prefix instead of XDG_RUNTIME_DIR"
    )]
    enable_alternate_prefix: bool,
    #[structopt(
        short = "p",
        long = "prefix",
        help = "Prefix to use instead of XDG_RUNTIME_DIR if enabled",
        default_value = ALTERNATE_PREFIX_DIR
    )]
    alternate_prefix_dir: String,
    #[structopt(
        short = "u",
        long = "send-total",
        help = "Filename of total bytes sent (in prefix dir)",
        default_value = DEFAULT_SEND_TOTAL_FILENAME
    )]
    send_total_filename: String,
    #[structopt(
        short = "d",
        long = "recv-total",
        help = "Filename of total bytes received (in prefix dir)",
        default_value = DEFAULT_RECV_TOTAL_FILENAME
    )]
    recv_total_filename: String,
    #[structopt(
        short = "s",
        long = "send-interval",
        help = "Filename of interval bytes sent (in prefix dir)",
        default_value = DEFAULT_SEND_INTERVAL_FILENAME
    )]
    send_interval_filename: String,
    #[structopt(
        short = "r",
        long = "recv-interval",
        help = "Filename of interval bytes recieved (in prefix dir)",
        default_value = DEFAULT_RECV_INTERVAL_FILENAME
    )]
    recv_interval_filename: String,

    #[structopt(
        short = "i",
        long = "pid-filename",
        help = "Filename to write pid to",
        default_value = DEFAULT_PID_FILENAME
    )]
    pid_filename: String,

    #[structopt(
        short = "v",
        long = "interval-seconds",
        help = "Interval in seconds between checking network rate",
        default_value = "5"
    )]
    interval_seconds: u64,
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
    let mut instant = Instant::now();
    let sleep_duration = Duration::from_secs(sleep_seconds);
    let half_sleep_duration = sleep_duration / 2;
    let double_sleep_duration = sleep_duration * 2;
    loop {
        func()?;
        let newer_instant = Instant::now();
        let elapsed = newer_instant - instant;
        // println!("{:?}", elapsed);
        instant = newer_instant;
        if elapsed < half_sleep_duration {
            sleep(sleep_duration);
        } else {
            sleep(double_sleep_duration - elapsed);
        }
    }
}

fn get_pid() -> Result<String, String> {
    let path = std::fs::read_link("/proc/self")
        .map_err(|e| format!("Failed to get path \"/proc/self\", {}", e))?;
    let name = path
        .file_name()
        .ok_or_else(|| String::from("Failed to get file_name of \"/proc/self\""))?
        .to_str()
        .ok_or_else(|| String::from("Failed to get str of file_name of \"/proc/self\""))?;
    Ok(name.to_string())
}

fn main() -> Result<(), String> {
    let opt = Opt::from_args();

    println!("Using net_dev == \"{}\"", opt.net_dev);

    let prefix_dir: String;
    if opt.enable_alternate_prefix {
        prefix_dir = opt.alternate_prefix_dir.clone();
    } else {
        prefix_dir = var("XDG_RUNTIME_DIR").map_err(|e| format!("{}", e))?;
        assert!(!prefix_dir.is_empty(), "XDG_RUNTIME_DIR is not set");
    }

    {
        let pid = get_pid()?;
        let mut pid_pathbuf = PathBuf::new();
        pid_pathbuf.push(&prefix_dir);
        pid_pathbuf.push(opt.pid_filename);
        let mut pid_file =
            File::create(&pid_pathbuf).map_err(|_| format!("Failed to open {:?}", &pid_pathbuf))?;
        pid_file
            .write_all(pid.as_bytes())
            .map_err(|_| format!("Failed to write into {:?}", &pid_pathbuf))?;
    }

    let mut send_total_path = PathBuf::new();
    send_total_path.push(&prefix_dir);
    send_total_path.push(opt.send_total_filename);
    let send_total_path = send_total_path;

    let mut recv_total_path = PathBuf::new();
    recv_total_path.push(&prefix_dir);
    recv_total_path.push(opt.recv_total_filename);
    let recv_total_path = recv_total_path;

    let mut send_interval_path = PathBuf::new();
    send_interval_path.push(&prefix_dir);
    send_interval_path.push(opt.send_interval_filename);
    let send_interval_path = send_interval_path;

    let mut recv_interval_path = PathBuf::new();
    recv_interval_path.push(&prefix_dir);
    recv_interval_path.push(opt.recv_interval_filename);
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
        opt.interval_seconds,
    )?;

    Ok(())
}
