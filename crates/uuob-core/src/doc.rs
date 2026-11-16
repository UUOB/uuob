//! Helpers for treating Automerge documents as JSON.
//!
//! Every container is one Automerge document. Records live at flat root keys
//! `<collection>/<record id>` as maps whose fields merge individually when two
//! devices edit the same record concurrently.

use automerge::{AutoCommit, ObjId, ObjType, ReadDoc, ScalarValue, Value, transaction::Transactable};
use serde_json::{Map, Number, Value as Json};

use crate::error::Result;
use crate::store::Meta;

/// What the store remembers about a document so it need not be loaded: a
/// fingerprint of its current state (two devices with the same fingerprint
/// hold the same document) and its record counts per collection.
pub fn meta(doc: &mut AutoCommit) -> Meta {
    use sha2::{Digest, Sha256};
    let mut heads = doc.get_heads();
    heads.sort();
    let mut hash = Sha256::new();
    for h in &heads {
        hash.update(h.0);
    }
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let keys: Vec<String> = doc.keys(automerge::ROOT).collect();
    for k in &keys {
        if let Some((collection, _)) = k.split_once('/') {
            *counts.entry(collection).or_default() += 1;
        }
    }
    Meta { heads: hash.finalize().to_vec(), stats: serde_json::to_string(&counts).unwrap_or_default() }
}

/// Convert the object `obj` of `doc` into JSON.
pub fn to_json(doc: &AutoCommit, obj: &ObjId) -> Json {
    match doc.object_type(obj) {
        Ok(ObjType::Map) | Ok(ObjType::Table) => {
            let mut map = Map::new();
            for key in doc.keys(obj) {
                if let Ok(Some((value, id))) = doc.get(obj, key.as_str()) {
                    map.insert(key, value_to_json(doc, value, &id));
                }
            }
            Json::Object(map)
        }
        Ok(ObjType::List) => {
            let len = doc.length(obj);
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                if let Ok(Some((value, id))) = doc.get(obj, i) {
                    out.push(value_to_json(doc, value, &id));
                }
            }
            Json::Array(out)
        }
        Ok(ObjType::Text) => Json::String(doc.text(obj).unwrap_or_default()),
        Err(_) => Json::Null,
    }
}

fn value_to_json(doc: &AutoCommit, value: Value<'_>, id: &ObjId) -> Json {
    match value {
        Value::Object(_) => to_json(doc, id),
        Value::Scalar(s) => scalar_to_json(&s),
    }
}

fn scalar_to_json(s: &ScalarValue) -> Json {
    match s {
        ScalarValue::Str(s) => Json::String(s.to_string()),
        ScalarValue::Int(i) => Json::Number((*i).into()),
        ScalarValue::Uint(u) => Json::Number((*u).into()),
        ScalarValue::F64(f) => Number::from_f64(*f).map(Json::Number).unwrap_or(Json::Null),
        ScalarValue::Counter(c) => Json::Number(i64::from(c).into()),
        ScalarValue::Timestamp(t) => Json::Number((*t).into()),
        ScalarValue::Boolean(b) => Json::Bool(*b),
        ScalarValue::Null => Json::Null,
        ScalarValue::Bytes(b) => Json::String(data_encoding::BASE64.encode(b)),
        _ => Json::Null,
    }
}

fn json_to_scalar(v: &Json) -> Option<ScalarValue> {
    Some(match v {
        Json::Null => ScalarValue::Null,
        Json::Bool(b) => ScalarValue::Boolean(*b),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                ScalarValue::Int(i)
            } else if let Some(u) = n.as_u64() {
                ScalarValue::Uint(u)
            } else {
                ScalarValue::F64(n.as_f64().unwrap_or(0.0))
            }
        }
        Json::String(s) => ScalarValue::Str(s.as_str().into()),
        Json::Array(_) | Json::Object(_) => return None,
    })
}

/// Write `value` at `obj[key]`, creating nested objects as needed.
pub fn put_json(doc: &mut AutoCommit, obj: &ObjId, key: &str, value: &Json) -> Result<()> {
    match value {
        Json::Object(map) => {
            let child = doc.put_object(obj, key, ObjType::Map)?;
            for (k, v) in map {
                put_json(doc, &child, k, v)?;
            }
        }
        Json::Array(items) => {
            let child = doc.put_object(obj, key, ObjType::List)?;
            fill_list(doc, &child, items)?;
        }
        scalar => {
            doc.put(obj, key, json_to_scalar(scalar).expect("scalar"))?;
        }
    }
    Ok(())
}

fn fill_list(doc: &mut AutoCommit, list: &ObjId, items: &[Json]) -> Result<()> {
    for (i, v) in items.iter().enumerate() {
        match v {
            Json::Object(map) => {
                let child = doc.insert_object(list, i, ObjType::Map)?;
                for (k, v) in map {
                    put_json(doc, &child, k, v)?;
                }
            }
            Json::Array(inner) => {
                let child = doc.insert_object(list, i, ObjType::List)?;
                fill_list(doc, &child, inner)?;
            }
            scalar => {
                doc.insert(list, i, json_to_scalar(scalar).expect("scalar"))?;
            }
        }
    }
    Ok(())
}

/// Return the child map at `obj[key]`, if any.
pub fn get_map(doc: &AutoCommit, obj: &ObjId, key: &str) -> Option<ObjId> {
    match doc.get(obj, key) {
        Ok(Some((Value::Object(ObjType::Map), id))) => Some(id),
        _ => None,
    }
}

/// Return the child map at `obj[key]`, creating it when missing.
pub fn ensure_map(doc: &mut AutoCommit, obj: &ObjId, key: &str) -> Result<ObjId> {
    if let Some(id) = get_map(doc, obj, key) {
        return Ok(id);
    }
    Ok(doc.put_object(obj, key, ObjType::Map)?)
}

/// Replace the record stored at `parent[id]` with the JSON object `value`.
///
/// Only fields that actually changed are written, so concurrent edits of
/// different fields on different devices both survive the merge.
pub fn set_record(doc: &mut AutoCommit, parent: &ObjId, id: &str, value: &Map<String, Json>) -> Result<()> {
    let rec = ensure_map(doc, parent, id)?;
    let existing: Vec<String> = doc.keys(&rec).collect();
    for key in existing {
        if !value.contains_key(&key) {
            doc.delete(&rec, key.as_str())?;
        }
    }
    for (k, v) in value {
        let current = match doc.get(&rec, k.as_str()) {
            Ok(Some((val, oid))) => Some(value_to_json(doc, val, &oid)),
            _ => None,
        };
        if current.as_ref() != Some(v) {
            put_json(doc, &rec, k, v)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::ROOT;
    use serde_json::json;

    #[test]
    fn roundtrip_and_field_merge() {
        let mut a = AutoCommit::new();
        let coll = ROOT;
        let v = json!({"title": "Buy milk", "done": false, "tags": ["home", 2], "meta": {"p": 1.5}});
        set_record(&mut a, &coll, "t1", v.as_object().unwrap()).unwrap();
        let rec = get_map(&a, &coll, "t1").unwrap();
        assert_eq!(to_json(&a, &rec), v);

        // Fork, edit different fields concurrently, merge.
        let mut b = a.fork();
        let coll_b = ROOT;
        let mut va = v.clone();
        va["done"] = json!(true);
        let mut vb = v.clone();
        vb["title"] = json!("Buy oat milk");
        set_record(&mut a, &coll, "t1", va.as_object().unwrap()).unwrap();
        set_record(&mut b, &coll_b, "t1", vb.as_object().unwrap()).unwrap();
        a.merge(&mut b).unwrap();
        let rec = get_map(&a, &coll, "t1").unwrap();
        let merged = to_json(&a, &rec);
        assert_eq!(merged["done"], json!(true));
        assert_eq!(merged["title"], json!("Buy oat milk"));
    }
}
