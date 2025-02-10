#![warn(clippy::undocumented_unsafe_blocks)]

use neon::prelude::*;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::SLGP_UNCPRIORITY;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use windows_core::*;

#[neon::main]
fn main(mut cx: ModuleContext<'_>) -> NeonResult<()> {
    cx.export_function("readShortcut", read_shortcut)?;
    Ok(())
}

fn read_shortcut(mut cx: FunctionContext<'_>) -> JsResult<'_, JsValue> {
    thread_local! {
        static COM: Result<ComContext> = ComContext::new()
    }

    let link_path = cx.argument::<JsString>(0)?.value(&mut cx);

    let path = get_shortcut_path(&link_path);

    match path {
        Some(path) => Ok(cx.string(path).as_value(&mut cx)),
        None => Ok(cx.null().as_value(&mut cx)),
    }
}

fn get_shortcut_path(path: &str) -> Option<String> {
    let path = HSTRING::from(path);
    let shell_link_result: Result<IShellLinkW> =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) };
    let Ok(shell_link) = shell_link_result else {
        // TODO: return err
        return None;
    };
    let persist_file_result: Result<IPersistFile> = shell_link.cast();
    let Ok(persist_file) = persist_file_result else {
        // TODO: return err
        return None;
    };
    unsafe {
        // TODO: return err
        persist_file.Load(PCWSTR(path.as_ptr()), STGM_READ).ok()?;
    }

    // TODO: what's a good len?
    let mut target_buffer = [0u16; 2 * 1024];
    unsafe {
        shell_link
            .GetPath(
                &mut target_buffer,
                std::ptr::null_mut(),
                SLGP_UNCPRIORITY.0 as u32,
            )
            .ok()?;
    }

    strip_null_terminator(&target_buffer)
}

fn strip_null_terminator(slice: &[u16]) -> Option<String> {
    let s = slice.split(|&c| c == 0).next()?;
    Some(String::from_utf16_lossy(&s))
}

struct ComContext {}

impl ComContext {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        };
        Ok(Self {})
    }
}

impl Drop for ComContext {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
