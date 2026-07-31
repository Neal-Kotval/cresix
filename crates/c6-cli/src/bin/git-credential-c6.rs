use std::{
    collections::BTreeMap,
    io::{self, Read},
};

use c6_cli::{
    config::{Config, Paths},
    credential::CredentialStore,
};
use c6_client::{Origin, valid_git_path};
use url::Url;

fn main() {
    if let Err(error) = execute() {
        eprintln!("git-credential-c6: {error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), String> {
    let operation = std::env::args()
        .nth(1)
        .ok_or("expected get, store, or erase")?;
    if !matches!(operation.as_str(), "get" | "store" | "erase") {
        return Err("expected get, store, or erase".into());
    }
    let mut input = String::new();
    io::stdin()
        .take(64 * 1024 + 1)
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    if input.len() > 64 * 1024 {
        return Err("credential input too large".into());
    }
    let fields = parse_fields(&input)?;
    let Some((origin, path)) = request_target(&fields)? else {
        return Ok(());
    };
    if !valid_git_path(&path) {
        return Ok(());
    }
    let paths = Paths::discover().map_err(|e| e.to_string())?;
    let _lock = paths.lock().map_err(|e| e.to_string())?;
    let config = Config::load(&paths).map_err(|e| e.to_string())?;
    let matched = config.servers.iter().find(|(_, server)| {
        Origin::parse(&server.base_url, server.allow_http_localhost)
            .ok()
            .as_ref()
            .is_some_and(|configured| configured == &origin)
    });
    let Some((alias, _)) = matched else {
        return Ok(());
    };
    let mut store = CredentialStore::load(&paths).map_err(|e| e.to_string())?;
    match operation.as_str() {
        "get" => {
            if let Some(token) = store.git_token(alias, &path) {
                println!("username=c6\npassword={}", token.expose());
            }
        }
        "store" => {
            if !config.plaintext_credentials {
                return Err("plaintext credential storage is not enabled; run `c6 auth login --plaintext-store --token-stdin` first".into());
            }
            if fields.get("username").is_some_and(|v| v != "c6") {
                return Ok(());
            }
            let Some(token) = fields.get("password") else {
                return Ok(());
            };
            if !token.starts_with("c6g_v1_") || token.chars().any(char::is_whitespace) {
                return Ok(());
            }
            store.set_git_token(alias.clone(), path, token.clone());
            store.save(&paths).map_err(|e| e.to_string())?;
        }
        "erase" => {
            if store.remove_git_token(alias, &path) {
                store.save(&paths).map_err(|e| e.to_string())?;
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn parse_fields(input: &str) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    for line in input.lines() {
        if line.is_empty() {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err("malformed credential input".into());
        };
        if !valid_field_name(key) {
            return Err("malformed credential key".into());
        }
        if value.chars().any(char::is_control) {
            return Err("malformed credential value".into());
        }
        // Git may add extension fields over time (for example repeated
        // `wwwauth[]` challenges). They are deliberately ignored: only these
        // single-valued fields can influence C6 target or token selection.
        if matches!(
            key,
            "url" | "protocol" | "host" | "path" | "username" | "password"
        ) && values.insert(key.into(), value.into()).is_some()
        {
            return Err("duplicate credential field".into());
        }
    }
    Ok(values)
}

fn valid_field_name(key: &str) -> bool {
    let base = key.strip_suffix("[]").unwrap_or(key);
    !base.is_empty()
        && !base.contains(['[', ']'])
        && base
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn request_target(fields: &BTreeMap<String, String>) -> Result<Option<(Origin, String)>, String> {
    let url = if let Some(value) = fields.get("url") {
        if fields.contains_key("protocol")
            || fields.contains_key("host")
            || fields.contains_key("path")
        {
            return Err("ambiguous credential target".into());
        }
        Url::parse(value).map_err(|_| "invalid credential URL")?
    } else {
        let Some(protocol) = fields.get("protocol") else {
            return Ok(None);
        };
        let Some(host) = fields.get("host") else {
            return Ok(None);
        };
        if !matches!(protocol.as_str(), "https" | "http") {
            return Ok(None);
        }
        Url::parse(&format!(
            "{protocol}://{host}/{}",
            fields.get("path").map(String::as_str).unwrap_or("")
        ))
        .map_err(|_| "invalid credential target")?
    };
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Ok(None);
    }
    let path = format!("/{}", url.path().trim_start_matches('/'));
    let origin_text = format!(
        "{}://{}{}",
        url.scheme(),
        url.host_str().ok_or("missing host")?,
        url.port().map(|p| format!(":{p}")).unwrap_or_default()
    );
    let origin = Origin::parse(&origin_text, url.scheme() == "http")
        .map_err(|_| "invalid credential origin")?;
    Ok(Some((origin, path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_git_target_without_credentials() {
        let fields =
            parse_fields("protocol=https\nhost=c6.example\npath=git/paper-street/weeknote.git\n\n")
                .unwrap();
        let (origin, path) = request_target(&fields).unwrap().unwrap();
        assert_eq!(origin.as_str(), "https://c6.example");
        assert_eq!(path, "/git/paper-street/weeknote.git");
        assert!(valid_git_path(&path));
    }

    #[test]
    fn rejects_userinfo_and_non_http_protocols() {
        let with_secret =
            BTreeMap::from([("url".into(), "https://token@c6.example/git/a/b.git".into())]);
        assert!(request_target(&with_secret).unwrap().is_none());
        let ssh = BTreeMap::from([
            ("protocol".into(), "ssh".into()),
            ("host".into(), "c6.example".into()),
            ("path".into(), "git/a/b.git".into()),
        ]);
        assert!(request_target(&ssh).unwrap().is_none());
    }

    #[test]
    fn malformed_lines_fail_closed() {
        assert!(parse_fields("password-without-separator\n").is_err());
        assert!(parse_fields("bad[key]=value\n").is_err());
        assert!(parse_fields("host=one.example\nhost=two.example\n").is_err());
        assert!(parse_fields("host=example.com\nunknown=value\u{7}\n").is_err());
    }

    #[test]
    fn tolerates_realistic_git_extension_fields_without_target_confusion() {
        let input = concat!(
            "protocol=https\n",
            "host=c6.example\n",
            "path=git/paper-street/weeknote.git\n",
            "wwwauth[]=Basic realm=\"C6 Git\", charset=\"UTF-8\"\n",
            "wwwauth[]=Bearer realm=\"ignored.example\"\n",
            "future-extension[]=https://evil.example/git/a/b.git\n",
            "future-extension=also ignored\n\n",
        );
        let fields = parse_fields(input).unwrap();
        assert_eq!(fields.len(), 3);
        let (origin, path) = request_target(&fields).unwrap().unwrap();
        assert_eq!(origin.as_str(), "https://c6.example");
        assert_eq!(path, "/git/paper-street/weeknote.git");
    }

    #[test]
    fn rejects_ambiguous_url_and_component_targets() {
        let fields = parse_fields(
            "url=https://c6.example/git/a/b.git\nprotocol=https\nhost=evil.example\n\n",
        )
        .unwrap();
        assert_eq!(
            request_target(&fields).unwrap_err(),
            "ambiguous credential target"
        );
    }
}
