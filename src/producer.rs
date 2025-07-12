#[cfg(unix)]
mod unix {
    use std::ffi::OsStr;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    pub struct Producer {
        stream: UnixStream,
    }

    impl Producer {
        pub fn new(name: &OsStr) -> std::io::Result<Self> {
            let stream = UnixStream::connect(name)?;
            Ok(Producer { stream })
        }

        pub fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.stream.write(data)
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::io::Write;
    use std::os::windows::io::FromRawHandle;

    use windows_core::HSTRING;
    pub struct Producer {
        pipe: std::fs::File,
    }

    fn open_pipe(name: &OsStr) -> std::io::Result<std::fs::File> {
        use windows::Win32::Foundation::GENERIC_WRITE;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, OPEN_EXISTING,
        };
        use windows::Win32::System::Pipes::WaitNamedPipeW;

        let hname = HSTRING::from(name);

        let handle = unsafe {
            CreateFileW(
                &hname,
                GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES::default(),
                None,
            )
        }?;

        if handle.is_invalid() {
            return Err(std::io::Error::last_os_error());
        }

        let w: bool = unsafe { WaitNamedPipeW(&hname, 20000) }.into();
        if !w {
            return Err(std::io::ErrorKind::TimedOut.into());
        }

        Ok(unsafe { std::fs::File::from_raw_handle(handle.0) })
    }

    impl Producer {
        pub fn new(name: &OsStr) -> std::io::Result<Self> {
            let pipe = open_pipe(name)?;
            Ok(Producer { pipe })
        }

        pub fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let len = self.pipe.write(data)?;
            self.pipe.flush()?;
            Ok(len)
        }
    }
}

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;
