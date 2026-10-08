//! Viper-only Windows HID discovery and cancellable overlapped feature I/O.
use crate::protocol::Result;
use std::ptr::{null, null_mut};
use windows_sys::{
    Win32::{
        Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
        Foundation::*,
        Storage::FileSystem::*,
        System::{IO::*, Threading::*},
    },
    core::GUID,
};
const GET_FEATURE: u32 = (0x0b << 16) | (100 << 2) | 2;
const SET_FEATURE: u32 = (0x0b << 16) | (100 << 2) | 1;
const TIMEOUT_MS: u32 = 3000;
pub struct FeatureDevice {
    handle: HANDLE,
}
// Each handle is owned by one worker and never concurrently accessed.
unsafe impl Send for FeatureDevice {}
impl Drop for FeatureDevice {
    fn drop(&mut self) {
        unsafe {
            CancelIoEx(self.handle, null());
            CloseHandle(self.handle);
        }
    }
}
struct Request {
    overlap: OVERLAPPED,
    bytes: [u8; 91],
}
impl Drop for Request {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.overlap.hEvent);
        }
    }
}
fn viper_path(path: &str, pid: &str) -> bool {
    path.to_ascii_lowercase()
        .contains(&format!("vid_1532&pid_{pid}&mi_00"))
}
impl FeatureDevice {
    pub fn open_viper() -> Result<(Self, bool)> {
        let mut guid: GUID = unsafe { std::mem::zeroed() };
        unsafe {
            HidD_GetHidGuid(&mut guid);
        }
        let paths = interface_paths(&guid)?;
        for (pid, wireless) in [("007a", false), ("007b", true)] {
            let candidates: Vec<_> = paths
                .iter()
                .filter(|p| viper_path(&String::from_utf16_lossy(p), pid))
                .collect();
            let mut matches = Vec::new();
            for path in candidates {
                let handle = unsafe {
                    CreateFileW(
                        path.as_ptr(),
                        0,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        null(),
                        OPEN_EXISTING,
                        FILE_FLAG_OVERLAPPED,
                        null_mut(),
                    )
                };
                if handle == INVALID_HANDLE_VALUE {
                    continue;
                }
                let device = Self { handle };
                let mut data: PHIDP_PREPARSED_DATA = 0;
                if !unsafe { HidD_GetPreparsedData(handle, &mut data) } {
                    continue;
                }
                let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
                let status = unsafe { HidP_GetCaps(data, &mut caps) };
                unsafe {
                    HidD_FreePreparsedData(data);
                }
                if status >= 0
                    && caps.UsagePage == 1
                    && caps.Usage == 2
                    && caps.FeatureReportByteLength == 91
                {
                    matches.push(device);
                }
            }
            if matches.len() > 1 {
                return Err(
                    "Multiple Viper Ultimate devices found. Connect one mouse at a time.".into(),
                );
            }
            if let Some(device) = matches.pop() {
                return Ok((device, wireless));
            }
        }
        Err("Viper Ultimate not connected".into())
    }
    pub fn send_feature_report(&self, bytes: &[u8; 91]) -> Result<()> {
        self.transfer(SET_FEATURE, *bytes).map(|_| ())
    }
    pub fn get_feature_report(&self, bytes: &mut [u8; 91]) -> Result<usize> {
        let (data, count) = self.transfer(GET_FEATURE, *bytes)?;
        *bytes = data;
        // Windows omits the zero report-ID byte from its returned feature length.
        Ok(if bytes[0] == 0 {
            (count + 1).min(91)
        } else {
            count
        })
    }
    fn transfer(&self, control: u32, bytes: [u8; 91]) -> Result<([u8; 91], usize)> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut request = Box::new(Request {
            overlap: OVERLAPPED {
                hEvent: event,
                ..unsafe { std::mem::zeroed() }
            },
            bytes,
        });
        let mut count = 0;
        let buffer = request.bytes.as_mut_ptr();
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                control,
                buffer.cast(),
                91,
                buffer.cast(),
                91,
                &mut count,
                &mut request.overlap,
            )
        };
        if result == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_IO_PENDING {
                return Err(format!("HID request failed: Windows error {error}"));
            }
            if let Err(error) =
                complete_request(self.handle, &request.overlap, &mut count, TIMEOUT_MS)
            {
                if error.pending {
                    std::mem::forget(request);
                }
                return Err(error.message);
            }
        }
        Ok((request.bytes, count as usize))
    }
}
struct IoFailure {
    message: String,
    pending: bool,
}
fn complete_request(
    handle: HANDLE,
    overlap: &OVERLAPPED,
    count: &mut u32,
    timeout: u32,
) -> std::result::Result<(), IoFailure> {
    if unsafe { GetOverlappedResultEx(handle, overlap, count, timeout, 0) } != 0 {
        return Ok(());
    }
    let error = unsafe { GetLastError() };
    unsafe {
        CancelIoEx(handle, overlap);
    }
    // A driver must release the buffers before they can be freed. If cancellation
    // fails to finish, the caller retains the request allocation for memory safety.
    let finished = unsafe { GetOverlappedResultEx(handle, overlap, count, 1000, 0) };
    let pending = finished == 0
        && matches!(
            unsafe { GetLastError() },
            WAIT_TIMEOUT | ERROR_IO_INCOMPLETE
        );
    Err(IoFailure {
        message: format!(
            "HID request timed out or failed (Windows error {error}). Wake or reconnect the mouse."
        ),
        pending,
    })
}
fn interface_paths(guid: &GUID) -> Result<Vec<Vec<u16>>> {
    for _ in 0..4 {
        let mut size = 0;
        let status = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut size,
                guid,
                null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if status != CR_SUCCESS {
            return Err(format!("HID interface size query failed: {status}"));
        }
        let mut data = vec![0u16; size as usize];
        let status = unsafe {
            CM_Get_Device_Interface_ListW(
                guid,
                null(),
                data.as_mut_ptr(),
                size,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if status == CR_BUFFER_SMALL {
            continue;
        }
        if status != CR_SUCCESS {
            return Err(format!("HID interface query failed: {status}"));
        }
        return Ok(data
            .split(|v| *v == 0)
            .filter(|p| !p.is_empty())
            .map(|p| {
                let mut path = p.to_vec();
                path.push(0);
                path
            })
            .collect());
    }
    Err("HID device list kept changing; try again".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_windows_io_times_out_and_cancels_without_leaking() {
        let name: Vec<u16> = format!(
            r"\\.\pipe\viper-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        let pipe = unsafe {
            windows_sys::Win32::System::Pipes::CreateNamedPipeW(
                name.as_ptr(),
                FILE_FLAG_OVERLAPPED | PIPE_ACCESS_DUPLEX,
                0,
                1,
                4096,
                4096,
                0,
                null(),
            )
        };
        assert_ne!(pipe, INVALID_HANDLE_VALUE);
        let device = FeatureDevice { handle: pipe };
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        assert!(!event.is_null());
        let mut request = Box::new(Request {
            overlap: OVERLAPPED {
                hEvent: event,
                ..unsafe { std::mem::zeroed() }
            },
            bytes: [0; 91],
        });
        assert_eq!(
            unsafe {
                windows_sys::Win32::System::Pipes::ConnectNamedPipe(pipe, &mut request.overlap)
            },
            0
        );
        assert_eq!(unsafe { GetLastError() }, ERROR_IO_PENDING);
        let mut count = 0;
        let started = std::time::Instant::now();
        let error = complete_request(pipe, &request.overlap, &mut count, 20)
            .err()
            .unwrap();
        assert!(!error.pending, "Cancelled request still owns its buffers");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(error.message.contains("timed out"));
        assert_eq!(
            unsafe { GetOverlappedResult(device.handle, &request.overlap, &mut count, 0) },
            0
        );
        assert_eq!(unsafe { GetLastError() }, ERROR_OPERATION_ABORTED);
    }
    #[test]
    fn discovery_filters_unrelated_devices_before_opening_them() {
        assert!(viper_path(
            r"\\?\hid#vid_1532&pid_007b&mi_00#receiver",
            "007b"
        ));
        assert!(!viper_path(
            r"\\?\hid#vid_1532&pid_007b&mi_01#receiver",
            "007b"
        ));
        assert!(!viper_path(
            r"\\?\hid#vid_1b1c&pid_007b&mi_00#other",
            "007b"
        ));
        assert!(!viper_path(
            r"\\?\hid#vid_1532&pid_007a&mi_00#cable",
            "007b"
        ));
    }
}
