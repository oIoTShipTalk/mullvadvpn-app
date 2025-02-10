#![warn(clippy::undocumented_unsafe_blocks)]

use lnk_parser::LNKParser;
use neon::prelude::*;

#[neon::main]
fn main(mut cx: ModuleContext<'_>) -> NeonResult<()> {
    cx.export_function("readShorcut", read_shortcut)?;
    Ok(())
}

fn read_shortcut(mut cx: FunctionContext<'_>) -> JsResult<'_, JsValue> {
    let link_path = cx.argument::<JsString>(0)?.value(&mut cx);
    let Ok(link) = LNKParser::from_path(&link_path) else {
        return cx.throw_error(format!("Failed to parse shortcut: {}", link_path));
    };

    let Some(target) = link.get_target_full_path() else {
        return Ok(cx.null().as_value(&mut cx));
    };

    Ok(cx.string(target).as_value(&mut cx))
}
