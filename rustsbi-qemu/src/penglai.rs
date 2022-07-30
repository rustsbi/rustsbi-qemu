/* todo: move to separate crate like sbi_spec? */
pub use host::EID_PENGLAI_HOST;
use rustsbi::spec::binary::SbiRet;

mod host {
    pub const EID_PENGLAI_HOST: usize /*u32*/ = 0x100100;

    pub const CREATE_ENCLAVE: usize = 99;
    pub const ATTEST_ENCLAVE: usize = 98;
    pub const RUN_ENCLAVE: usize = 97;
    pub const STOP_ENCLAVE: usize = 96;
    pub const RESUME_ENCLAVE: usize = 95;
    pub const DESTROY_ENCALVE: usize = 94;

    #[repr(C)]
    pub struct EnclaveCreate {
        pub enclave_id_ptr: usize, /* todo: what's this? */
        pub enclave_name: [u8; 16],
        pub enclave_type: EnclaveType,
        pub enclave_physical_address: usize,
        pub enclave_length: usize,
        pub entry_point: usize,
        pub free_memory: usize, // what's this?
        pub kbuffer_physical_address: usize,
        pub kbuffer_length: usize,
        pub shared_mem_physical_address: usize,
        pub shared_mem_length: usize,
        pub ecall_argument: [usize; 4],
        pub return_value: usize,
    }

    #[repr(C)]
    pub struct EnclaveRun {
        pub memory_argument_address: usize,
        pub memory_argument_size: usize,
        // todo: extended arguments
    }

    #[repr(usize)]
    pub enum EnclaveType {
        Normal = 0,
        Server = 1,
    }

    #[repr(C)]
    pub struct ShangMiReport {
        pub hash: [u8; 32],
        pub signature: [u8; 64],
        pub public_key: [u8; 64],
    }

    #[repr(C)]
    pub struct EnclaveReport {
        pub hash: [u8; 32],
        pub signature: [u8; 64],
        pub nonce: usize,
    }

    #[repr(C)]
    pub struct Report {
        pub shangmi_report: ShangMiReport,
        pub enclave_report: EnclaveReport,
        pub device_public_key: [u8; 64],
    }
}

use host::*;

// todo: instance based? strctu Contect { penglai: Penglai, ... }

#[inline]
pub fn ecall(extension: usize, function: usize, param: [usize; 6]) -> SbiRet {
    match (extension, function) {
        (EID_PENGLAI_HOST, CREATE_ENCLAVE) => create_enclave(param[0]),
        _ => SbiRet::not_supported(),
    }
}

#[inline]
fn create_enclave(param: usize) -> SbiRet {
    todo!()
}
