use std::{env, fmt, time::Duration};

use lettre::{
    Address, AsyncSmtpTransport, AsyncTransport, Message as LettreMessage, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::{authentication::Credentials, extension::ClientId},
};
use tracing::info;

const DEFAULT_WEB_ORIGIN: &str = "http://localhost:5173";
const DEFAULT_FROM_NAME: &str = "World Keystone";
const DEFAULT_FROM_EMAIL: &str = "no-reply@worldkeystone.com";
const DEFAULT_SMTP_HOST: &str = "127.0.0.1";
const DEFAULT_SMTP_PORT: u16 = 25;
const DEFAULT_SMTP_STARTTLS_PORT: u16 = 587;
const DEFAULT_SMTP_IMPLICIT_TLS_PORT: u16 = 465;
const DEFAULT_SMTP_TIMEOUT_SECONDS: u64 = 10;

#[derive(Debug, Clone)]
pub enum Mailer {
    Log(LogMailer),
    Smtp(SmtpMailer),
}

#[derive(Debug, Clone)]
pub struct LogMailer {
    from_name: String,
    from_email: String,
    web_origin: String,
}

#[derive(Debug, Clone)]
pub struct SmtpMailer {
    from_name: String,
    from_email: String,
    web_origin: String,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

#[derive(Debug, Clone)]
pub struct MailMessage {
    pub to_email: String,
    pub subject: String,
    pub text_body: String,
}

#[derive(Debug)]
pub struct MailError {
    message: String,
}

impl MailError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MailError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmtpSecurity {
    ImplicitTls,
    StartTls,
    None,
}

impl SmtpSecurity {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "implicit_tls" | "tls" | "ssl" | "ssl_tls" | "smtps" | "wrapper" => {
                Ok(Self::ImplicitTls)
            }
            "starttls" | "required" | "tls_required" => Ok(Self::StartTls),
            "none" | "plain" | "local" | "unencrypted" => Ok(Self::None),
            _ => Err("MAIL_SMTP_SECURITY must be implicit_tls, starttls, or none.".to_string()),
        }
    }

    fn default_port(self) -> u16 {
        match self {
            Self::ImplicitTls => DEFAULT_SMTP_IMPLICIT_TLS_PORT,
            Self::StartTls => DEFAULT_SMTP_STARTTLS_PORT,
            Self::None => DEFAULT_SMTP_PORT,
        }
    }
}

impl Mailer {
    pub fn from_env(is_production: bool) -> Result<Self, String> {
        let mode = env::var("MAIL_MODE").unwrap_or_else(|_| {
            if is_production {
                "smtp".to_string()
            } else {
                "log".to_string()
            }
        });

        let from_name =
            env::var("MAIL_FROM_NAME").unwrap_or_else(|_| DEFAULT_FROM_NAME.to_string());
        let from_email =
            env::var("MAIL_FROM_EMAIL").unwrap_or_else(|_| DEFAULT_FROM_EMAIL.to_string());
        let web_origin = env::var("PUBLIC_WEB_ORIGIN")
            .ok()
            .or_else(|| env::var("WEB_ORIGIN").ok())
            .unwrap_or_else(|| DEFAULT_WEB_ORIGIN.to_string())
            .trim_end_matches('/')
            .to_string();

        if is_production && !web_origin.starts_with("https://") {
            return Err(
                "PUBLIC_WEB_ORIGIN or WEB_ORIGIN must use https:// in production.".to_string(),
            );
        }

        match mode.trim().to_ascii_lowercase().as_str() {
            "log" => {
                if is_production {
                    return Err("MAIL_MODE=log is not allowed in production.".to_string());
                }

                Ok(Mailer::Log(LogMailer {
                    from_name,
                    from_email,
                    web_origin,
                }))
            }
            "smtp" => {
                let host =
                    env::var("MAIL_SMTP_HOST").unwrap_or_else(|_| DEFAULT_SMTP_HOST.to_string());
                let configured_port = env::var("MAIL_SMTP_PORT")
                    .ok()
                    .map(|value| {
                        value
                            .parse::<u16>()
                            .map_err(|_| "MAIL_SMTP_PORT must be a valid port.".to_string())
                    })
                    .transpose()?;
                let security = env::var("MAIL_SMTP_SECURITY")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| SmtpSecurity::parse(&value))
                    .transpose()?
                    .unwrap_or_else(|| infer_smtp_security(&host, configured_port));
                let port = configured_port.unwrap_or_else(|| security.default_port());
                let helo_name = env::var("MAIL_SMTP_HELO_NAME")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "localhost".to_string());
                let username = env::var("MAIL_SMTP_USERNAME")
                    .ok()
                    .filter(|value| !value.trim().is_empty());
                let password = env::var("MAIL_SMTP_PASSWORD")
                    .ok()
                    .filter(|value| !value.trim().is_empty());
                let timeout_seconds = env::var("MAIL_SMTP_TIMEOUT_SECONDS")
                    .ok()
                    .map(|value| {
                        value.parse::<u64>().map_err(|_| {
                            "MAIL_SMTP_TIMEOUT_SECONDS must be a positive integer.".to_string()
                        })
                    })
                    .transpose()?
                    .unwrap_or(DEFAULT_SMTP_TIMEOUT_SECONDS);

                if username.is_some() != password.is_some() {
                    return Err(
                        "MAIL_SMTP_USERNAME and MAIL_SMTP_PASSWORD must be set together."
                            .to_string(),
                    );
                }

                if is_production && security == SmtpSecurity::None && !is_loopback_smtp_host(&host)
                {
                    return Err(
                        "MAIL_SMTP_SECURITY=none is only allowed for a local relay in production."
                            .to_string(),
                    );
                }

                if is_production && !is_loopback_smtp_host(&host) && username.is_none() {
                    return Err(
                        "Production SMTP relays outside localhost must set MAIL_SMTP_USERNAME and MAIL_SMTP_PASSWORD."
                            .to_string(),
                    );
                }

                let timeout = Duration::from_secs(timeout_seconds.max(1));
                let transport = build_smtp_transport(
                    &host, port, security, &helo_name, username, password, timeout,
                )?;

                Ok(Mailer::Smtp(SmtpMailer {
                    from_name,
                    from_email,
                    web_origin,
                    transport,
                }))
            }
            _ => Err("MAIL_MODE must be either log or smtp.".to_string()),
        }
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        token: &str,
    ) -> Result<(), MailError> {
        let message = match self {
            Mailer::Log(config) => {
                build_verification_message(&config.from_email, &config.web_origin, to_email, token)
            }
            Mailer::Smtp(config) => {
                build_verification_message(&config.from_email, &config.web_origin, to_email, token)
            }
        }?;

        self.send(message).await
    }

    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        token: &str,
    ) -> Result<(), MailError> {
        let message = match self {
            Mailer::Log(config) => build_password_reset_message(
                &config.from_email,
                &config.web_origin,
                to_email,
                token,
            ),
            Mailer::Smtp(config) => build_password_reset_message(
                &config.from_email,
                &config.web_origin,
                to_email,
                token,
            ),
        }?;

        self.send(message).await
    }

    async fn send(&self, message: MailMessage) -> Result<(), MailError> {
        match self {
            Mailer::Log(config) => {
                info!(
                    to = %message.to_email,
                    from_name = %config.from_name,
                    from = %config.from_email,
                    subject = %message.subject,
                    body = %message.text_body,
                    "mail delivery logged"
                );
                Ok(())
            }
            Mailer::Smtp(config) => config.send(message).await,
        }
    }
}

impl SmtpMailer {
    async fn send(&self, message: MailMessage) -> Result<(), MailError> {
        let email = build_lettre_message(&self.from_name, &self.from_email, &message)?;
        self.transport
            .send(email)
            .await
            .map(|_| ())
            .map_err(|err| MailError::new(format!("SMTP delivery failed: {err}")))
    }
}

fn build_verification_message(
    from_email: &str,
    web_origin: &str,
    to_email: &str,
    token: &str,
) -> Result<MailMessage, MailError> {
    validate_email_address(from_email)?;
    validate_email_address(to_email)?;

    Ok(MailMessage {
        to_email: to_email.to_string(),
        subject: "Verify your World Keystone account".to_string(),
        text_body: format!(
            "Your World Keystone verification token is:\n\n{token}\n\nIt expires in 24 hours.\n\nOpen {web_origin} and paste this token into the email verification form.\n\nIf you did not request this, you can ignore this message.",
        ),
    })
}

fn build_password_reset_message(
    from_email: &str,
    web_origin: &str,
    to_email: &str,
    token: &str,
) -> Result<MailMessage, MailError> {
    validate_email_address(from_email)?;
    validate_email_address(to_email)?;

    Ok(MailMessage {
        to_email: to_email.to_string(),
        subject: "Reset your World Keystone password".to_string(),
        text_body: format!(
            "Your World Keystone password reset token is:\n\n{token}\n\nIt expires in 1 hour.\n\nOpen {web_origin} and paste this token into the password reset form.\n\nIf you did not request this, you can ignore this message.",
        ),
    })
}

fn build_smtp_transport(
    host: &str,
    port: u16,
    security: SmtpSecurity,
    helo_name: &str,
    username: Option<String>,
    password: Option<String>,
    duration: Duration,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let mut builder = match security {
        SmtpSecurity::ImplicitTls => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
            .map_err(|err| format!("MAIL_SMTP_HOST TLS configuration failed: {err}"))?,
        SmtpSecurity::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|err| format!("MAIL_SMTP_HOST STARTTLS configuration failed: {err}"))?,
        SmtpSecurity::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
    }
    .port(port)
    .timeout(Some(duration));

    let helo_name = helo_name.trim();
    if !helo_name.is_empty() {
        builder = builder.hello_name(ClientId::Domain(helo_name.to_string()));
    }

    if let (Some(username), Some(password)) = (username, password) {
        builder = builder.credentials(Credentials::new(username, password));
    }

    Ok(builder.build())
}

fn build_lettre_message(
    from_name: &str,
    from_email: &str,
    message: &MailMessage,
) -> Result<LettreMessage, MailError> {
    let subject = sanitize_header_value(&message.subject)?;
    let from_address = parse_mail_address(from_email, "Invalid sender email address.")?;
    let to_address = parse_mail_address(&message.to_email, "Invalid recipient email address.")?;
    let from_name = sanitize_header_value(from_name)?;
    let from = if from_name.trim().is_empty() {
        Mailbox::new(None, from_address)
    } else {
        Mailbox::new(Some(from_name), from_address)
    };

    LettreMessage::builder()
        .from(from)
        .to(Mailbox::new(None, to_address))
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(message.text_body.clone())
        .map_err(|err| MailError::new(format!("Failed to build email message: {err}")))
}

fn parse_mail_address(value: &str, error_message: &str) -> Result<Address, MailError> {
    validate_email_address(value)?;
    value
        .parse::<Address>()
        .map_err(|_| MailError::new(error_message))
}

fn validate_email_address(email: &str) -> Result<(), MailError> {
    if email.contains(['\r', '\n']) || !email.contains('@') || email.trim() != email {
        return Err(MailError::new("Invalid email address for mail delivery."));
    }

    Ok(())
}

fn sanitize_header_value(value: &str) -> Result<String, MailError> {
    if value.contains(['\r', '\n']) {
        return Err(MailError::new("Header values cannot contain newlines."));
    }

    Ok(value.trim().to_string())
}

fn infer_smtp_security(host: &str, port: Option<u16>) -> SmtpSecurity {
    match port {
        Some(DEFAULT_SMTP_IMPLICIT_TLS_PORT | 2465) => SmtpSecurity::ImplicitTls,
        Some(DEFAULT_SMTP_STARTTLS_PORT | 2587) => SmtpSecurity::StartTls,
        Some(DEFAULT_SMTP_PORT) if is_loopback_smtp_host(host) => SmtpSecurity::None,
        Some(_) if is_loopback_smtp_host(host) => SmtpSecurity::None,
        Some(_) => SmtpSecurity::StartTls,
        None if is_loopback_smtp_host(host) => SmtpSecurity::None,
        None => SmtpSecurity::StartTls,
    }
}

fn is_loopback_smtp_host(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "[::1]"
    )
}

#[cfg(test)]
mod tests {
    use super::{SmtpSecurity, infer_smtp_security, is_loopback_smtp_host, parse_mail_address};

    #[test]
    fn smtp_security_accepts_common_provider_terms() {
        assert_eq!(
            SmtpSecurity::parse("implicit_tls").expect("implicit tls should parse"),
            SmtpSecurity::ImplicitTls
        );
        assert_eq!(
            SmtpSecurity::parse("smtps").expect("smtps should parse"),
            SmtpSecurity::ImplicitTls
        );
        assert_eq!(
            SmtpSecurity::parse("STARTTLS").expect("starttls should parse"),
            SmtpSecurity::StartTls
        );
        assert_eq!(
            SmtpSecurity::parse("plain").expect("plain should parse"),
            SmtpSecurity::None
        );
    }

    #[test]
    fn smtp_security_infers_from_host_and_port() {
        assert_eq!(
            infer_smtp_security("smtp.resend.com", Some(465)),
            SmtpSecurity::ImplicitTls
        );
        assert_eq!(
            infer_smtp_security("smtp.resend.com", Some(587)),
            SmtpSecurity::StartTls
        );
        assert_eq!(
            infer_smtp_security("127.0.0.1", Some(25)),
            SmtpSecurity::None
        );
        assert_eq!(
            infer_smtp_security("smtp.example.com", None),
            SmtpSecurity::StartTls
        );
    }

    #[test]
    fn loopback_detection_allows_common_local_relay_hosts() {
        assert!(is_loopback_smtp_host("localhost"));
        assert!(is_loopback_smtp_host("127.0.0.1"));
        assert!(is_loopback_smtp_host("::1"));
        assert!(!is_loopback_smtp_host("smtp.example.com"));
    }

    #[test]
    fn mail_address_parsing_rejects_header_injection() {
        assert!(parse_mail_address("person@example.com", "bad").is_ok());
        assert!(parse_mail_address("person@example.com\r\nBcc: other@example.com", "bad").is_err());
    }
}
