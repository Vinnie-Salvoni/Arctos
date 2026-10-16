use super::level_zero::*;
use core::ptr;

/// A safe Rust wrapper for the Intel Arc 140T GPU context.
pub struct ArcHardwareContext {
    driver: ze_driver_handle_t,
    device: ze_device_handle_t,
    context: ze_context_handle_t,
}

impl ArcHardwareContext {
    /// Initializes the Level Zero driver and attempts to find an Arc GPU.
    pub fn initialize() -> Result<Self, &'static str> {
        unsafe {
            // 1. Initialize Driver
            if zeInit(0) != ZE_RESULT_SUCCESS {
                return Err("Failed to initialize Level Zero API");
            }

            // 2. Get Driver Handle
            let mut driver_count: u32 = 0;
            if zeDriverGet(&mut driver_count, ptr::null_mut()) != ZE_RESULT_SUCCESS || driver_count == 0 {
                return Err("No Level Zero driver found");
            }
            
            let mut driver: ze_driver_handle_t = ptr::null_mut();
            if zeDriverGet(&mut driver_count, &mut driver) != ZE_RESULT_SUCCESS {
                return Err("Failed to acquire driver handle");
            }

            // 3. Get Device Handle (Arc 140T)
            let mut device_count: u32 = 0;
            if zeDeviceGet(driver, &mut device_count, ptr::null_mut()) != ZE_RESULT_SUCCESS || device_count == 0 {
                return Err("No Arc GPU found on driver");
            }

            let mut device: ze_device_handle_t = ptr::null_mut();
            if zeDeviceGet(driver, &mut device_count, &mut device) != ZE_RESULT_SUCCESS {
                return Err("Failed to acquire device handle");
            }

            // 4. Create Context
            let context_desc = ze_context_desc_t {
                stype: 0, // In C++ this maps to ZE_STRUCTURE_TYPE_CONTEXT_DESC
                pNext: ptr::null(),
                flags: 0,
            };
            let mut context: ze_context_handle_t = ptr::null_mut();
            
            if zeContextCreate(driver, &context_desc, &mut context) != ZE_RESULT_SUCCESS {
                return Err("Failed to create Level Zero context");
            }

            Ok(Self { driver, device, context })
        }
    }

    /// Allocates Unified Shared Memory (USM) visible to both CPU and Arc GPU.
    pub fn allocate_usm(&self, size: usize) -> Result<*mut f32, &'static str> {
        let device_desc = ze_device_mem_alloc_desc_t {
            stype: 0,
            pNext: ptr::null(),
            flags: 0,
            ordinal: 0,
        };
        let host_desc = ze_host_mem_alloc_desc_t {
            stype: 0,
            pNext: ptr::null(),
            flags: 0,
        };

        let mut out_ptr: *mut core::ffi::c_void = ptr::null_mut();
        
        unsafe {
            if zeMemAllocShared(
                self.context,
                &device_desc,
                &host_desc,
                size,
                8, // 8-byte alignment
                self.device,
                &mut out_ptr,
            ) != ZE_RESULT_SUCCESS {
                return Err("Failed to allocate USM memory");
            }
        }
        
        Ok(out_ptr as *mut f32)
    }
}
