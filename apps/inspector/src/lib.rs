slint::include_modules!();
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};
use ui::tree::{
    data::{TreeData, uinode::UiNodeData},
    viewmodel::ViewModel,
};

use platynui_core::platform::HighlightRequest;
use platynui_core::ui::{Namespace, UiValue};
use platynui_link::platynui_link_providers;
use platynui_runtime::Runtime;
use std::time::Duration;

platynui_link_providers!();

mod ui;

pub fn run() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;

    // Create PlatynUI runtime and get desktop node - keep runtime alive for entire application lifetime
    let runtime = Runtime::new().map_err(|e| {
        eprintln!("Failed to create PlatynUI runtime: {}", e);
        slint::PlatformError::Other(format!("Runtime creation failed: {}", e))
    })?;
    let runtime: Arc<Runtime> = Arc::new(runtime);

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
    let runtime_for_select = Arc::clone(&runtime);
    let main_window_handle = main_window.as_weak();
    main_window.on_tree_node_selected_index(move |index| {
        if let Some(node) = adapter5.borrow().resolve_node_by_index(index as usize) {
            // Collect attributes and push into the Slint table model
            if let Some(win) = main_window_handle.upgrade() {
                use slint::{ModelRc, SharedString, StandardListViewItem, VecModel};
                // Helper to create a cell without struct literal (non_exhaustive)
                let cell_owned = |s: String| StandardListViewItem::from(SharedString::from(s));

                // Outer model (list of rows)
                let outer: Rc<VecModel<ModelRc<StandardListViewItem>>> = Rc::new(VecModel::default());

                // Small helper to push a row of three cells directly
                let push_row = |c1: String, c2: String, c3: String| {
                    let inner: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
                    inner.push(cell_owned(c1));
                    inner.push(cell_owned(c2));
                    inner.push(cell_owned(c3));
                    outer.push(ModelRc::from(inner));
                };

                // Dynamic attributes: stream directly into models
                // Also cache control:Bounds once if present to use it for highlighting later
                let mut cached_bounds: Option<platynui_core::types::Rect> = None;
                for attr in node.attributes() {
                    use std::fmt::Write as _;
                    let ns = attr.namespace();
                    let name = attr.name();
                    let value = attr.value();
                    // Capture control:Bounds if available and non-empty
                    if cached_bounds.is_none()
                        && let (Namespace::Control, "Bounds") = (ns, name)
                        && let UiValue::Rect(r) = &value
                        && !r.is_empty()
                    {
                        cached_bounds = Some(*r);
                    }
                    let (val_str, ty_str) = match value {
                        UiValue::Null => ("<null>".to_string(), "null".to_string()),
                        UiValue::Bool(b) => (b.to_string(), "bool".to_string()),
                        UiValue::Integer(i) => (i.to_string(), "integer".to_string()),
                        UiValue::Number(n) => (n.to_string(), "number".to_string()),
                        UiValue::String(s) => (s, "string".to_string()),
                        UiValue::Point(p) => (format!("{:.0}, {:.0}", p.x(), p.y()), "Point".to_string()),
                        UiValue::Size(s) => (format!("{:.0} x {:.0}", s.width(), s.height()), "Size".to_string()),
                        UiValue::Rect(r) => (
                            format!("{:.0}, {:.0}, {:.0}, {:.0}", r.x(), r.y(), r.width(), r.height()),
                            "Rect".to_string(),
                        ),
                        UiValue::Array(a) => {
                            let mut s = String::new();
                            s.push('[');
                            for (i, it) in a.into_iter().enumerate() {
                                if i > 0 {
                                    s.push_str(", ");
                                }
                                let _ = write!(
                                    &mut s,
                                    "{}",
                                    match it {
                                        UiValue::String(st) => st,
                                        _ => format!("{:?}", it),
                                    }
                                );
                            }
                            s.push(']');
                            (s, "array".to_string())
                        }
                        UiValue::Object(o) => {
                            let mut s = String::new();
                            s.push('{');
                            for (i, (k, v)) in o.into_iter().enumerate() {
                                if i > 0 {
                                    s.push_str(", ");
                                }
                                let _ = write!(&mut s, "{}: {:?}", k, v);
                            }
                            s.push('}');
                            (s, "object".to_string())
                        }
                    };
                    let ns_name = match ns {
                        Namespace::Control => "control",
                        Namespace::Item => "item",
                        Namespace::App => "app",
                        Namespace::Native => "native",
                    };
                    let full_name = format!("{}: {}", ns_name, name);
                    push_row(full_name, val_str, ty_str);
                }

                win.set_attr_rows(ModelRc::from(outer));

                // Try to highlight the cached bounds (if present); otherwise clear highlight
                if let Some(bounds) = cached_bounds {
                    let rt = Arc::clone(&runtime_for_select);
                    std::thread::spawn(move || {
                        let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
                        if let Err(err) = rt.highlight(&req) {
                            eprintln!("Highlight error: {}", err);
                        }
                    });
                } else {
                    let rt = Arc::clone(&runtime_for_select);
                    std::thread::spawn(move || {
                        let _ = rt.clear_highlight();
                    });
                }
            }
        }
    });

    main_window.on_exit_requested(move || {
        let _ = slint::quit_event_loop();
    });

    main_window.run()
}
