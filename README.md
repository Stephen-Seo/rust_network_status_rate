# Linux Traffic Rate in Rust

This program was used in conjunction with i3status to add network-rate-info to
swaybar. The `const` variables at the top of `main.rs` can be configured for
different use cases.

It simply reads from `/proc/net/dev` to obtain byte-counts of the specified
network interface and writes to four files, two of which keep track of the total
byte count for sending and receiving, and the other two contain the "diffs" for
each (configurable) interval (by default 5 seconds).
