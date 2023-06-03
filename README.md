# simple-ntp

Library to retrieve Unix timestamps using SNTP requests.

only for practice.

# example

use cargo add simple-ntp.
```shell
cargo add simple-ntp
```

example code:
```rust
use simple_ntp::sntp;

fn main() {
    let timestamp = sntp::unix_timestamp("ntp.aliyun.com").unwrap();
    println!("{:?}", timestamp);

    // use specified port
    let timestamp = sntp::unix_timestamp("ntp.aliyun.com:123").unwrap();
    println!("{:?}", timestamp);

    let delta = sntp::clock_offset_nanos("ntp.aliyun.com").unwrap();
    println!("{:?}", delta as f64 / 1e9);
}
```

# license

MIT license