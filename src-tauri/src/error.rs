//! Erros que aparecem na tela vão como código + dados (JSON numa string), e o front traduz
//! (`src/i18n`, chaves `error.<code>`). Nada de frase em inglês montada aqui chegando ao usuário.

use serde_json::{json, Map, Value};

/// `coded("port_unavailable", json!({ "port": 8787 }))` → `{"code":"port_unavailable","port":8787}`.
pub fn coded(code: &str, data: Value) -> String {
    let mut object = match data {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    object.insert("code".into(), json!(code));
    Value::Object(object).to_string()
}

/// Código sem dados extras.
pub fn code(code: &str) -> String {
    coded(code, Value::Null)
}

/// Código com o detalhe técnico do Windows/biblioteca (mostrado entre parênteses).
pub fn with_detail(code: &str, detail: impl std::fmt::Display) -> String {
    coded(code, json!({ "detail": detail.to_string() }))
}
