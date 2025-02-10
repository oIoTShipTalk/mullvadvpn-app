#![warn(clippy::undocumented_unsafe_blocks)]

use std::sync::{mpsc, OnceLock};

use neon::prelude::*;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::SLGP_UNCPRIORITY;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use windows_core::*;

static THREAD_SENDER: OnceLock<mpsc::Sender<Message>> = OnceLock::new();

enum Message {
    ResolveShortcut {
        path: String,
        result_tx: mpsc::Sender<Result<Option<String>>>,
    },
}

#[neon::main]
fn main(mut cx: ModuleContext<'_>) -> NeonResult<()> {
    cx.export_function("readShortcut", read_shortcut)?;

    Ok(())
}

fn read_shortcut(mut cx: FunctionContext<'_>) -> JsResult<'_, JsValue> {
    let tx = THREAD_SENDER.get_or_init(move || {
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let _com = ComContext::new()?;

            while let Ok(msg) = rx.recv() {
                match msg {
                    Message::ResolveShortcut { path, result_tx } => {
                        let _ = result_tx.send(get_shortcut_path(&path));
                    }
                }
            }

            Ok::<_, Error>(())
        });

        tx
    });

    let link_path = cx.argument::<JsString>(0)?.value(&mut cx);

    //let path = get_shortcut_path(&link_path);
    let (result_tx, result_rx) = mpsc::channel();
    // TODO: handle err
    let _ = tx.send(Message::ResolveShortcut {
        path: link_path,
        result_tx,
    });

    let Ok(result) = result_rx.recv() else {
        // TODO
        return Ok(cx.null().as_value(&mut cx));
    };

    match result {
        Ok(Some(path)) => Ok(cx.string(path).as_value(&mut cx)),
        _ => Ok(cx.null().as_value(&mut cx)),
    }
}

fn get_shortcut_path(path: &str) -> Result<Option<String>> {
    let path = HSTRING::from(path);
    let shell_link_result: Result<IShellLinkW> =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) };
    let Ok(shell_link) = shell_link_result else {
        // TODO: return err
        return Ok(None);
    };
    let persist_file_result: Result<IPersistFile> = shell_link.cast();
    let Ok(persist_file) = persist_file_result else {
        // TODO: return err
        return Ok(None);
    };
    unsafe {
        persist_file.Load(PCWSTR(path.as_ptr()), STGM_READ)?;
    }

    // TODO: what's a good len?
    let mut target_buffer = [0u16; 2 * 1024];
    unsafe {
        shell_link.GetPath(
            &mut target_buffer,
            std::ptr::null_mut(),
            SLGP_UNCPRIORITY.0 as u32,
        )?;
    }

    Ok(strip_null_terminator(&target_buffer))
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
