// Bin target sharing the crate's dependency list: `windows`/`libloading` serve only the `jab_spike`
// bin, and the platform crates are linked by `platynui_link_os_providers!` in non-test builds only.
#![allow(unused_crate_dependencies)]

use std::{thread::sleep, time::Duration};

use platynui_core::platform::HighlightRequest;
use platynui_link::platynui_link_os_providers;
use platynui_runtime::{EvaluationItem, Runtime};

platynui_link_os_providers!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new()?;
    println!("Known Key Names: {:?}", runtime.keyboard_known_key_names());
    if let Some(result) = runtime.evaluate_single(None, "Window[ends-with(@Name,'Notepad')]")? {
        if let EvaluationItem::Node(notepad) = result {
            println!("Found Notepad window with runtime ID: {}", notepad.runtime_id());
            runtime.focus(&notepad)?;

            let _attr = notepad.attribute(platynui_core::ui::Namespace::Control, "Bounds");
            if let Some(attribute) = notepad.attribute(platynui_core::ui::Namespace::Control, "Bounds") {
                let value = attribute.value();
                if let platynui_core::ui::UiValue::Rect(bounds) = value
                    && !bounds.is_empty()
                {
                    let req = HighlightRequest::new(bounds);
                    let _ = runtime.highlight(&req);

                    runtime.focus(&notepad)?;
                    runtime.keyboard_type("<Control+a><Delete>", None)?;
                    runtime.keyboard_type("Hallo Welt<Return>", None)?;
                    runtime.keyboard_type("öäüÖÄÜ<Return>", None)?;
                    runtime.keyboard_type("<Up><Delete><Delete><Delete><Delete><Delete><Delete><Delete>", None)?;
                    runtime.keyboard_type("µ@€<Return>", None)?;
                    runtime.keyboard_type("Hi\nDu\nDa", None)?;

                    sleep(Duration::from_millis(500));
                    runtime.keyboard_type("<ESC>", None)?;
                    //sleep(Duration::from_millis(1500));
                }
            }
        }
    } else {
        println!("Notepad window not found");
    }

    Ok(())
}
