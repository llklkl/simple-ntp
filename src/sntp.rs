use std::net::{UdpSocket};
use std::time;
use std::time::Duration;

#[derive(Debug)]
pub enum NtpError {
    ServiceUnavailable(String),
    BadNtpServerAddr(String),
    UnexpectedErr(String),
    TruncatedNtpMessage,
    UntrustedMessage,
}

// const NTP_VERSION_3: u8 = 3;
const NTP_VERSION_4: u8 = 4;

const NTP_MODE_CLIENT: u8 = 3;
// const NTP_MODE_SERVER: u8 = 4;

const NTP_DEFAULT_PORT: &str = "123";

/// Retrieve current unix timestamp.
///
/// Example
/// ```rust
/// # use simple_ntp::sntp::clock_offset_nanos;
///
/// fn main() {
///     match unix_timestamp("ntp.aliyun.com:123") {
///         Ok(msg) => {
///             println!("{:?}", msg);
///         }
///         Err(err) => println!("{:?}", err)
///     }
/// }
/// ```
pub fn unix_timestamp(ntp_server: &str) -> Result<Duration, NtpError> {
    let (t1, t2, t3, t4) = ntp(ntp_server)?;

    Ok((t1 * 2 + t2 + t3 - t1 - t4) / 2)
}

/// Get system clock offset in nano seconds. local timestamp sub remote timestamp.
///
/// Example
/// ```rust
/// # use simple_ntp::sntp::clock_offset_nanos;
///
/// fn main() {
///     match clock_offset_nanos("ntp.aliyun.com") {
///         Ok(msg) => { println!("{:?}", msg as f64 / 1e9); }
///         Err(err) => println!("{:?}", err)
///     }
/// }
///
/// ```
pub fn clock_offset_nanos(ntp_server: &str) -> Result<i64, NtpError> {
    let (t1, t2, t3, t4) = ntp(ntp_server)?;

    let mut diff = (t2.as_secs() as i64 - t1.as_secs() as i64 + t3.as_secs() as i64 - t4.as_secs() as i64) * 1_000_000_000 / 2;
    diff += (t2.subsec_nanos() as i64 - t1.subsec_nanos() as i64 + t3.subsec_nanos() as i64 - t4.subsec_nanos() as i64) / 2;
    Ok(diff)
}

/// Convert time.Duration to ntp timestamp format
pub fn duration_to_ntp_timestamp(d: &Duration) -> u64 {
    let seconds = d.as_secs();
    let nanos = d.subsec_nanos();

    seconds << 32 | (u32::MAX / 1000000000 * nanos) as u64
}

/// Convert ntp timestamp to time.Duration
pub fn ntp_timestamp_to_duration(t: u64) -> Duration {
    let seconds = (t >> 32) - 2208988800; // 2208988800 为 1900.1.1 到 1970.1.1 的秒数
    let nanos = (t & u32::MAX as u64) * 1000000000 / u32::MAX as u64;

    Duration::new(seconds, nanos as u32)
}

/// Retrieve four time from ntp server: t1, t2, t3 and t4.
///
/// t1: client transmit time
///
/// t2: server received time
///
/// t3: server transmit time
///
/// t4: client received time
///
/// So, system clock offset = ((t2 - t1) + (t3 - t4)) / 2,
/// and round-trip time = ((t4 - t1) - (t3 - t2)) / 2.
pub fn ntp(ntp_server: &str) -> Result<(Duration, Duration, Duration, Duration), NtpError> {
    let socket = make_socket(ntp_server)?;

    let validate_time = sys_time();
    let timestamp = duration_to_ntp_timestamp(&validate_time);
    let client_msg = NtpMsg::new_for_client(NTP_VERSION_4, timestamp);

    let mut buf = client_msg.marshal();
    let transmit_time = sys_time();
    send_full(&socket, buf.as_slice())?;
    let n = recv_full(&socket, buf.as_mut_slice())?;
    let receive_time = sys_time();
    buf.truncate(n);

    let mut server_msg = NtpMsg::new();
    server_msg.unmarshal(buf.as_slice())?;

    if server_msg.originate_timestamp != timestamp {
        return Err(NtpError::UntrustedMessage);
    }

    Ok((transmit_time,
        ntp_timestamp_to_duration(server_msg.receiver_timestamp),
        ntp_timestamp_to_duration(server_msg.transmit_timestamp),
        receive_time
    ))
}

fn sys_time() -> Duration {
    time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap()
}

fn getaddr(svr: &str) -> String {
    if svr.contains(':') {
        svr.to_string()
    } else {
        svr.to_string() + ":" + NTP_DEFAULT_PORT
    }
}

fn make_socket(target_addr: &str) -> Result<UdpSocket, NtpError> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|err| {
        NtpError::ServiceUnavailable(err.to_string())
    })?;
    socket.connect(getaddr(target_addr)).map_err(|err| {
        NtpError::UnexpectedErr(err.to_string())
    })?;
    socket.set_write_timeout(Some(Duration::from_secs(5))).map_err(|err| {
        NtpError::UnexpectedErr(err.to_string())
    })?;
    socket.set_read_timeout(Some(Duration::from_secs(5))).map_err(|err| {
        NtpError::UnexpectedErr(err.to_string())
    })?;

    Ok(socket)
}

fn send_full(socket: &UdpSocket, buf: &[u8]) -> Result<(), NtpError> {
    socket.send(buf).map_err(|err| {
        NtpError::ServiceUnavailable(err.to_string())
    })?;

    Ok(())
}

fn recv_full(socket: &UdpSocket, buf: &mut [u8]) -> Result<usize, NtpError> {
    let n = socket.recv(buf).map_err(|err| {
        NtpError::ServiceUnavailable(err.to_string())
    })?;

    Ok(n)
}

#[derive(Debug)]
pub struct NtpMsg {
    leap_indicator: u8,
    version_number: u8,
    mode: u8,
    stratum: u8,
    poll: u8,
    precision: u8,
    root_delay: u32,
    root_dispersion: u32,
    reference_identifier: u32,
    reference_timestamp: u64,
    originate_timestamp: u64,
    receiver_timestamp: u64,
    transmit_timestamp: u64,
}

impl NtpMsg {
    fn new() -> Self {
        NtpMsg {
            leap_indicator: 0,
            version_number: 0,
            mode: 0,
            stratum: 0,
            poll: 0,
            precision: 0,
            root_delay: 0,
            root_dispersion: 0,
            reference_identifier: 0,
            reference_timestamp: 0,
            originate_timestamp: 0,
            receiver_timestamp: 0,
            transmit_timestamp: 0,
        }
    }

    fn new_for_client(version: u8, transmit_timestamp: u64) -> Self {
        NtpMsg {
            leap_indicator: 0,
            version_number: version,
            mode: NTP_MODE_CLIENT,
            stratum: 0,
            poll: 0,
            precision: 0,
            root_delay: 0,
            root_dispersion: 0,
            reference_identifier: 0,
            reference_timestamp: 0,
            originate_timestamp: 0,
            receiver_timestamp: 0,
            transmit_timestamp,
        }
    }

    fn marshal(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(48);
        data.push(self.leap_indicator << 6 | self.version_number << 3 | self.mode);
        data.push(self.stratum);
        data.push(self.poll);
        data.push(self.precision);
        data.extend_from_slice(self.root_delay.to_be_bytes().as_slice());
        data.extend_from_slice(self.root_dispersion.to_be_bytes().as_slice());
        data.extend_from_slice(self.reference_identifier.to_be_bytes().as_slice());
        data.extend_from_slice(self.reference_timestamp.to_be_bytes().as_slice());
        data.extend_from_slice(self.originate_timestamp.to_be_bytes().as_slice());
        data.extend_from_slice(self.receiver_timestamp.to_be_bytes().as_slice());
        data.extend_from_slice(self.transmit_timestamp.to_be_bytes().as_slice());

        data
    }

    fn unmarshal(&mut self, data: &[u8]) -> Result<(), NtpError> {
        if data.len() != 48 {
            return Err(NtpError::TruncatedNtpMessage);
        }

        self.leap_indicator = data[0] >> 6;
        self.version_number = (data[0] >> 3) & 0b111;
        self.mode = data[0] & 0b111;
        self.stratum = data[1];
        self.poll = data[2];
        self.precision = data[3];
        self.root_delay = u32::from_be_bytes(data[4..8].try_into().unwrap());
        self.root_dispersion = u32::from_be_bytes(data[8..12].try_into().unwrap());
        self.reference_identifier = u32::from_be_bytes(data[12..16].try_into().unwrap());
        self.reference_timestamp = u64::from_be_bytes(data[16..24].try_into().unwrap());
        self.originate_timestamp = u64::from_be_bytes(data[24..32].try_into().unwrap());
        self.receiver_timestamp = u64::from_be_bytes(data[32..40].try_into().unwrap());
        self.transmit_timestamp = u64::from_be_bytes(data[40..48].try_into().unwrap());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sntp::*;

    #[test]
    fn test_ntp() {
        match ntp("ntp.aliyun.com") {
            Ok(msg) => {
                println!("{:?}", msg);
            }
            Err(err) => println!("{:?}", err)
        }
    }

    #[test]
    fn test_delta() {
        match clock_offset_nanos("ntp.aliyun.com") {
            Ok(msg) => {
                println!("{:?}", msg as f64 / 1e9);
            }
            Err(err) => println!("{:?}", err)
        }
    }

    #[test]
    fn test_timestamp() {
        match unix_timestamp("ntp.aliyun.com") {
            Ok(msg) => {
                println!("{:?}", msg);
            }
            Err(err) => println!("{:?}", err)
        }

        match unix_timestamp("ntp.aliyun.com:123") {
            Ok(msg) => {
                println!("{:?}", msg);
            }
            Err(err) => println!("{:?}", err)
        }
    }
}