//! Effective Boolean Value (EBV) per XPath 2.0 §2.4.3.
//!
//! This module provides the canonical EBV implementation used by the evaluator
//! (And/Or/Not/ToEBV/JumpIf), built-in functions (`fn:boolean`, `fn:not`),
//! and predicate evaluation.
//!
//! # XPath 2.0 EBV Rules (in priority order)
//!
//! 1. Empty sequence → `false`
//! 2. First item is a node → `true`
//! 3. Singleton `xs:boolean` → its value
//! 4. Singleton `xs:string` or `xs:anyURI` → `false` if empty, else `true`
//! 5. Singleton numeric → `false` if `0` or `NaN`, else `true`
//! 6. Singleton `xs:untypedAtomic` → `false` if empty, else `true`
//! 7. Otherwise → error `FORG0006`
//!
//! Rule 2 short-circuits: once the first item is a node, the result is `true`
//! regardless of any subsequent items.

use crate::engine::runtime::{Error, ErrorCode};
use crate::xdm::{SequenceCursor, XdmAtomicValue, XdmItem};

/// Compute the EBV of a single atomic value (rules 3–7).
pub fn ebv_of_atomic(a: &XdmAtomicValue) -> Result<bool, Error> {
    match a {
        XdmAtomicValue::Boolean(b) => Ok(*b),
        XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => Ok(!s.is_empty()),
        XdmAtomicValue::Integer(i) => Ok(*i != 0),
        XdmAtomicValue::Decimal(d) => Ok(!d.is_zero()),
        XdmAtomicValue::Double(d) => Ok(*d != 0.0 && !d.is_nan()),
        XdmAtomicValue::Float(f) => Ok(*f != 0.0 && !f.is_nan()),
        _ => Err(Error::from_code(ErrorCode::FORG0006, "effective boolean value not defined for this atomic type")),
    }
}

/// Compute the EBV of a streaming sequence (rules 1–7).
///
/// Only reads as many items as needed:
/// - Empty → `false` (reads nothing)
/// - First item is a node → `true` (reads one item, rest is ignored)
/// - First item is an atomic → compute EBV, then verify no second item exists
pub fn ebv_of_stream<N>(cursor: &mut dyn SequenceCursor<N>) -> Result<bool, Error> {
    let first = match cursor.next_item() {
        None => return Ok(false),
        Some(result) => result?,
    };

    match first {
        // Rule 2: first item is a node → true (short-circuit, ignore rest)
        XdmItem::Node(_) => Ok(true),
        // Rules 3–7: first item is atomic
        XdmItem::Atomic(ref a) => {
            // If there is a second item, it's an error (length > 1 starting with atomic)
            if let Some(second) = cursor.next_item() {
                // Propagate cursor errors before reporting the EBV error
                second?;
                return Err(Error::from_code(ErrorCode::FORG0006, "effective boolean value of sequence of length > 1"));
            }
            ebv_of_atomic(a)
        }
    }
}
