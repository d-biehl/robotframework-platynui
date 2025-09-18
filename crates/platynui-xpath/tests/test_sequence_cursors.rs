use platynui_xpath::xdm::{XdmItem, XdmSequenceStream};
use rstest::rstest;

fn sample_stream() -> XdmSequenceStream<&'static str> {
    let items = vec![
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String("a".into())),
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String("b".into())),
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String("c".into())),
    ];
    XdmSequenceStream::from_vec(items)
}

#[rstest]
fn vec_cursor_iteration() {
    let stream = sample_stream();
    let mut cursor = stream.cursor();
    assert_eq!(cursor.size_hint(), (3, Some(3)));

    let mut collected = Vec::new();
    while let Some(item) = cursor.next_item() {
        collected.push(item.unwrap());
    }

    assert_eq!(collected.len(), 3);
}

#[rstest]
fn vec_cursor_clone_preserves_position() {
    let stream = sample_stream();
    let mut cursor = stream.cursor();
    // advance one item
    let _ = cursor.next_item();
    assert_eq!(cursor.size_hint(), (2, Some(2)));

    let mut cloned = cursor.boxed_clone();
    let original_next = cursor.next_item().unwrap().unwrap();
    let cloned_next = cloned.next_item().unwrap().unwrap();

    assert_eq!(original_next, cloned_next);
}

#[rstest]
fn materialize_runs_tracing() {
    let stream = sample_stream();
    let materialized = stream.materialize().unwrap();
    assert_eq!(materialized.len(), 3);
}
