#![no_std]
#![feature(likely_unlikely)]

use core::time::Duration;

use nx::{
    result::NxResult,
    thread::{read_ipc_buffer, write_ipc_buffer},
};

pub(crate) mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_service_api.rs"));
}

mod types;

#[repr(C)]
pub struct ServiceName([u8; 8]);

const _: () = {
    impl From<&[u8; 0]> for ServiceName {
        fn from(_: &[u8; 0]) -> Self {
            Self([0u8; 8])
        }
    }

    impl From<&[u8; 1]> for ServiceName {
        fn from(value: &[u8; 1]) -> Self {
            Self([value[0], 0, 0, 0, 0, 0, 0, 0])
        }
    }

    impl From<&[u8; 2]> for ServiceName {
        fn from(value: &[u8; 2]) -> Self {
            Self([value[0], value[1], 0, 0, 0, 0, 0, 0])
        }
    }

    impl From<&[u8; 3]> for ServiceName {
        fn from(value: &[u8; 3]) -> Self {
            Self([value[0], value[1], value[2], 0, 0, 0, 0, 0])
        }
    }

    impl From<&[u8; 4]> for ServiceName {
        fn from(value: &[u8; 4]) -> Self {
            Self([value[0], value[1], value[2], value[3], 0, 0, 0, 0])
        }
    }

    impl From<&[u8; 5]> for ServiceName {
        fn from(value: &[u8; 5]) -> Self {
            Self([value[0], value[1], value[2], value[3], value[4], 0, 0, 0])
        }
    }

    impl From<&[u8; 6]> for ServiceName {
        fn from(value: &[u8; 6]) -> Self {
            Self([
                value[0], value[1], value[2], value[3], value[4], value[5], 0, 0,
            ])
        }
    }

    impl From<&[u8; 7]> for ServiceName {
        fn from(value: &[u8; 7]) -> Self {
            Self([
                value[0], value[1], value[2], value[3], value[4], value[5], value[6], 0,
            ])
        }
    }

    impl From<&[u8; 8]> for ServiceName {
        fn from(value: &[u8; 8]) -> Self {
            Self([
                value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
            ])
        }
    }
};

pub struct ServiceManager(u32);

impl ServiceManager {
    /// Constructs a new service manager
    ///
    /// # Notes
    /// This will fail if it is unable to acquire a handle to the `sm:` port, which might happen
    /// if a `ServiceManager` already exists
    pub fn new() -> NxResult<Self> {
        loop {
            match nx::svc::connect_to_named_port(c"sm:") {
                Ok(handle) => {
                    break Ok(Self(handle));
                }
                Err(nx::result::svc::NOT_FOUND) => nx::svc::sleep_thread(Duration::from_millis(50)),
                Err(e) => break Err(e),
            }
        }
    }

    /// Registers this process as a client of the `ServiceManager`
    pub fn register_client(&self) -> Result<(), u32> {
        use generated::service_manager::*;
        nx::thread::write_ipc_buffer(RegisterClientRequest::new(0));

        nx::svc::send_sync_request(self.0).unwrap();

        let response: RegisterClientResponse = unsafe { nx::thread::read_ipc_buffer() };

        if core::hint::likely(response.result() == 0) {
            Ok(())
        } else {
            Err(response.result())
        }
    }

    pub fn get_service_handle(&self, name: impl Into<ServiceName>) -> Result<u32, u32> {
        use generated::service_manager::*;
        write_ipc_buffer(GetServiceHandleRequest::new(name.into()));

        nx::svc::send_sync_request(self.0).unwrap();

        let response: GetServiceHandleResponse = unsafe { read_ipc_buffer() };
        if core::hint::likely(response.result() == 0) {
            Ok(response.service())
        } else {
            Err(response.result())
        }
    }

    pub fn register_service<T>(
        &self,
        name: impl Into<ServiceName>,
        is_light: bool,
        max_sessions: i32,
    ) -> Result<u32, u32> {
        use generated::service_manager::*;
        write_ipc_buffer(RegisterServiceRequest::new(
            name.into(),
            is_light,
            max_sessions,
        ));

        nx::svc::send_sync_request(self.0).unwrap();

        let response: RegisterServiceResponse = unsafe { read_ipc_buffer() };

        if core::hint::likely(response.result() == 0) {
            Ok(response.service())
        } else {
            Err(response.result())
        }
    }

    pub fn unregister_service(&self, name: impl Into<ServiceName>) -> Result<(), u32> {
        use generated::service_manager::*;
        write_ipc_buffer(UnregisterServiceRequest::new(name.into()));

        nx::svc::send_sync_request(self.0).unwrap();

        let response: UnregisterServiceResponse = unsafe { read_ipc_buffer() };
        if core::hint::likely(response.result() == 0) {
            Ok(())
        } else {
            Err(response.result())
        }
    }

    pub fn detach_client(&self) -> Result<(), u32> {
        use generated::service_manager::*;
        write_ipc_buffer(DetachClientRequest::new(0));

        nx::svc::send_sync_request(self.0).unwrap();

        let response: DetachClientResponse = unsafe { read_ipc_buffer() };

        if core::hint::likely(response.result() == 0) {
            Ok(())
        } else {
            Err(response.result())
        }
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        // TODO: Lifetime guarantees?
        nx::svc::close_handle(self.0).unwrap();
    }
}
