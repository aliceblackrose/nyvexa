use std::env;

use ed25519_dalek::{Signature, VerifyingKey};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use vercel_runtime::{Error, Request, Response, ResponseBody, run, service_fn};

const SIGNATURE_HEADER: &str = "x-signature-ed25519";
const TIMESTAMP_HEADER: &str = "x-signature-timestamp";

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(request: Request) -> Result<Response<ResponseBody>, Error> {
    if request.method().as_str() != "POST" {
        return response(405, json!({ "error": "method not allowed" }));
    }

    let signature = match header(&request, SIGNATURE_HEADER) {
        Some(value) => value.to_owned(),
        None => return response(401, json!({ "error": "missing signature" })),
    };
    let timestamp = match header(&request, TIMESTAMP_HEADER) {
        Some(value) => value.to_owned(),
        None => return response(401, json!({ "error": "missing timestamp" })),
    };

    let public_key = env::var("DISCORD_PUBLIC_KEY")?;
    let (_, body) = request.into_parts();
    let body = body.collect().await?.to_bytes();

    if !verify(&public_key, &signature, &timestamp, &body) {
        return response(401, json!({ "error": "invalid request signature" }));
    }

    let payload: Value = serde_json::from_slice(&body)?;
    if payload.get("type").and_then(Value::as_u64) == Some(1) {
        return response(200, json!({ "type": 1 }));
    }

    let interaction = serde_json::from_value(payload)?;
    match nyvexa::commands::response(&interaction)? {
        Some(command_response) => response(200, serde_json::to_value(command_response)?),
        None => response(400, json!({ "error": "unsupported interaction" })),
    }
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.headers().get(name)?.to_str().ok()
}

fn verify(public_key: &str, signature: &str, timestamp: &str, body: &[u8]) -> bool {
    let Ok(public_key) = hex::decode(public_key) else {
        return false;
    };
    let Ok(public_key): Result<[u8; 32], _> = public_key.try_into() else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&public_key) else {
        return false;
    };

    let Ok(signature) = hex::decode(signature) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature) else {
        return false;
    };

    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);

    verifying_key.verify_strict(&message, &signature).is_ok()
}

fn response(status: u16, body: Value) -> Result<Response<ResponseBody>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.into())?)
}
