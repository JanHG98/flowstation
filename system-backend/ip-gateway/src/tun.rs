#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::CString;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, RawFd};

    const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
    const TUNSETPERSIST: libc::c_ulong = 0x4004_54cb;
    const TUNSETOWNER: libc::c_ulong = 0x4004_54cc;
    const IFF_TUN: libc::c_short = 0x0001;
    const IFF_NO_PI: libc::c_short = 0x1000;

    #[repr(C)]
    union IfReqData {
        flags: libc::c_short,
        padding: [u8; 24],
    }

    #[repr(C)]
    struct IfReq {
        name: [libc::c_char; libc::IFNAMSIZ],
        data: IfReqData,
    }

    pub struct TunDevice {
        file: File,
        name: String,
    }

    impl TunDevice {
        pub fn open(name: &str, owner_user: Option<&str>, persistent: bool) -> Result<Self, String> {
            if name.is_empty() || name.len() >= libc::IFNAMSIZ {
                return Err("invalid TUN interface name".to_string());
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/net/tun")
                .map_err(|error| format!("open /dev/net/tun: {error}"))?;
            let mut request = IfReq {
                name: [0; libc::IFNAMSIZ],
                data: IfReqData {
                    flags: IFF_TUN | IFF_NO_PI,
                },
            };
            for (index, byte) in name.bytes().enumerate() {
                request.name[index] = byte as libc::c_char;
            }
            let result = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETIFF, &mut request) };
            if result < 0 {
                return Err(format!(
                    "TUNSETIFF {name}: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if let Some(owner_user) = owner_user.filter(|value| !value.trim().is_empty()) {
                let owner = CString::new(owner_user)
                    .map_err(|_| "TUN owner_user contains a NUL byte".to_string())?;
                let password = unsafe { libc::getpwnam(owner.as_ptr()) };
                if password.is_null() {
                    return Err(format!("TUN owner user {owner_user} does not exist"));
                }
                let uid = unsafe { (*password).pw_uid };
                let result = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETOWNER, uid) };
                if result < 0 {
                    return Err(format!(
                        "TUNSETOWNER {owner_user}: {}",
                        std::io::Error::last_os_error()
                    ));
                }
            }
            if persistent {
                let result = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETPERSIST, 1) };
                if result < 0 {
                    return Err(format!(
                        "TUNSETPERSIST {name}: {}",
                        std::io::Error::last_os_error()
                    ));
                }
            }
            set_nonblocking(file.as_raw_fd())?;
            Ok(Self {
                file,
                name: name.to_string(),
            })
        }

        pub fn name(&self) -> &str {
            &self.name
        }

        pub fn read_packet(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, String> {
            match self.file.read(buffer) {
                Ok(0) => Ok(None),
                Ok(size) => Ok(Some(size)),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => Ok(None),
                Err(error) => Err(format!("read TUN {}: {error}", self.name)),
            }
        }

        pub fn write_packet(&mut self, packet: &[u8]) -> Result<(), String> {
            self.file
                .write_all(packet)
                .map_err(|error| format!("write TUN {}: {error}", self.name))
        }
    }

    fn set_nonblocking(fd: RawFd) -> Result<(), String> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(format!(
                "fcntl(F_GETFL): {}",
                std::io::Error::last_os_error()
            ));
        }
        let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        if result < 0 {
            return Err(format!(
                "fcntl(F_SETFL): {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    pub use TunDevice as PlatformTunDevice;
}

#[cfg(not(target_os = "linux"))]
mod other {
    pub struct PlatformTunDevice;

    impl PlatformTunDevice {
        pub fn open(
            _name: &str,
            _owner_user: Option<&str>,
            _persistent: bool,
        ) -> Result<Self, String> {
            Err("TUN is supported only on Linux".to_string())
        }
        pub fn name(&self) -> &str {
            "unsupported"
        }
        pub fn read_packet(&mut self, _buffer: &mut [u8]) -> Result<Option<usize>, String> {
            Ok(None)
        }
        pub fn write_packet(&mut self, _packet: &[u8]) -> Result<(), String> {
            Err("TUN is supported only on Linux".to_string())
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::PlatformTunDevice as TunDevice;
#[cfg(not(target_os = "linux"))]
pub use other::PlatformTunDevice as TunDevice;
