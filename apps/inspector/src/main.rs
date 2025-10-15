slint::include_modules!();
use crate::ui::tree::adapter::TreeViewAdapter;
use std::{cell::RefCell, rc::Rc};
use ui::tree::{
    data::{TreeData, uinode::UiNodeData},
    viewmodel::ViewModel,
};

use platynui_link::platynui_link_providers;
use platynui_runtime::Runtime;

platynui_link_providers!();

mod ui;

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    // Create PlatynUI runtime and get desktop node - keep runtime alive for entire application lifetime
    let runtime = Runtime::new().map_err(|e| {
        eprintln!("Failed to create PlatynUI runtime: {}", e);
        slint::PlatformError::Other(format!("Runtime creation failed: {}", e))
    })?;

    let desktop_node = runtime.desktop_node();
    let root_data: Box<dyn TreeData> = Box::new(UiNodeData::new(desktop_node));

    let adapter: Rc<RefCell<dyn TreeViewAdapter>> = Rc::new(RefCell::new(ViewModel::new(root_data)));
    main_window.set_tree_model(adapter.borrow().visible_model());

    // Handle expand/collapse + lazy load requests
    let main_weak = main_window.as_weak();
    let adapter1 = Rc::clone(&adapter);
    main_window.on_tree_node_toggled(move |node_id, expanded| {
        adapter1.borrow_mut().toggle(&node_id, expanded);
    });

    let adapter2 = Rc::clone(&adapter);
    main_window.on_tree_request_children(move |node_id| {
        adapter2.borrow_mut().request_children(&node_id);
    });

    let adapter3 = Rc::clone(&adapter);
    let main_weak2 = main_weak.clone();
    main_window.on_tree_request_parent(move |node_id| {
        if let Some(parent_id) = adapter3.borrow().parent_of(&node_id) {
            if let Some(mw) = main_weak2.upgrade() {
                mw.invoke_select_tree_node_id(parent_id);
            }
        }
    });

    main_window.on_exit_requested(move || {
        let _ = slint::quit_event_loop();
    });

    main_window.run()
}
