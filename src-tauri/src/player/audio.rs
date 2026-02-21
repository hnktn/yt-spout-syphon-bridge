/// オーディオデバイス列挙
///
/// macOS: CoreAudio の AudioObjectGetPropertyData を使って列挙
/// mpv が起動していない状態でも使用可能

pub fn enumerate_devices() -> Vec<(String, String)> {
    #[cfg(target_os = "macos")]
    {
        enumerate_coreaudio()
    }

    #[cfg(not(target_os = "macos"))]
    {
        vec![("auto".to_string(), "デフォルト".to_string())]
    }
}

#[cfg(target_os = "macos")]
fn enumerate_coreaudio() -> Vec<(String, String)> {
    use std::ffi::CStr;
    use std::mem;

    // CoreAudio の型・定数を直接定義（coreaudio-sys 不要）
    type AudioObjectID = u32;
    type AudioObjectPropertySelector = u32;
    type AudioObjectPropertyScope = u32;
    type AudioObjectPropertyElement = u32;
    type OSStatus = i32;

    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope,
        element: AudioObjectPropertyElement,
    }

    const K_AUDIO_OBJECT_SYSTEM_OBJECT: AudioObjectID = 1;
    const K_AUDIO_HARDWARE_PROPERTY_DEVICES: AudioObjectPropertySelector = u32::from_be_bytes(*b"dev#");
    const K_AUDIO_DEVICE_PROPERTY_DEVICE_NAME_CF_STRING: AudioObjectPropertySelector = u32::from_be_bytes(*b"lnam");
    const K_AUDIO_DEVICE_PROPERTY_DEVICE_UID: AudioObjectPropertySelector = u32::from_be_bytes(*b"uid ");
    const K_AUDIO_DEVICE_PROPERTY_STREAMS: AudioObjectPropertySelector = u32::from_be_bytes(*b"stm#");
    const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: AudioObjectPropertyScope = u32::from_be_bytes(*b"glob");
    const K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT: AudioObjectPropertyScope = u32::from_be_bytes(*b"outp");
    const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: AudioObjectPropertyElement = 0;

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyDataSize(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            out_data_size: *mut u32,
        ) -> OSStatus;

        fn AudioObjectGetPropertyData(
            object_id: AudioObjectID,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            io_data_size: *mut u32,
            out_data: *mut std::ffi::c_void,
        ) -> OSStatus;
    }

    // CFStringRef から String に変換するヘルパー
    fn cfstring_to_string(cf_str: *mut std::ffi::c_void) -> Option<String> {
        if cf_str.is_null() {
            return None;
        }
        // CFStringGetCString で UTF-8 に変換
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFStringGetCString(
                the_string: *mut std::ffi::c_void,
                buffer: *mut std::os::raw::c_char,
                buffer_size: i64,
                encoding: u32,
            ) -> bool;
            fn CFRelease(cf: *mut std::ffi::c_void);
        }
        const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
        let mut buf = vec![0i8; 256];
        let ok = unsafe {
            CFStringGetCString(cf_str, buf.as_mut_ptr(), buf.len() as i64, K_CF_STRING_ENCODING_UTF8)
        };
        unsafe { CFRelease(cf_str) };
        if ok {
            let cstr = unsafe { CStr::from_ptr(buf.as_ptr()) };
            cstr.to_str().ok().map(|s| s.to_string())
        } else {
            None
        }
    }

    let mut devices = vec![("auto".to_string(), "システムデフォルト".to_string())];

    unsafe {
        // デバイス ID 一覧を取得
        let addr = AudioObjectPropertyAddress {
            selector: K_AUDIO_HARDWARE_PROPERTY_DEVICES,
            scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };

        let mut data_size: u32 = 0;
        let status = AudioObjectGetPropertyDataSize(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut data_size,
        );
        if status != 0 {
            log::error!("AudioObjectGetPropertyDataSize failed: {}", status);
            return devices;
        }

        let device_count = data_size as usize / mem::size_of::<AudioObjectID>();
        let mut device_ids: Vec<AudioObjectID> = vec![0; device_count];
        let status = AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut data_size,
            device_ids.as_mut_ptr() as *mut _,
        );
        if status != 0 {
            log::error!("AudioObjectGetPropertyData (devices) failed: {}", status);
            return devices;
        }

        for &device_id in &device_ids {
            // 出力ストリームがあるデバイスのみを対象にする
            let stream_addr = AudioObjectPropertyAddress {
                selector: K_AUDIO_DEVICE_PROPERTY_STREAMS,
                scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT,
                element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
            };
            let mut stream_size: u32 = 0;
            let st = AudioObjectGetPropertyDataSize(
                device_id,
                &stream_addr,
                0,
                std::ptr::null(),
                &mut stream_size,
            );
            if st != 0 || stream_size == 0 {
                // 出力ストリームなし → スキップ
                continue;
            }

            // デバイス UID（mpv の audio-device 値として使う）
            let uid_addr = AudioObjectPropertyAddress {
                selector: K_AUDIO_DEVICE_PROPERTY_DEVICE_UID,
                scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
            };
            let mut uid_cf: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut uid_size = mem::size_of::<*mut std::ffi::c_void>() as u32;
            let st = AudioObjectGetPropertyData(
                device_id,
                &uid_addr,
                0,
                std::ptr::null(),
                &mut uid_size,
                &mut uid_cf as *mut _ as *mut _,
            );
            if st != 0 {
                continue;
            }
            let uid = match cfstring_to_string(uid_cf) {
                Some(s) => s,
                None => continue,
            };

            // デバイス表示名
            let name_addr = AudioObjectPropertyAddress {
                selector: K_AUDIO_DEVICE_PROPERTY_DEVICE_NAME_CF_STRING,
                scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
            };
            let mut name_cf: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut name_size = mem::size_of::<*mut std::ffi::c_void>() as u32;
            let st = AudioObjectGetPropertyData(
                device_id,
                &name_addr,
                0,
                std::ptr::null(),
                &mut name_size,
                &mut name_cf as *mut _ as *mut _,
            );
            if st != 0 {
                continue;
            }
            let name = cfstring_to_string(name_cf).unwrap_or_else(|| uid.clone());

            log::info!("オーディオ出力デバイス: {} ({})", name, uid);

            // mpv は CoreAudio UID を "coreaudio/<UID>" 形式で受け付ける
            devices.push((format!("coreaudio/{}", uid), name));
        }
    }

    log::info!("CoreAudio デバイス列挙完了: {} 件", devices.len());
    devices
}
