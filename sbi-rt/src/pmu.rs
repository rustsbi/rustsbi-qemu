//! Chapter 11. Performance Monitoring Unit Extension (EID #0x504D55 "PMU")

use crate::binary::{eid_from_str, sbi_call_0, sbi_call_1, sbi_call_3, SbiRet};

pub const EID_PMU: usize = eid_from_str("PMU") as _;

const FID_PMU_NUM_COUNTERS: usize = 0;
const FID_PMU_COUNTER_GET_INFO: usize = 1;
const FID_PMU_COUNTER_CONFIG_MATCHING: usize = 2;
const FID_PMU_COUNTER_START: usize = 3;
const FID_PMU_COUNTER_STOP: usize = 4;
const FID_PMU_COUNTER_FW_READ: usize = 5;

#[inline]
pub fn pmu_num_counters() -> SbiRet {
    sbi_call_0(EID_PMU, FID_PMU_NUM_COUNTERS)
}

#[inline]
pub fn pmu_counter_get_info(counter_idx: usize) -> SbiRet {
    sbi_call_1(EID_PMU, FID_PMU_COUNTER_GET_INFO, counter_idx)
}

#[inline]
pub fn pmu_counter_config_matching(
    counter_idx_base: usize,
    counter_idx_mask: usize,
    config_flags: usize,
    event_idx: usize,
    event_data: u64,
) -> SbiRet {
    match () {
        #[cfg(target_pointer_width = "32")]
        () => crate::binary::sbi_call_6(
            EID_PMU,
            FID_PMU_COUNTER_CONFIG_MATCHING,
            counter_idx_base,
            counter_idx_mask,
            config_flags,
            event_idx,
            event_data as _,
            (event_data >> 32) as _,
        ),
        #[cfg(target_pointer_width = "64")]
        () => crate::binary::sbi_call_5(
            EID_PMU,
            FID_PMU_COUNTER_CONFIG_MATCHING,
            counter_idx_base,
            counter_idx_mask,
            config_flags,
            event_idx,
            event_data as _,
        ),
    }
}

#[inline]
pub fn pmu_counter_start(
    counter_idx_base: usize,
    counter_idx_mask: usize,
    start_flags: usize,
    initial_value: u64,
) -> SbiRet {
    match () {
        #[cfg(target_pointer_width = "32")]
        () => crate::binary::sbi_call_5(
            EID_PMU,
            FID_PMU_COUNTER_START,
            counter_idx_base,
            counter_idx_mask,
            start_flags,
            initial_value as _,
            (initial_value >> 32) as _,
        ),
        #[cfg(target_pointer_width = "64")]
        () => crate::binary::sbi_call_4(
            EID_PMU,
            FID_PMU_COUNTER_START,
            counter_idx_base,
            counter_idx_mask,
            start_flags,
            initial_value as _,
        ),
    }
}

#[inline]
pub fn pmu_counter_stop(
    counter_idx_base: usize,
    counter_idx_mask: usize,
    stop_flags: usize,
) -> SbiRet {
    sbi_call_3(
        EID_PMU,
        FID_PMU_COUNTER_STOP,
        counter_idx_base,
        counter_idx_mask,
        stop_flags,
    )
}

#[inline]
pub fn pmu_counter_fw_read(counter_idx:usize) -> SbiRet {
    sbi_call_1(EID_PMU, FID_PMU_COUNTER_FW_READ, counter_idx)
}
