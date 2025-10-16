#![allow(clippy::arc_with_non_send_sync)]
slint::include_modules!();
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};
use ui::tree::{
    data::{TreeData, uinode::UiNodeData},
    viewmodel::ViewModel,
};

use platynui_core::platform::HighlightRequest;
use platynui_core::ui::node::UiNodeExt;
use platynui_core::ui::pattern::{WindowSurfaceActions, WindowSurfacePattern};
use platynui_core::ui::{Namespace, UiValue};
use platynui_link::platynui_link_providers;
use platynui_runtime::Runtime;
use std::time::Duration;

platynui_link_providers!();

mod ui;

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    // Create PlatynUI runtime and get desktop node - keep runtime alive for entire application lifetime
    let runtime = Runtime::new().map_err(|e| {
        eprintln!("Failed to create PlatynUI runtime: {}", e);
        slint::PlatformError::Other(format!("Runtime creation failed: {}", e))
    })?;
    let runtime: Rc<Runtime> = Rc::new(runtime);

    let desktop_node = runtime.desktop_node();
    let root_data: Arc<dyn TreeData<Underlying = Arc<dyn platynui_core::ui::UiNode>>> =
        Arc::new(UiNodeData::new(desktop_node));

    let adapter: Rc<RefCell<ViewModel>> = Rc::new(RefCell::new(ViewModel::new(root_data)));
    main_window.set_tree_model(adapter.borrow().visible_model());

    // Handle expand/collapse + lazy load requests (index-based only)
    let adapter1b = Rc::clone(&adapter);
    main_window.on_tree_node_toggled_index(move |index, expanded| {
        adapter1b.borrow_mut().toggle_index(index as usize, expanded);
    });
    // Request children (index-based)
    let adapter2b = Rc::clone(&adapter);
    main_window.on_tree_request_children_index(move |index| {
        adapter2b.borrow_mut().request_children_index(index as usize);
    });

    // Refresh a specific row (index-based): clear caches under that node and rebuild
    let adapter_refresh = Rc::clone(&adapter);
    main_window.on_tree_refresh_index(move |idx| {
        // Clear caches for this node then rebuild visible rows
        let mut vm = adapter_refresh.borrow_mut();
        vm.refresh_row(idx as usize);
        vm.force_rebuild();
    });
    let adapter_refresh2 = Rc::clone(&adapter);
    main_window.on_tree_refresh_subtree_index(move |idx| {
        let mut vm = adapter_refresh2.borrow_mut();
        vm.refresh_row_recursive(idx as usize);
        vm.force_rebuild();
    });

    // Index-based selection
    let adapter5 = Rc::clone(&adapter);
    let runtime_for_select = Rc::clone(&runtime);
    main_window.on_tree_node_selected_index(move |index| {
        if let Some(node) = adapter5.borrow().resolve_node_by_index(index as usize) {
            let name = node.name();
            let role = node.role();
            eprintln!("Selected[idx={}]: role={} name={}", index, role, name);

            // Try to highlight the node's Bounds if available
            if let Some(attr) = node.attribute(Namespace::Control, "Bounds") {
                match attr.value() {
                    UiValue::Rect(bounds) if !bounds.is_empty() => {
                        if let Some(window) = node.top_level_or_self().pattern::<WindowSurfaceActions>() {
                            let _ = window.activate();
                        }
                        let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
                        if let Err(err) = runtime_for_select.highlight(&[req]) {
                            eprintln!("Highlight error: {}", err);
                        }
                    }
                    _ => {
                        // Clear existing highlight if selection has no usable bounds
                        let _ = runtime_for_select.clear_highlight();
                    }
                }
            } else {
                let _ = runtime_for_select.clear_highlight();
            }
        }
    });

    main_window.on_exit_requested(move || {
        let _ = slint::quit_event_loop();
    });

    main_window.run()
}
