use regex::Regex;

#[derive(Debug, Clone)]
pub struct ParsedAuthAction {
    pub action_url: Option<String>,
    pub action_code: Option<String>,
    pub action_hint: String,
    pub allowed_loopback_port: Option<u16>,
}

pub trait AgentAuthOutputParser {
    fn parse_line(&self, line: &str) -> Option<ParsedAuthAction>;
}

#[derive(Debug)]
struct CodexAuthParser;

#[derive(Debug)]
struct ClaudeAuthParser;

#[derive(Debug)]
struct GeminiAuthParser;

#[derive(Debug)]
struct CursorAuthParser;

#[derive(Debug)]
struct GenericAuthParser;

impl AgentAuthOutputParser for CodexAuthParser {
    fn parse_line(&self, line: &str) -> Option<ParsedAuthAction> {
        parse_codex_auth_line(line)
    }
}

impl AgentAuthOutputParser for ClaudeAuthParser {
    fn parse_line(&self, line: &str) -> Option<ParsedAuthAction> {
        parse_claude_auth_line(line)
    }
}

impl AgentAuthOutputParser for GeminiAuthParser {
    fn parse_line(&self, line: &str) -> Option<ParsedAuthAction> {
        parse_gemini_auth_line(line)
    }
}

impl AgentAuthOutputParser for CursorAuthParser {
    fn parse_line(&self, line: &str) -> Option<ParsedAuthAction> {
        parse_cursor_auth_line(line)
    }
}

impl AgentAuthOutputParser for GenericAuthParser {
    fn parse_line(&self, line: &str) -> Option<ParsedAuthAction> {
        parse_generic_auth_line(line)
    }
}

static CODEX_AUTH_PARSER: CodexAuthParser = CodexAuthParser;
static CLAUDE_AUTH_PARSER: ClaudeAuthParser = ClaudeAuthParser;
static GEMINI_AUTH_PARSER: GeminiAuthParser = GeminiAuthParser;
static CURSOR_AUTH_PARSER: CursorAuthParser = CursorAuthParser;
static GENERIC_AUTH_PARSER: GenericAuthParser = GenericAuthParser;

pub fn parser_for_provider(provider: &str) -> &'static dyn AgentAuthOutputParser {
    match provider {
        "openai-codex" => &CODEX_AUTH_PARSER,
        "claude-code" => &CLAUDE_AUTH_PARSER,
        "gemini-cli" => &GEMINI_AUTH_PARSER,
        "cursor-cli" => &CURSOR_AUTH_PARSER,
        _ => &GENERIC_AUTH_PARSER,
    }
}

pub fn parse_auth_required_action(provider: &str, line: &str) -> Option<ParsedAuthAction> {
    parser_for_provider(provider).parse_line(line)
}

pub fn parse_loopback_port(url: &str) -> Option<u16> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "127.0.0.1" && host != "localhost" {
        return None;
    }
    parsed.port_or_known_default()
}

fn parse_codex_auth_line(line: &str) -> Option<ParsedAuthAction> {
    let action_url = extract_auth_url(line);
    let action_code = extract_device_code(line);
    if action_url.is_none() && action_code.is_none() {
        return None;
    }

    Some(ParsedAuthAction {
        action_url,
        action_code,
        action_hint:
            "Open the auth URL in browser and enter the one-time device code if requested."
                .to_string(),
        allowed_loopback_port: None,
    })
}

fn parse_claude_auth_line(line: &str) -> Option<ParsedAuthAction> {
    let action_url = extract_auth_url(line)?;
    if !is_supported_claude_auth_url(&action_url) {
        return None;
    }
    let allowed_loopback_port = parse_loopback_port(&action_url);
    let action_hint = if allowed_loopback_port.is_some() {
        "Complete auth in browser. If redirected to localhost and it fails, paste that localhost URL below."
            .to_string()
    } else {
        "Open the Claude authorization URL in browser to continue.".to_string()
    };

    Some(ParsedAuthAction {
        action_url: Some(action_url),
        action_code: None,
        action_hint,
        allowed_loopback_port,
    })
}

fn is_supported_claude_auth_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };

    let host = parsed
        .host_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if host == "127.0.0.1" || host == "localhost" {
        return true;
    }

    if host == "claude.ai" || host.ends_with(".claude.ai") {
        let path = parsed.path().to_ascii_lowercase();
        return path.contains("/oauth");
    }

    false
}

/// Google OAuth base URL without query params causes 400 "Required parameter is missing: response_type".
/// Don't expose it as a clickable link; only full URLs (with ?...) are valid.
fn is_bare_google_oauth_url(url: &str) -> bool {
    let u = url.trim_end_matches('/');
    u == "https://accounts.google.com/o/oauth2/v2/auth"
        || u == "https://accounts.google.com/o/oauth2/auth"
}

/// Gemini CLI may print docs/tos links (e.g. geminicli.com/docs/resources/tos-privacy/). These are not auth URLs.
fn is_gemini_docs_or_non_auth_url(url: &str) -> bool {
    url.to_lowercase().contains("geminicli.com")
}

fn parse_gemini_auth_line(line: &str) -> Option<ParsedAuthAction> {
    let mut action_url = extract_auth_url(line);
    if action_url
        .as_deref()
        .map_or(false, is_bare_google_oauth_url)
    {
        action_url = None;
    }
    if action_url
        .as_deref()
        .map_or(false, is_gemini_docs_or_non_auth_url)
    {
        action_url = None;
    }
    let action_code = extract_oob_code(line).or_else(|| extract_device_code(line));
    if action_url.is_none() && action_code.is_none() {
        return None;
    }

    let allowed_loopback_port = action_url.as_deref().and_then(parse_loopback_port);
    let action_hint = if allowed_loopback_port.is_some() {
        "Complete Google auth in browser. If redirected to localhost and it fails, paste that localhost URL below."
            .to_string()
    } else if action_code.is_some() {
        "Use the one-time code below in the Google sign-in page, or open the URL in browser if the CLI printed a full link.".to_string()
    } else {
        "Open the Google auth URL and submit the code/callback shown by the provider.".to_string()
    };

    Some(ParsedAuthAction {
        action_url,
        action_code,
        action_hint,
        allowed_loopback_port,
    })
}

fn parse_cursor_auth_line(line: &str) -> Option<ParsedAuthAction> {
    let action_url = extract_auth_url(line)?;
    if !action_url.contains("cursor.com") {
        return None;
    }

    Some(ParsedAuthAction {
        action_url: Some(action_url),
        action_code: None,
        action_hint:
            "Open this URL in browser to complete Cursor login. No need to paste callback."
                .to_string(),
        allowed_loopback_port: None,
    })
}

fn parse_generic_auth_line(line: &str) -> Option<ParsedAuthAction> {
    let action_url = extract_auth_url(line);
    let action_code = extract_device_code(line);
    if action_url.is_none() && action_code.is_none() {
        return None;
    }

    let allowed_loopback_port = action_url.as_deref().and_then(parse_loopback_port);
    Some(ParsedAuthAction {
        action_url,
        action_code,
        action_hint: "Continue authentication by following provider instructions.".to_string(),
        allowed_loopback_port,
    })
}

fn extract_auth_url(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|token| {
        let normalized = token.trim_matches(|c: char| {
            c == '"'
                || c == '\''
                || c == ')'
                || c == '('
                || c == ','
                || c == ';'
                || c == '.'
                || c == '['
                || c == ']'
        });
        if normalized.starts_with("http://") || normalized.starts_with("https://") {
            Some(normalized.to_string())
        } else {
            None
        }
    })
}

fn extract_device_code(line: &str) -> Option<String> {
    let code_re = Regex::new(r"\b[A-Z0-9]{3,10}-[A-Z0-9]{3,10}\b").ok()?;
    code_re.find(line).map(|m| m.as_str().to_string())
}

fn extract_oob_code(line: &str) -> Option<String> {
    let code_re = Regex::new(r"\b4/[A-Za-z0-9\-_]+\b").ok()?;
    code_re.find(line).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_auth_required_action, parse_loopback_port, parser_for_provider};

    #[test]
    fn codex_parser_extracts_url_and_code() {
        let line = "copy code ABCD-1234 and open https://github.com/login/device";
        let parsed =
            parse_auth_required_action("openai-codex", line).expect("expected parsed action");
        assert_eq!(parsed.action_code.as_deref(), Some("ABCD-1234"));
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("https://github.com/login/device")
        );
    }

    #[test]
    fn gemini_parser_extracts_oob_code() {
        let line = "Authorization code: 4/0AbCdEf123";
        let parsed =
            parse_auth_required_action("gemini-cli", line).expect("expected parsed action");
        assert_eq!(parsed.action_code.as_deref(), Some("4/0AbCdEf123"));
    }

    #[test]
    fn codex_parser_handles_changed_wording() {
        let line = "visit https://github.com/login/device and enter one-time code ZX9K-22QW";
        let parsed =
            parse_auth_required_action("openai-codex", line).expect("expected parsed action");
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("https://github.com/login/device")
        );
        assert_eq!(parsed.action_code.as_deref(), Some("ZX9K-22QW"));
    }

    #[test]
    fn codex_parser_handles_current_device_code_shape() {
        let line = "Enter this one-time code (expires in 15 minutes) 4R0D-57CSL";
        let parsed =
            parse_auth_required_action("openai-codex", line).expect("expected parsed action");
        assert_eq!(parsed.action_code.as_deref(), Some("4R0D-57CSL"));
    }

    #[test]
    fn claude_parser_extracts_loopback_port() {
        let line = "Redirect URL: http://127.0.0.1:53124/callback?code=abc";
        let parsed =
            parse_auth_required_action("claude-code", line).expect("expected parsed action");
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("http://127.0.0.1:53124/callback?code=abc")
        );
        assert_eq!(parsed.allowed_loopback_port, Some(53124));
    }

    #[test]
    fn claude_parser_ignores_non_claude_url_noise() {
        let line =
            "For terminal raw mode check: https://github.com/vadimdemedes/ink/#israwmodesupported";
        assert!(parse_auth_required_action("claude-code", line).is_none());
    }

    #[test]
    fn claude_parser_accepts_claude_oauth_url() {
        let line = "Open browser: https://claude.ai/oauth/authorize?client_id=abc";
        let parsed =
            parse_auth_required_action("claude-code", line).expect("expected parsed action");
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("https://claude.ai/oauth/authorize?client_id=abc")
        );
        assert_eq!(parsed.allowed_loopback_port, None);
    }

    #[test]
    fn gemini_parser_skips_bare_google_oauth_url() {
        // Bare URL without query params causes 400 from Google; we do not expose it as action_url.
        let line = "Open this URL in your browser: https://accounts.google.com/o/oauth2/v2/auth";
        assert!(parse_auth_required_action("gemini-cli", line).is_none());
    }

    #[test]
    fn gemini_parser_keeps_google_oauth_url_with_params() {
        let line = "Open https://accounts.google.com/o/oauth2/v2/auth?client_id=xxx&response_type=code&redirect_uri=...";
        let parsed =
            parse_auth_required_action("gemini-cli", line).expect("expected parsed action");
        assert!(parsed
            .action_url
            .as_deref()
            .unwrap()
            .contains("response_type=code"));
    }

    #[test]
    fn gemini_parser_handles_no_browser_authorize_url_output() {
        let line = "https://accounts.google.com/o/oauth2/v2/auth?redirect_uri=https%3A%2F%2Fcodeassist.google.com%2Fauthcode&response_type=code&client_id=sample.apps.googleusercontent.com";
        let parsed =
            parse_auth_required_action("gemini-cli", line).expect("expected parsed action");
        let url = parsed.action_url.expect("expected action url");
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("response_type=code"));
    }

    #[test]
    fn gemini_parser_skips_geminicli_docs_tos_url() {
        // Line with only docs/tos URL yields no auth action (no valid URL, no code).
        let line = "See terms: https://geminicli.com/docs/resources/tos-privacy/";
        assert!(parse_auth_required_action("gemini-cli", line).is_none());
    }

    #[test]
    fn loopback_port_accepts_localhost_only() {
        assert_eq!(
            parse_loopback_port("http://127.0.0.1:4000/callback?code=1"),
            Some(4000)
        );
        assert_eq!(parse_loopback_port("http://localhost:3456/"), Some(3456));
        assert_eq!(parse_loopback_port("http://example.com:3456/"), None);
    }

    #[test]
    fn adapter_registry_routes_unknown_provider_to_generic_parser() {
        let parser = parser_for_provider("unknown-provider");
        let parsed = parser
            .parse_line("open https://example.com/auth and enter ABCD-1234")
            .expect("expected generic parser output");
        assert_eq!(parsed.action_code.as_deref(), Some("ABCD-1234"));
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("https://example.com/auth")
        );
        assert_eq!(
            parsed.action_hint,
            "Continue authentication by following provider instructions."
        );
    }

    #[test]
    fn cursor_parser_extracts_cursor_login_url() {
        let line = "Open this URL: https://cursor.com/loginDeepControl?challenge=abc&uuid=xyz&mode=login&redirectTarget=cli";
        let parsed =
            parse_auth_required_action("cursor-cli", line).expect("expected parsed action");
        assert_eq!(
            parsed.action_url.as_deref(),
            Some("https://cursor.com/loginDeepControl?challenge=abc&uuid=xyz&mode=login&redirectTarget=cli")
        );
        assert!(parsed.action_code.is_none());
        assert!(parsed.action_hint.to_lowercase().contains("open this url"));
    }

    #[test]
    fn cursor_parser_ignores_non_cursor_urls() {
        let line = "Open https://github.com/login/device";
        assert!(parse_auth_required_action("cursor-cli", line).is_none());
    }

    #[test]
    fn adapter_registry_routes_claude_to_loopback_hint() {
        let parser = parser_for_provider("claude-code");
        let parsed = parser
            .parse_line("Open browser: http://localhost:51111/callback?code=abc")
            .expect("expected claude parser output");
        assert_eq!(parsed.allowed_loopback_port, Some(51111));
        assert!(parsed.action_code.is_none());
        assert!(parsed.action_hint.to_lowercase().contains("localhost"));
    }
}
