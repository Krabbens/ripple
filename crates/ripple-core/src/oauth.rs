use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

const ISSUER: &str = "https://auth.openai.com";
const RESOURCE: &str = "https://api.openai.com/v1";
const DYNAMIC_CLIENT: &str = "dynamic_agent_client";
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

#[derive(Clone, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

pub struct OAuthClient {
    client: Client,
    discovery: Discovery,
}

// Intentionally no Debug: none of these objects should dump credentials to logs.
#[derive(Clone, Deserialize, Serialize)]
pub struct Credentials {
    pub(crate) account: Account,
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) id_token: String,
    pub(crate) scopes: Vec<String>,
    pub(crate) expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Account {
    pub issuer: String,
    pub subject: String,
    pub client_id: String,
    pub host_id: String,
    pub email: Option<String>,
}

impl Credentials {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn needs_refresh(&self) -> bool {
        self.expires_at <= now() + 60
    }

    pub(crate) fn bearer(&self) -> Result<&str> {
        if self.expires_at <= now() || !self.scopes.iter().any(|s| s == PLAN_SCOPE) {
            return Err(Error::Invalid(
                "Sign in with ChatGPT before using your plan.",
            ));
        }
        Ok(&self.access_token)
    }
}

pub struct AuthAttempt {
    pub authorization_url: Url,
    redirect_uri: Url,
    state: String,
    nonce: String,
    verifier: String,
    host_id: String,
    returning: Option<Account>,
    expires_at: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    token_type: String,
    expires_in: u64,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Clone, Deserialize)]
struct Identity {
    sub: String,
    iat: u64,
    nonce: Option<String>,
    email: Option<String>,
}

pub fn random_value() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::Invalid("Secure randomness is unavailable."))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn callback_uri(value: &str) -> Result<Url> {
    let uri = Url::parse(value).map_err(|_| Error::Invalid("Invalid callback URI."))?;
    if uri.scheme() != "http"
        || uri.host_str() != Some("127.0.0.1")
        || uri.path() != "/auth/callback"
        || uri.query().is_some()
        || uri.fragment().is_some()
        || !uri.username().is_empty()
        || uri.password().is_some()
    {
        return Err(Error::Invalid(
            "Use an HTTP 127.0.0.1 /auth/callback listener.",
        ));
    }
    Ok(uri)
}

impl OAuthClient {
    pub async fn discover() -> Result<Self> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()?;
        let response = client
            .get(format!("{ISSUER}/.well-known/openid-configuration"))
            .send()
            .await?;
        let discovery: Discovery = checked(response, "OAuth discovery").await?.json().await?;
        if discovery.issuer != ISSUER {
            return Err(Error::Invalid("Unexpected OpenAI issuer."));
        }
        for endpoint in [
            &discovery.authorization_endpoint,
            &discovery.token_endpoint,
            &discovery.jwks_uri,
        ] {
            let uri =
                Url::parse(endpoint).map_err(|_| Error::Invalid("Invalid OpenAI endpoint."))?;
            if uri.scheme() != "https"
                || uri.host_str() != Some("auth.openai.com")
                || uri.port().is_some()
                || !uri.username().is_empty()
                || uri.password().is_some()
            {
                return Err(Error::Invalid("Unexpected OpenAI endpoint."));
            }
        }
        Ok(Self { client, discovery })
    }

    pub fn begin(
        &self,
        host_id: &str,
        redirect: &str,
        returning: Option<&Account>,
    ) -> Result<AuthAttempt> {
        if host_id.is_empty() {
            return Err(Error::Invalid("A stable host ID is required."));
        }
        let redirect_uri = callback_uri(redirect)?;
        if let Some(account) = returning
            && (account.host_id != host_id
                || account.issuer != ISSUER
                || account.client_id == DYNAMIC_CLIENT)
        {
            return Err(Error::Invalid("This registration belongs to another host."));
        }
        let state = random_value()?;
        let nonce = random_value()?;
        let verifier = random_value()?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut authorization_url = Url::parse(&self.discovery.authorization_endpoint)
            .map_err(|_| Error::Invalid("Invalid authorization endpoint."))?;
        let client_id = returning
            .map(|a| a.client_id.as_str())
            .unwrap_or(DYNAMIC_CLIENT);
        {
            let mut query = authorization_url.query_pairs_mut();
            query
                .append_pair("client_id", client_id)
                .append_pair("ext_agent_host_id", host_id)
                .append_pair("response_type", "code")
                .append_pair("redirect_uri", redirect_uri.as_str())
                .append_pair("scope", SCOPES)
                .append_pair("resource", RESOURCE)
                .append_pair("state", &state)
                .append_pair("nonce", &nonce)
                .append_pair("code_challenge_method", "S256")
                .append_pair("code_challenge", &challenge);
            if returning.is_none() {
                query.append_pair("agent_name_hint", "Ripple");
            }
        }
        Ok(AuthAttempt {
            authorization_url,
            redirect_uri,
            state,
            nonce,
            verifier,
            host_id: host_id.into(),
            returning: returning.cloned(),
            expires_at: now() + 600,
        })
    }

    pub async fn finish(&self, attempt: AuthAttempt, callback: &str) -> Result<Credentials> {
        let (code, client_id) = attempt.validate_callback(callback)?;
        let response = self
            .client
            .post(&self.discovery.token_endpoint)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", &client_id),
                ("code", &code),
                ("code_verifier", &attempt.verifier),
                ("redirect_uri", attempt.redirect_uri.as_str()),
                ("resource", RESOURCE),
            ])
            .send()
            .await?;
        let tokens: TokenResponse = checked(response, "code exchange").await?.json().await?;
        let id_token = tokens
            .id_token
            .as_deref()
            .ok_or(Error::Invalid("Missing ID token."))?;
        let identity = self
            .verify_identity(id_token, &client_id, Some(&attempt.nonce))
            .await?;
        if let Some(account) = &attempt.returning
            && identity.sub != account.subject
        {
            return Err(Error::Invalid("The selected ChatGPT account changed."));
        }
        let account = Account {
            issuer: ISSUER.into(),
            subject: identity.sub,
            client_id,
            host_id: attempt.host_id,
            email: identity.email,
        };
        credentials(tokens, account, None)
    }

    pub async fn refresh(&self, current: &Credentials) -> Result<Credentials> {
        let response = self
            .client
            .post(&self.discovery.token_endpoint)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", current.account.client_id.as_str()),
                ("refresh_token", current.refresh_token.as_str()),
                ("resource", RESOURCE),
            ])
            .send()
            .await?;
        let tokens: TokenResponse = checked(response, "token renewal").await?.json().await?;
        if let Some(id_token) = &tokens.id_token {
            let identity = self
                .verify_identity(id_token, &current.account.client_id, None)
                .await?;
            if identity.sub != current.account.subject {
                return Err(Error::Invalid("Token renewal changed the account."));
            }
        }
        credentials(tokens, current.account.clone(), Some(current))
    }

    async fn verify_identity(
        &self,
        token: &str,
        client_id: &str,
        nonce: Option<&str>,
    ) -> Result<Identity> {
        let response = self.client.get(&self.discovery.jwks_uri).send().await?;
        let keys: JwkSet = checked(response, "identity verification")
            .await?
            .json()
            .await?;
        verify_identity(token, client_id, nonce, &keys)
    }
}

fn verify_identity(
    token: &str,
    client_id: &str,
    nonce: Option<&str>,
    keys: &JwkSet,
) -> Result<Identity> {
    let invalid = || Error::Invalid("The OpenAI identity token could not be verified.");
    let header = decode_header(token).map_err(|_| invalid())?;
    if header.alg != Algorithm::RS256 {
        return Err(invalid());
    }
    let key = keys
        .find(header.kid.as_deref().ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    let key = DecodingKey::from_jwk(key).map_err(|_| invalid())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.leeway = 5;
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    validation.set_required_spec_claims(&["sub", "exp", "iat", "iss", "aud"]);
    let identity = decode::<Identity>(token, &key, &validation)
        .map_err(|_| invalid())?
        .claims;
    if identity.sub.is_empty()
        || identity.iat > now() + 5
        || nonce.is_some_and(|n| identity.nonce.as_deref() != Some(n))
    {
        return Err(invalid());
    }
    Ok(identity)
}

impl AuthAttempt {
    fn validate_callback(&self, callback: &str) -> Result<(String, String)> {
        let uri = Url::parse(callback).map_err(|_| Error::Invalid("Invalid OAuth callback."))?;
        if now() >= self.expires_at
            || uri.origin() != self.redirect_uri.origin()
            || uri.path() != self.redirect_uri.path()
            || uri.fragment().is_some()
        {
            return Err(Error::Invalid(
                "The sign-in attempt expired or the callback changed.",
            ));
        }
        let mut params = std::collections::HashMap::new();
        for (key, value) in uri.query_pairs() {
            if params
                .insert(key.into_owned(), value.into_owned())
                .is_some()
            {
                return Err(Error::Invalid("Duplicate OAuth callback parameters."));
            }
        }
        if params.get("state") != Some(&self.state) {
            return Err(Error::Invalid("The OAuth state does not match."));
        }
        if params.contains_key("error") {
            return Err(Error::Invalid("ChatGPT sign-in was declined or failed."));
        }
        let code = params
            .get("code")
            .filter(|v| !v.is_empty())
            .ok_or(Error::Invalid("Missing authorization code."))?;
        let client_id = match &self.returning {
            Some(account) => {
                if params
                    .get("client_id")
                    .is_some_and(|id| id != &account.client_id)
                {
                    return Err(Error::Invalid("The OAuth client changed."));
                }
                account.client_id.clone()
            }
            None => params
                .get("client_id")
                .filter(|id| !id.is_empty() && id.as_str() != DYNAMIC_CLIENT)
                .ok_or(Error::Invalid("Registration did not issue a client ID."))?
                .clone(),
        };
        Ok((code.clone(), client_id))
    }
}

fn credentials(
    tokens: TokenResponse,
    account: Account,
    previous: Option<&Credentials>,
) -> Result<Credentials> {
    let scopes: Vec<String> = match tokens.scope {
        Some(scope) => scope.split_whitespace().map(str::to_owned).collect(),
        None => previous.map(|p| p.scopes.clone()).unwrap_or_default(),
    };
    if !scopes.iter().any(|s| s == PLAN_SCOPE) {
        return Err(Error::Invalid("ChatGPT plan usage was not authorized."));
    }
    if !tokens.token_type.eq_ignore_ascii_case("bearer")
        || tokens.access_token.is_empty()
        || tokens.expires_in == 0
    {
        return Err(Error::Invalid("Invalid OAuth token response."));
    }
    let refresh_token = tokens
        .refresh_token
        .or_else(|| previous.map(|p| p.refresh_token.clone()))
        .filter(|t| !t.is_empty())
        .ok_or(Error::Invalid("Missing refresh token."))?;
    let id_token = tokens
        .id_token
        .or_else(|| previous.map(|p| p.id_token.clone()))
        .filter(|t| !t.is_empty())
        .ok_or(Error::Invalid("Missing ID token."))?;
    let expires_at = now()
        .checked_add(tokens.expires_in)
        .ok_or(Error::Invalid("Invalid token expiry."))?;
    Ok(Credentials {
        account,
        access_token: tokens.access_token,
        refresh_token,
        id_token,
        scopes,
        expires_at,
    })
}

pub(crate) async fn checked(
    response: reqwest::Response,
    operation: &'static str,
) -> Result<reqwest::Response> {
    if !response.status().is_success() {
        return Err(Error::Http {
            operation,
            status: response.status().as_u16(),
        });
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_signed_identity_and_rejects_wrong_nonce_audience_issuer_and_expiry() {
        use aws_lc_rs::{
            rand::SystemRandom,
            rsa::{KeyPair, KeySize},
            signature::{KeyPair as _, RSA_PKCS1_SHA256},
        };
        use serde_json::json;
        let key = KeyPair::generate(KeySize::Rsa2048).unwrap();
        let public = key.public_key();
        let keys: JwkSet = serde_json::from_value(
            json!({"keys":[{"kty":"RSA","kid":"test","alg":"RS256","use":"sig",
            "n":URL_SAFE_NO_PAD.encode(public.modulus().big_endian_without_leading_zero()),
            "e":URL_SAFE_NO_PAD.encode(public.exponent().big_endian_without_leading_zero())}]}),
        )
        .unwrap();
        let sign = |claims: &serde_json::Value| {
            let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","kid":"test"}"#);
            let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap());
            let input = format!("{header}.{payload}");
            let mut signature = vec![0; key.public_modulus_len()];
            key.sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                input.as_bytes(),
                &mut signature,
            )
            .unwrap();
            format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
        };
        let claims = json!({"sub":"test-user","iat":now(),"exp":now()+3600,"iss":ISSUER,"aud":"oaiapp_test","nonce":"test-nonce"});
        let token = sign(&claims);
        assert_eq!(
            verify_identity(&token, "oaiapp_test", Some("test-nonce"), &keys)
                .unwrap()
                .sub,
            "test-user"
        );
        assert!(verify_identity(&token, "oaiapp_test", Some("wrong"), &keys).is_err());
        assert!(verify_identity(&token, "wrong-client", Some("test-nonce"), &keys).is_err());
        for (field, value) in [
            ("iss", json!("https://other.example")),
            ("exp", json!(now() - 100)),
            ("iat", json!(now() + 100)),
        ] {
            let mut modified = claims.clone();
            modified[field] = value;
            assert!(
                verify_identity(&sign(&modified), "oaiapp_test", Some("test-nonce"), &keys)
                    .is_err()
            );
        }
        let forged = format!(
            "{}.{}",
            token.rsplit_once('.').unwrap().0,
            URL_SAFE_NO_PAD.encode(vec![0; key.public_modulus_len()])
        );
        assert!(verify_identity(&forged, "oaiapp_test", Some("test-nonce"), &keys).is_err());
    }

    fn client() -> OAuthClient {
        OAuthClient {
            client: Client::new(),
            discovery: Discovery {
                issuer: ISSUER.into(),
                authorization_endpoint: format!("{ISSUER}/api/accounts/authorize"),
                token_endpoint: format!("{ISSUER}/api/accounts/oauth/token"),
                jwks_uri: format!("{ISSUER}/.well-known/jwks.json"),
            },
        }
    }

    fn callback(attempt: &AuthAttempt, extra: &str) -> String {
        format!(
            "{}?state={}&code=test&{extra}",
            attempt.redirect_uri, attempt.state
        )
    }

    #[test]
    fn creates_pkce_and_requires_issued_client_and_matching_state() {
        let attempt = client()
            .begin("host", "http://127.0.0.1:4310/auth/callback", None)
            .unwrap();
        let pairs: std::collections::HashMap<_, _> =
            attempt.authorization_url.query_pairs().collect();
        assert_eq!(pairs["client_id"], DYNAMIC_CLIENT);
        assert_eq!(pairs["agent_name_hint"], "Ripple");
        assert_eq!(
            pairs["code_challenge"],
            URL_SAFE_NO_PAD.encode(Sha256::digest(attempt.verifier.as_bytes()))
        );
        assert!(
            attempt
                .validate_callback(&callback(&attempt, "client_id=oaiapp_test"))
                .is_ok()
        );
        assert!(
            attempt
                .validate_callback(&callback(&attempt, "client_id=dynamic_agent_client"))
                .is_err()
        );
        assert!(attempt.validate_callback(&callback(&attempt, "")).is_err());
        assert!(
            attempt
                .validate_callback(&callback(&attempt, "client_id=oaiapp_test&state=wrong"))
                .is_err()
        );
        assert!(
            attempt
                .validate_callback(
                    &callback(&attempt, "client_id=oaiapp_test").replace(&attempt.state, "wrong")
                )
                .is_err()
        );
    }

    #[test]
    fn reuses_registration_without_replacing_account_or_callback() {
        let account = Account {
            issuer: ISSUER.into(),
            subject: "user".into(),
            client_id: "oaiapp_test".into(),
            host_id: "host".into(),
            email: None,
        };
        let mut attempt = client()
            .begin(
                "host",
                "http://127.0.0.1:4310/auth/callback",
                Some(&account),
            )
            .unwrap();
        assert!(
            !attempt
                .authorization_url
                .query_pairs()
                .any(|(k, _)| k == "agent_name_hint")
        );
        assert!(attempt.validate_callback(&callback(&attempt, "")).is_ok());
        assert!(
            attempt
                .validate_callback(&callback(&attempt, "client_id=another"))
                .is_err()
        );
        assert!(
            attempt
                .validate_callback(&callback(&attempt, "error=access_denied"))
                .is_err()
        );
        assert!(
            attempt
                .validate_callback(&callback(&attempt, "").replace("4310", "4311"))
                .is_err()
        );
        attempt.expires_at = 0;
        assert!(attempt.validate_callback(&callback(&attempt, "")).is_err());
        assert!(
            client()
                .begin("host", "http://localhost:4310/auth/callback", None)
                .is_err()
        );
    }

    #[test]
    fn rejects_unverifiable_identity_and_missing_plan_scope() {
        let keys = JwkSet { keys: vec![] };
        assert!(verify_identity("not.a.jwt", "client", Some("nonce"), &keys).is_err());
        let tokens = TokenResponse {
            access_token: "test".into(),
            refresh_token: Some("test".into()),
            id_token: Some("test".into()),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: Some("openid email".into()),
        };
        let account = Account {
            issuer: ISSUER.into(),
            subject: "user".into(),
            client_id: "oaiapp_test".into(),
            host_id: "host".into(),
            email: None,
        };
        assert!(credentials(tokens, account, None).is_err());
    }
}
