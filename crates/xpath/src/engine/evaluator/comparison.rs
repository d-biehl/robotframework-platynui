//! Atomic value comparison for XPath 2.0 value and general comparisons.

use chrono::TimeZone;
use crate::compiler::ir::ComparisonOp;
use crate::engine::runtime::{Error, ErrorCode};
use crate::model::XdmNode;
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

use super::numeric::{classify, unify_numeric};
use super::Vm;

impl<N: 'static + XdmNode + Clone> Vm<N> {
    pub(crate) fn atomize(seq: XdmSequence<N>) -> XdmSequence<N> {
        let mut out = Vec::with_capacity(seq.len());
        for it in seq {
            match it {
                XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a)),
                XdmItem::Node(n) => {
                    for atom in n.typed_value() {
                        out.push(XdmItem::Atomic(atom));
                    }
                }
            }
        }
        out
    }

    pub(crate) fn atomic_to_number(a: &XdmAtomicValue) -> Result<f64, Error> {
        Ok(match a {
            XdmAtomicValue::Integer(i) => *i as f64,
            XdmAtomicValue::Decimal(d) => {
                use rust_decimal::prelude::ToPrimitive;
                d.to_f64().unwrap_or(f64::NAN)
            }
            XdmAtomicValue::Double(d) => *d,
            XdmAtomicValue::Float(f) => *f as f64,
            XdmAtomicValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) => s.parse::<f64>().unwrap_or(f64::NAN),
            _ => f64::NAN,
        })
    }

    pub(crate) fn compare_atomic(
        &self,
        a: &XdmAtomicValue,
        b: &XdmAtomicValue,
        op: ComparisonOp,
    ) -> Result<bool, Error> {
        use ComparisonOp::*;
        // XPath 2.0 value comparison promotions (refined numeric path):
        // 1. untypedAtomic normalization (string or attempt numeric if other numeric)
        // 2. Numeric tower minimal promotion: integer + integer -> integer; integer + decimal -> decimal; decimal + float -> float;
        //    any + double -> double; float + float -> float; decimal + decimal -> decimal; integer + float -> float; etc.
        // 3. Boolean: only Eq/Ne allowed vs boolean; relational ops on booleans error
        // 4. String vs numeric relational is error (still simplified to FORG0006 here)
        use XdmAtomicValue as V;

        // Normalize untypedAtomic per context: if the counterpart is numeric attempt numeric cast (error on failure),
        // else treat both sides' untyped as string. untyped vs untyped -> both strings.
        let (a_norm, b_norm) = match (a, b) {
            (V::UntypedAtomic(sa), V::UntypedAtomic(sb)) => (V::String(sa.clone()), V::String(sb.clone())),
            (V::UntypedAtomic(s), other)
                if matches!(other, V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)) =>
            {
                let num =
                    s.parse::<f64>().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid numeric literal"))?;
                (V::Double(num), other.clone())
            }
            (other, V::UntypedAtomic(s))
                if matches!(other, V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)) =>
            {
                let num =
                    s.parse::<f64>().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid numeric literal"))?;
                (other.clone(), V::Double(num))
            }
            (V::UntypedAtomic(s), other) => (V::String(s.clone()), other.clone()),
            (other, V::UntypedAtomic(s)) => (other.clone(), V::String(s.clone())),
            _ => (a.clone(), b.clone()),
        };

        // Boolean handling
        if let (V::Boolean(x), V::Boolean(y)) = (&a_norm, &b_norm) {
            return Ok(match op {
                Eq => x == y,
                Ne => x != y,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "relational op on boolean"));
                }
            });
        }

        // If both (after normalization) are strings and not numeric context
        if matches!((&a_norm, &b_norm), (V::String(_), V::String(_))) && matches!(op, Lt | Le | Gt | Ge | Eq | Ne) {
            let ls = if let V::String(s) = &a_norm { s } else { unreachable!("expected string after normalization") };
            let rs = if let V::String(s) = &b_norm { s } else { unreachable!("expected string after normalization") };
            // Collation-aware: use default collation (fallback to codepoint)
            let coll_arc;
            let coll: &dyn crate::engine::collation::Collation = if let Some(c) = &self.default_collation {
                c.as_ref()
            } else {
                coll_arc = self
                    .dyn_ctx
                    .collations
                    .get(crate::engine::collation::CODEPOINT_URI)
                    .unwrap_or_else(|| std::rc::Rc::new(crate::engine::collation::CodepointCollation));
                coll_arc.as_ref()
            };
            return Ok(match op {
                Eq => coll.key(ls) == coll.key(rs),
                Ne => coll.key(ls) != coll.key(rs),
                Lt => coll.compare(ls, rs).is_lt(),
                Le => {
                    let ord = coll.compare(ls, rs);
                    ord.is_lt() || ord.is_eq()
                }
                Gt => coll.compare(ls, rs).is_gt(),
                Ge => {
                    let ord = coll.compare(ls, rs);
                    ord.is_gt() || ord.is_eq()
                }
            });
        }

        // QName equality (only Eq/Ne permitted); compare namespace URI + local name; ignore prefix
        if let (
            XdmAtomicValue::QName { ns_uri: nsa, local: la, .. },
            XdmAtomicValue::QName { ns_uri: nsb, local: lb, .. },
        ) = (a, b)
        {
            return Ok(match op {
                Eq => nsa == nsb && la == lb,
                Ne => nsa != nsb || la != lb,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "relational op on QName"));
                }
            });
        }

        // NOTATION equality (only Eq/Ne permitted); current engine treats NOTATION as lexical string
        if let (XdmAtomicValue::Notation(na), XdmAtomicValue::Notation(nb)) = (a, b) {
            return Ok(match op {
                Eq => na == nb,
                Ne => na != nb,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "relational op on NOTATION"));
                }
            });
        }

        // Numeric path with minimal promotion
        if let (Some(ca), Some(cb)) = (classify(&a_norm), classify(&b_norm)) {
            let (ua, ub) = unify_numeric(ca, cb);
            let (ln, rn) = (ua.to_f64(), ub.to_f64());
            if ln.is_nan() || rn.is_nan() {
                return Ok(matches!(op, ComparisonOp::Ne));
            }
            return Ok(match op {
                Eq => ln == rn,
                Ne => ln != rn,
                Lt => ln < rn,
                Le => ln <= rn,
                Gt => ln > rn,
                Ge => ln >= rn,
            });
        }

        // dateTime relational comparisons by absolute instant
        if let (XdmAtomicValue::DateTime(da), XdmAtomicValue::DateTime(db)) = (a, b) {
            let (a_ts, b_ts) = (da.timestamp(), db.timestamp());
            let (a_ns, b_ns) = (da.timestamp_subsec_nanos(), db.timestamp_subsec_nanos());
            let ord = (a_ts, a_ns).cmp(&(b_ts, b_ns));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // duration comparisons (same family only)
        if let (XdmAtomicValue::YearMonthDuration(ma), XdmAtomicValue::YearMonthDuration(mb)) = (a, b) {
            let ord = ma.cmp(mb);
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }
        if let (XdmAtomicValue::DayTimeDuration(sa), XdmAtomicValue::DayTimeDuration(sb)) = (a, b) {
            let ord = sa.cmp(sb);
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // date comparisons: normalize to midnight in effective timezone
        if let (XdmAtomicValue::Date { date: da, tz: ta }, XdmAtomicValue::Date { date: db, tz: tb }) = (a, b) {
            let eff_tz_a = (*ta).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tb).unwrap_or_else(|| self.implicit_timezone());
            let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0)
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "invalid time 00:00:00"))?;
            let na = da.and_time(midnight);
            let nb = db.and_time(midnight);
            let dta = eff_tz_a
                .from_local_datetime(&na)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let dtb = eff_tz_b
                .from_local_datetime(&nb)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let ord =
                (dta.timestamp(), dta.timestamp_subsec_nanos()).cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // time comparisons: anchor to a fixed date and compare instants in effective timezone
        if let (XdmAtomicValue::Time { time: ta, tz: tza }, XdmAtomicValue::Time { time: tb, tz: tzb }) = (a, b) {
            let eff_tz_a = (*tza).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tzb).unwrap_or_else(|| self.implicit_timezone());
            let base = chrono::NaiveDate::from_ymd_opt(2000, 1, 1)
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "invalid base date 2000-01-01"))?;
            let na = base.and_time(*ta);
            let nb = base.and_time(*tb);
            let dta = eff_tz_a
                .from_local_datetime(&na)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let dtb = eff_tz_b
                .from_local_datetime(&nb)
                .single()
                .ok_or_else(|| Error::from_code(ErrorCode::FOAR0001, "ambiguous local datetime"))?;
            let ord =
                (dta.timestamp(), dta.timestamp_subsec_nanos()).cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // Unsupported / incomparable type combination → type error (XPTY0004)
        Err(Error::from_code(ErrorCode::XPTY0004, "incomparable atomic types"))
    }
}
