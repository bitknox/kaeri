//! The Cast receiver page on the website: the messages of `receiver-*.ftl` as one JSON file per
//! language in `website/public/cast/receiver/i18n/`, and `index.json` with the languages and
//! each message's argument types. The page fetches the file of the language the sender names.
//!
//! A message is a list of parts, each a string or `{ "arg": name }`, so no text needs escaping;
//! a plural is `{ "plural": arg, "forms": { category: parts } }`, picked with `Intl.PluralRules`.

use anyhow::Result;
use serde_json::{Map, Value, json};

use crate::catalogue::{ArgType, Body, Catalogue, Part, Platform};
use crate::ship::{Mode, select};
use crate::util::Files;

pub const DIR: &str = "website/public/cast/receiver/i18n";

pub fn generate(cat: &Catalogue, mode: Mode) -> Result<Files> {
    let langs = select(cat, mode, |only| only == Some(Platform::Receiver))?;
    let mut files = Files::new();
    for lang in &langs {
        let mut messages = Map::new();
        for e in &lang.entries {
            messages.insert(e.source.id.clone(), body(&e.body));
        }
        files.insert(
            format!("{DIR}/{}.json", lang.tag),
            pretty(&json!({ "tag": lang.tag, "messages": messages })),
        );
    }
    let mut args = Map::new();
    if let Some(source) = langs.first() {
        for e in &source.entries {
            if e.source.args.is_empty() {
                continue;
            }
            let types: Map<String, Value> = e
                .source
                .args
                .iter()
                .map(|a| (a.name.clone(), json!(type_name(a.ty))))
                .collect();
            args.insert(e.source.id.clone(), Value::Object(types));
        }
    }
    let tags: Vec<&str> = langs.iter().map(|l| l.tag.as_str()).collect();
    files.insert(
        format!("{DIR}/index.json"),
        pretty(&json!({ "source": cat.config.source, "languages": tags, "args": args })),
    );
    Ok(files)
}

fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("JSON values always serialise");
    text.push('\n');
    text
}

fn parts(parts: &[Part]) -> Value {
    let mut out: Vec<Value> = Vec::new();
    for part in parts {
        match part {
            Part::Text(t) => match out.last_mut() {
                Some(Value::String(prev)) => prev.push_str(t),
                _ => out.push(json!(t)),
            },
            Part::Var(v) => out.push(json!({ "arg": v })),
            Part::Term { .. } => unreachable!("terms are inlined before generation"),
        }
    }
    Value::Array(out)
}

fn body(body: &Body) -> Value {
    match body {
        Body::Plain(p) => parts(p),
        Body::Plural { selector, variants } => {
            let forms: Map<String, Value> =
                variants.iter().map(|v| (v.category.clone(), parts(&v.parts))).collect();
            json!({ "plural": selector, "forms": forms })
        }
    }
}

fn type_name(ty: ArgType) -> &'static str {
    match ty {
        ArgType::Text | ArgType::Cause => "text",
        ArgType::Int => "int",
        ArgType::Plain => "plain",
        ArgType::Decimal => "decimal",
        ArgType::Percent => "percent",
        ArgType::Seconds => "seconds",
        ArgType::Bytes => "bytes",
        ArgType::Timestamp => "timestamp",
    }
}
