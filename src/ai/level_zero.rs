//! Intel Level Zero API - Bare Metal FFI Bindings
//! 
//! This module provides the raw C bindings to interface directly with the
//! Intel Arc 140T GPU (Xe-2) and the AI Boost NPU 3.0 via the Level Zero API.
//! This allows us to orchestrate hardware execution and Unified Shared Memory (USM)
//! directly from our Rust microkernel without any OS overhead.

#![allow(non_camel_case_types)]
#![allow(dead_code)]

use core::ffi::c_void;

// --- Basic Level Zero Types ---

pub type ze_result_t = u32;

// Success
pub const ZE_RESULT_SUCCESS: ze_result_t = 0;

// Handles
pub type ze_driver_handle_t = *mut c_void;
pub type ze_device_handle_t = *mut c_void;
pub type ze_context_handle_t = *mut c_void;
pub type ze_command_queue_handle_t = *mut c_void;
pub type ze_command_list_handle_t = *mut c_void;

// Enums and Structs
#[repr(C)]
pub struct ze_context_desc_t {
    pub stype: u32,
    pub pNext: *const c_void,
    pub flags: u32,
}

#[repr(C)]
pub struct ze_device_mem_alloc_desc_t {
    pub stype: u32,
    pub pNext: *const c_void,
    pub flags: u32,
    pub ordinal: u32,
}

#[repr(C)]
pub struct ze_host_mem_alloc_desc_t {
    pub stype: u32,
    pub pNext: *const c_void,
    pub flags: u32,
}

// Error code for uninitialized hardware / driver not loaded in bare metal
pub const ZE_RESULT_ERROR_UNINITIALIZED: ze_result_t = 0x00010001;

// --- Bare-Metal Kernel Stubs (Resolves Linker Symbols) ---

#[no_mangle]
pub unsafe extern "C" fn zeInit(_flags: u32) -> ze_result_t {
    ZE_RESULT_ERROR_UNINITIALIZED
}

#[no_mangle]
pub unsafe extern "C" fn zeDriverGet(
    pCount: *mut u32,
    _phDrivers: *mut ze_driver_handle_t,
) -> ze_result_t {
    if !pCount.is_null() {
        *pCount = 0;
    }
    ZE_RESULT_ERROR_UNINITIALIZED
}

#[no_mangle]
pub unsafe extern "C" fn zeDeviceGet(
    _hDriver: ze_driver_handle_t,
    pCount: *mut u32,
    _phDevices: *mut ze_device_handle_t,
) -> ze_result_t {
    if !pCount.is_null() {
        *pCount = 0;
    }
    ZE_RESULT_ERROR_UNINITIALIZED
}

#[no_mangle]
pub unsafe extern "C" fn zeContextCreate(
    _hDriver: ze_driver_handle_t,
    _desc: *const ze_context_desc_t,
    _phContext: *mut ze_context_handle_t,
) -> ze_result_t {
    ZE_RESULT_ERROR_UNINITIALIZED
}

#[no_mangle]
pub unsafe extern "C" fn zeMemAllocShared(
    _hContext: ze_context_handle_t,
    _device_desc: *const ze_device_mem_alloc_desc_t,
    _host_desc: *const ze_host_mem_alloc_desc_t,
    _size: usize,
    _alignment: usize,
    _hDevice: ze_device_handle_t,
    _pptr: *mut *mut c_void,
) -> ze_result_t {
    ZE_RESULT_ERROR_UNINITIALIZED
}

#[no_mangle]
pub unsafe extern "C" fn zeMemFree(
    _hContext: ze_context_handle_t,
    _ptr: *mut c_void,
) -> ze_result_t {
    ZE_RESULT_ERROR_UNINITIALIZED
}
