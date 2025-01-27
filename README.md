# Linux Traffic Rate in Rust

[NOTICE: `swaybar_info` may be a better choice.](https://github.com/Stephen-Seo/swaybar_info)

This program was used in conjunction with i3status to add network-rate-info to
swaybar. The `const` variables at the top of `main.rs` can be configured for
different use cases.

It simply reads from `/proc/net/dev` to obtain byte-counts of the specified
network interface and writes to four files, two of which keep track of the total
byte count for sending and receiving, and the other two contain the "diffs" for
each (configurable) interval (by default 5 seconds).

    rust_network_status_rate 0.1.0
    
    USAGE:
        rust_network_status_rate [FLAGS] [OPTIONS] <net-dev>
    
    FLAGS:
        -s, --disable-scaling      Disables byte scaling into interval files
        -e, --enable-alt-prefix    Enable use of alternate prefix instead of XDG_RUNTIME_DIR
        -h, --help                 Prints help information
        -V, --version              Prints version information
    
    OPTIONS:
        -p, --prefix <alternate-prefix-dir>             Prefix to use instead of XDG_RUNTIME_DIR if enabled [default: /tmp]
        -v, --interval-seconds <interval-seconds>       Interval in seconds between checking network rate [default: 5]
        -i, --pid-filename <pid-filename>               Filename to write pid to [default: rust_network_rate_pid]
        -r, --recv-interval <recv-interval-filename>
                Filename of interval bytes recieved (in prefix dir) [default: rust_recv_interval]
    
        -d, --recv-total <recv-total-filename>
                Filename of total bytes received (in prefix dir) [default: rust_recv_total]
    
        -s, --send-interval <send-interval-filename>
                Filename of interval bytes sent (in prefix dir) [default: rust_send_interval]
    
        -u, --send-total <send-total-filename>
                Filename of total bytes sent (in prefix dir) [default: rust_send_total]
    
    
    ARGS:
        <net-dev>
