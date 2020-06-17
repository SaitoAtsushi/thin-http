#![cfg(windows)]
extern crate winapi;

mod wide_string {

    use std::convert::From;
    use std::ffi::OsStr;
    use std::ops::Deref;
    use std::os::windows::prelude::*;

    pub struct WideString(Vec<u16>);

    impl From<&str> for WideString {
        fn from(s: &str) -> Self {
            WideString(
                OsStr::new(s)
                    .encode_wide()
                    .chain(Some(0).into_iter())
                    .collect::<Vec<_>>(),
            )
        }
    }

    impl Deref for WideString {
        type Target = Vec<u16>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}

pub mod wininet {
    use super::wide_string::WideString;
    use std::convert::From;
    use std::iter::Iterator;
    use std::ptr::null;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::wininet::InternetReadFile;
    use winapi::um::wininet::*;
    use winapi::um::wininet::HTTP_QUERY_STATUS_CODE;

    #[derive(Debug)]
    pub struct Internet(HINTERNET);

    impl Drop for Internet {
        fn drop(&mut self) {
            unsafe {
                winapi::um::wininet::InternetCloseHandle(self.0);
            }
        }
    }

    #[allow(dead_code)]
    enum OpenType {
        Proxy = winapi::um::wininet::INTERNET_OPEN_TYPE_PROXY as isize,
        Direct = winapi::um::wininet::INTERNET_OPEN_TYPE_DIRECT as isize,
        Preconfig = winapi::um::wininet::INTERNET_OPEN_TYPE_PRECONFIG as isize,
        PreconfigWithNoAutoproxy =
            winapi::um::wininet::INTERNET_OPEN_TYPE_PRECONFIG_WITH_NO_AUTOPROXY as isize,
    }

    #[allow(dead_code)]
    enum InternetFlag {
        None = 0,
        Offline = winapi::um::wininet::INTERNET_FLAG_OFFLINE as isize,
        Async = winapi::um::wininet::INTERNET_FLAG_ASYNC as isize,
        OfflineAndAsync = winapi::um::wininet::INTERNET_FLAG_OFFLINE as isize
            | winapi::um::wininet::INTERNET_FLAG_ASYNC as isize,
    }

    impl From<OpenType> for DWORD {
        fn from(w: OpenType) -> Self {
            w as DWORD
        }
    }

    impl From<InternetFlag> for DWORD {
        fn from(w: InternetFlag) -> Self {
            w as DWORD
        }
    }

    pub struct Response(HINTERNET);

    impl Drop for Response {
        fn drop(&mut self) {
            unsafe {
                winapi::um::wininet::InternetCloseHandle(self.0);
            }
        }
    }

    impl Internet {
        pub fn open(agent: &str, proxy: Option<&str>) -> Option<Internet> {
            let agent = WideString::from(agent);

            let internet_handle = unsafe {
                InternetOpenW(
                    agent.as_ptr(),
                    match proxy {
                        Some(_) => OpenType::Proxy.into(),
                        None => OpenType::Direct.into(),
                    },
                    match proxy {
                        Some(&ref proxyname) => WideString::from(proxyname).as_ptr(),
                        None => null(),
                    },
                    null(),
                    InternetFlag::None.into(),
                )
            };

            if internet_handle.is_null() {
                None
            } else {
                Some(Internet(internet_handle))
            }
        }

        pub fn get(&self, url: &str, headers: Option<&str>) -> Option<Response> {
            let handle = unsafe {
                InternetOpenUrlW(
                    self.0,
                    WideString::from(url).as_ptr(),
                    match headers {
                        Some(&ref headers) => WideString::from(headers).as_ptr(),
                        None => null(),
                    },
                    0xFFFFFFFF,
                    INTERNET_FLAG_RELOAD
                        | INTERNET_FLAG_DONT_CACHE
                        | INTERNET_FLAG_RAW_DATA
                        | INTERNET_FLAG_SECURE,
                    0,
                )
            };
            if handle.is_null() {
                None
            } else {
                Some(Response(handle))
            }
        }
    }

    const BUFFER_SIZE: DWORD = 1000;

    pub struct Bytes<'a> {
        handle: &'a Response,
        data: [u8; BUFFER_SIZE as usize],
        index: DWORD,
        size: DWORD,
    }

    impl<'a> Bytes<'a> {
        fn new(handle: &'a Response) -> Bytes<'a> {
            Bytes {
                handle: handle,
                data: [0; BUFFER_SIZE as usize],
                index: 0,
                size: 0,
            }
        }
    }

    impl<'a> Iterator for Bytes<'a> {
        type Item = u8;
        fn next(&mut self) -> Option<Self::Item> {
            let mut result = 0;
            let mut read_size: DWORD = 0;
            if self.index >= self.size {
                unsafe {
                    result = InternetReadFile(
                        self.handle.0,
                        (&mut self.data[..]).as_mut_ptr() as *mut winapi::ctypes::c_void,
                        BUFFER_SIZE,
                        &mut read_size as *mut DWORD,
                    );
                    self.size = read_size;
                    self.index = 0;
                };
            };
            let return_value = if (result != 0) && (read_size == 0) {
                None
            } else {
                Some(self.data[self.index as usize])
            };
            self.index += 1;
            return_value
        }
    }

    impl Response {
        pub fn as_bytes(&self) -> Bytes {
            Bytes::new(&self)
        }

        pub fn status(&self) -> DWORD {
            let mut status_code: DWORD = 0;
            let mut len: DWORD = 4;
            let mut index: DWORD = 0;
            unsafe {
                HttpQueryInfoW(
                    self.0,
                    HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
                    (&mut status_code as *mut DWORD) as *mut winapi::ctypes::c_void,
                    &mut len as *mut DWORD,
                    &mut index as *mut DWORD
                );
            };
            status_code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wininet;

    #[test]
    fn it_works() {
        let internet = wininet::Internet::open("agent", None).unwrap();
        let response = internet.get("http://example.com/", None).unwrap();
        assert_eq!(response.status(), 200);
        assert!(response.as_bytes().map(|f| f as char).collect::<String>().find("<h1>Example Domain</h1>").is_some()) ;
    }
}
