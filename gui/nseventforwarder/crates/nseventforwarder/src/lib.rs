use std::sync::Mutex;

use block2::RcBlock;
use neon::prelude::*;
use objc2_app_kit::{NSEvent, NSEventMask};

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("start", start)?;
    Ok(())
}

fn start(mut cx: FunctionContext) -> JsResult<JsFunction> {
    let callback = cx.argument::<JsFunction>(0)?;

    let (sender, receiver) = std::sync::mpsc::channel();

    let block = RcBlock::new(move |_event| {
        println!("1 Event received: {:?}", _event);
        let _ = sender.send(());
    });

    let _handler = unsafe {
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown,
            &block,
        )
    };

    let stop = Mutex::new(false);

    if let Ok(_) = receiver.recv() {
        let stop = *stop.lock().unwrap();
        if !stop {
            // break;
        }

        println!("2 Event received");
        let this = JsNull::new(&mut cx);
        let _ = callback.call(&mut cx, this, []);
    }

    JsFunction::new(&mut cx, move |mut cx| {
        let mut stop = stop.lock().unwrap();
        *stop = true;
        Ok(JsUndefined::new(&mut cx))
    })
}
