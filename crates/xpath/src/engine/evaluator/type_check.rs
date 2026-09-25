//! Type checking for `XPath` `instance of` and related operations.

use crate::compiler::ir::SeqTypeIR;
use crate::engine::runtime::Error;
use crate::model::XdmNode;
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequenceStream};

use super::Vm;

impl<N: 'static + XdmNode + Clone> Vm<N> {
    pub(crate) fn instance_of_stream(stream: &XdmSequenceStream<N>, t: &SeqTypeIR) -> Result<bool, Error> {
        use crate::compiler::ir::{OccurrenceIR, SeqTypeIR};
        let mut c = stream.cursor();
        match t {
            SeqTypeIR::EmptySequence => Ok(c.next_item().is_none()),
            SeqTypeIR::Typed { item, occ } => {
                let mut count = 0usize;
                while let Some(it) = c.next_item() {
                    let it = it?;
                    count = count.saturating_add(1);
                    if !Self::item_matches_type(&it, item) {
                        return Ok(false);
                    }
                    match occ {
                        OccurrenceIR::One | OccurrenceIR::ZeroOrOne if count > 1 => {
                            return Ok(false);
                        }
                        _ => {}
                    }
                }
                match occ {
                    OccurrenceIR::One => Ok(count == 1),
                    OccurrenceIR::ZeroOrOne => Ok(count <= 1),
                    OccurrenceIR::ZeroOrMore => Ok(true),
                    OccurrenceIR::OneOrMore => Ok(count >= 1),
                }
            }
        }
    }

    pub(crate) fn item_matches_type(item: &XdmItem<N>, t: &crate::compiler::ir::ItemTypeIR) -> bool {
        use crate::compiler::ir::ItemTypeIR;
        use XdmItem::{Atomic, Node};
        match (item, t) {
            (_, ItemTypeIR::AnyItem) | (Node(_), ItemTypeIR::AnyNode) => true,
            (Atomic(_), ItemTypeIR::AnyNode | ItemTypeIR::Kind(_)) | (Node(_), ItemTypeIR::Atomic(_)) => false,
            (Node(n), ItemTypeIR::Kind(k)) => Self::node_test(n, &k.clone()), // reuse existing node_test via IR NodeTestIR
            (Atomic(a), ItemTypeIR::Atomic(exp)) => Self::atomic_matches_name(a, exp),
        }
    }

    pub(crate) fn atomic_matches_name(a: &XdmAtomicValue, exp: &crate::xdm::ExpandedName) -> bool {
        use XdmAtomicValue::{
            AnyUri, Base64Binary, Boolean, Byte, Date, DateTime, DayTimeDuration, Decimal, Double, Entity, Float,
            HexBinary, Id, IdRef, Int, Integer, Language, Long, NCName, NMTOKEN, Name, NegativeInteger,
            NonNegativeInteger, NonPositiveInteger, NormalizedString, Notation, PositiveInteger, QName, Short, String,
            Time, Token, UnsignedByte, UnsignedInt, UnsignedLong, UnsignedShort, UntypedAtomic, YearMonthDuration,
        };
        // Only recognize XML Schema built-ins (xs:*). Unknown namespaces do not match.
        let xs_ns = crate::consts::XS;
        if let Some(ns) = &exp.ns_uri
            && ns.as_str() != xs_ns
        {
            return false;
        }
        match exp.local.as_str() {
            // Supertype
            "anyAtomicType" => true,

            // Primitives
            "string" => matches!(
                a,
                String(_)
                    | NormalizedString(_)
                    | Token(_)
                    | Language(_)
                    | Name(_)
                    | NCName(_)
                    | NMTOKEN(_)
                    | Id(_)
                    | IdRef(_)
                    | Entity(_)
            ),
            "boolean" => matches!(a, Boolean(_)),
            "decimal" => matches!(
                a,
                Decimal(_)
                    | Integer(_)
                    | Long(_)
                    | Int(_)
                    | Short(_)
                    | Byte(_)
                    | UnsignedLong(_)
                    | UnsignedInt(_)
                    | UnsignedShort(_)
                    | UnsignedByte(_)
                    | NonPositiveInteger(_)
                    | NegativeInteger(_)
                    | NonNegativeInteger(_)
                    | PositiveInteger(_)
            ),
            "integer" => matches!(
                a,
                Integer(_)
                    | Long(_)
                    | Int(_)
                    | Short(_)
                    | Byte(_)
                    | UnsignedLong(_)
                    | UnsignedInt(_)
                    | UnsignedShort(_)
                    | UnsignedByte(_)
                    | NonPositiveInteger(_)
                    | NegativeInteger(_)
                    | NonNegativeInteger(_)
                    | PositiveInteger(_)
            ),
            "double" => matches!(a, Double(_)),
            "float" => matches!(a, Float(_)),
            "anyURI" => matches!(a, AnyUri(_)),
            "QName" => matches!(a, QName { .. }),
            "NOTATION" => matches!(a, Notation(_)),

            // Untyped
            "untypedAtomic" => matches!(a, UntypedAtomic(_)),

            // Binary
            "base64Binary" => matches!(a, Base64Binary(_)),
            "hexBinary" => matches!(a, HexBinary(_)),

            // Temporal and durations
            "dateTime" => matches!(a, DateTime(_)),
            "date" => matches!(a, Date { .. }),
            "time" => matches!(a, Time { .. }),
            "yearMonthDuration" => matches!(a, YearMonthDuration(_)),
            "dayTimeDuration" => matches!(a, DayTimeDuration(_)),

            // String-derived specifics (covered by string above but allow exact tests)
            "normalizedString" => matches!(a, NormalizedString(_)),
            "token" => matches!(a, Token(_)),
            "language" => matches!(a, Language(_)),
            "Name" => matches!(a, Name(_)),
            "NCName" => matches!(a, NCName(_)),
            "NMTOKEN" => matches!(a, NMTOKEN(_)),
            "ID" => matches!(a, Id(_)),
            "IDREF" => matches!(a, IdRef(_)),
            "ENTITY" => matches!(a, Entity(_)),

            // Unknown atomic type name -> no match
            _ => false,
        }
    }
}
