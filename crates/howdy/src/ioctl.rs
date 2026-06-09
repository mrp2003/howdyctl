//! Minimal v4l2 `VIDIOC_QUERYCAP` ioctl (Linux), computed without any C bindings —
//! enough to tell a real capture node from a metadata-only one.
use std::io;
use std::os::unix::io::RawFd;

const IOC_READ: u32 = 2;

/// Build a Linux `_IOC()` request number.
const fn ioc(dir: u32, ty: u32, nr: u32, size: u32) -> u32 {
    (dir << 30) | (ty << 8) | nr | (size << 16)
}

/// `struct v4l2_capability` from `<linux/videodev2.h>` (104 bytes, `#[repr(C)]`).
#[repr(C)]
pub struct V4l2Capability {
    pub driver: [u8; 16],
    pub card: [u8; 32],
    pub bus_info: [u8; 32],
    pub version: u32,
    pub capabilities: u32,
    pub device_caps: u32,
    pub reserved: [u32; 3],
}

/// `V4L2_CAP_VIDEO_CAPTURE` — the device can capture video.
pub const CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
/// `V4L2_CAP_DEVICE_CAPS` — `device_caps` is filled in and describes *this* node.
pub const CAP_DEVICE_CAPS: u32 = 0x8000_0000;

/// `VIDIOC_QUERYCAP` — query a v4l2 device's capabilities.
pub fn querycap(fd: RawFd) -> io::Result<V4l2Capability> {
    let req = ioc(
        IOC_READ,
        b'V' as u32,
        0,
        std::mem::size_of::<V4l2Capability>() as u32,
    ) as libc::c_ulong;
    // SAFETY: `cap` is a correctly-sized, zeroed buffer matching the kernel struct;
    // the ioctl only writes into it.
    let mut cap: V4l2Capability = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::ioctl(fd, req, &mut cap as *mut V4l2Capability) };
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_struct_is_104_bytes() {
        // Must match the kernel ABI exactly or QUERYCAP returns garbage.
        assert_eq!(std::mem::size_of::<V4l2Capability>(), 104);
    }

    #[test]
    fn querycap_request_number() {
        // _IOR('V', 0, 104) == 0x80685600
        let req = ioc(IOC_READ, b'V' as u32, 0, 104);
        assert_eq!(req, 0x8068_5600);
    }
}
