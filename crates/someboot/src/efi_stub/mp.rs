use arrayvec::ArrayVec;
use kernutil::StaticCell;
use uefi::boot;
use uefi::proto::unsafe_protocol;
use uefi::{Status, StatusExt};

static CPU_IDS: StaticCell<ArrayVec<usize, 256>> = StaticCell::new(ArrayVec::new_const());

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct EfiCpuPhysicalLocation {
    package: u32,
    core: u32,
    thread: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct EfiProcessorInformation {
    processor_id: u64,
    status_flag: u32,
    location: EfiCpuPhysicalLocation,
}

const PROCESSOR_ENABLED: u32 = 1;

#[unsafe_protocol("3fdda605-a76e-4f46-ad29-12f4531b3d08")]
#[repr(C)]
struct MpServices {
    get_number_of_processors: unsafe extern "efiapi" fn(
        this: *const MpServices,
        total: *mut usize,
        enabled: *mut usize,
    ) -> Status,
    get_processor_info: unsafe extern "efiapi" fn(
        this: *const MpServices,
        processor_number: usize,
        info: *mut EfiProcessorInformation,
    ) -> Status,

    // The following members are part of the protocol but are not used for enumeration.
    startup_all_aps: usize,
    startup_this_ap: usize,
    switch_bsp: usize,
    enable_disable_ap: usize,
    who_am_i: usize,
}

pub(crate) fn init_cpu_id_list() {
    let mut ids = ArrayVec::<usize, 256>::new();

    let Ok(handle) = boot::get_handle_for_protocol::<MpServices>() else {
        store(ids);
        return;
    };

    let Ok(mp) = boot::open_protocol_exclusive::<MpServices>(handle) else {
        store(ids);
        return;
    };

    let this = &*mp as *const MpServices;

    let mut total = 0usize;
    let mut enabled = 0usize;
    let status = unsafe { (mp.get_number_of_processors)(this, &mut total, &mut enabled) };
    if status.to_result().is_err() {
        store(ids);
        return;
    }

    for idx in 0..total {
        let mut info = EfiProcessorInformation::default();
        let status = unsafe { (mp.get_processor_info)(this, idx, &mut info) };
        if status.to_result().is_err() {
            continue;
        }
        if (info.status_flag & PROCESSOR_ENABLED) == 0 {
            continue;
        }
        let _ = ids.try_push(info.processor_id as usize);
    }

    store(ids);
}

pub(crate) fn cpu_id_list() -> Option<impl Iterator<Item = usize> + 'static> {
    let ids: &ArrayVec<usize, 256> = &CPU_IDS;
    (!ids.is_empty()).then(|| ids.iter().copied())
}

fn store(new_ids: ArrayVec<usize, 256>) {
    unsafe {
        CPU_IDS.update(|ids| {
            ids.clear();
            for id in new_ids {
                let _ = ids.try_push(id);
            }
        });
    }
}
