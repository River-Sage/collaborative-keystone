use std::{env, fmt, time::Duration};

use chrono::Utc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::timeout,
};
use tracing::info;
use uuid::Uuid;

const DEFAULT_WEB_ORIGIN: &str = "http://localhost:5173";
const DEFAULT_FROM_NAME: &str = "Collaborative Keystone";
const DEFAULT_FROM_EMAIL: &str = "no-reply@collaborativekeystone.com";
const DEFAULT_SMTP_HOST: &str = "127.0.0.1";
const DEFAULT_SMTP_PORT: u16 = 25;
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
    host: String,
    port: u16,
    helo_name: String,
    username: Option<String>,
    password: Option<String>,
    timeout: Duration,
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

#[derive(Debug)]
struct SmtpResponse {
    code: u16,
    lines: Vec<String>,
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
                let port = env::var("MAIL_SMTP_PORT")
                    .ok()
                    .map(|value| {
                        value
                            .parse::<u16>()
                            .map_err(|_| "MAIL_SMTP_PORT must be a valid port.".to_string())
                    })
                    .transpose()?
                    .unwrap_or(DEFAULT_SMTP_PORT);
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

                Ok(Mailer::Smtp(SmtpMailer {
                    from_name,
                    from_email,
                    web_origin,
                    host,
                    port,
                    helo_name,
                    username,
                    password,
                    timeout: Duration::from_secs(timeout_seconds.max(1)),
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
        let stream = timeout(
            self.timeout,
            TcpStream::connect((self.host.as_str(), self.port)),
        )
        .await
        .map_err(|_| MailError::new("SMTP connection timed out."))?
        .map_err(|err| MailError::new(format!("SMTP connection failed: {err}")))?;

        let mut reader = BufReader::new(stream);
        expect_response(&mut reader, self.timeout, &[220]).await?;

        send_line(
            &mut reader,
            self.timeout,
            &format!("EHLO {}", self.helo_name),
        )
        .await?;
        let ehlo_response = read_response(&mut reader, self.timeout).await?;
        if ehlo_response.code != 250 {
            send_line(
                &mut reader,
                self.timeout,
                &format!("HELO {}", self.helo_name),
            )
            .await?;
            expect_response(&mut reader, self.timeout, &[250]).await?;
        }

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            send_line(&mut reader, self.timeout, "AUTH LOGIN").await?;
            expect_response(&mut reader, self.timeout, &[334]).await?;
            send_line(
                &mut reader,
                self.timeout,
                &base64_encode(username.as_bytes()),
            )
            .await?;
            expect_response(&mut reader, self.timeout, &[334]).await?;
            send_line(
                &mut reader,
                self.timeout,
                &base64_encode(password.as_bytes()),
            )
            .await?;
            expect_response(&mut reader, self.timeout, &[235]).await?;
        }

        send_line(
            &mut reader,
            self.timeout,
            &format!("MAIL FROM:<{}>", self.from_email),
        )
        .await?;
        expect_response(&mut reader, self.timeout, &[250]).await?;

        send_line(
            &mut reader,
            self.timeout,
            &format!("RCPT TO:<{}>", message.to_email),
        )
        .await?;
        expect_response(&mut reader, self.timeout, &[250, 251]).await?;

        send_line(&mut reader, self.timeout, "DATA").await?;
        expect_response(&mut reader, self.timeout, &[354]).await?;

        let raw_message = format!(
            "{}\r\n.\r\n",
            dot_stuff(&format_message(
                &self.from_name,
                &self.from_email,
                &message
            )?)
        );
        write_all(&mut reader, self.timeout, raw_message.as_bytes()).await?;
        expect_response(&mut reader, self.timeout, &[250]).await?;

        send_line(&mut reader, self.timeout, "QUIT").await?;
        let _ = read_response(&mut reader, self.timeout).await;

        Ok(())
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
        subject: "Verify your Collaborative Keystone account".to_string(),
        text_body: format!(
            "Your Collaborative Keystone verification token is:\n\n{token}\n\nIt expires in 24 hours.\n\nOpen {web_origin} and paste this token into the email verification form.\n\nIf you did not request this, you can ignore this message.",
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
        subject: "Reset your Collaborative Keystone password".to_string(),
        text_body: format!(
            "Your Collaborative Keystone password reset token is:\n\n{token}\n\nIt expires in 1 hour.\n\nOpen {web_origin} and paste this token into the password reset form.\n\nIf you did not request this, you can ignore this message.",
        ),
    })
}

fn format_message(
    from_name: &str,
    from_email: &str,
    message: &MailMessage,
) -> Result<String, MailError> {
    let subject = sanitize_header_value(&message.subject)?;
    let to_email = sanitize_header_value(&message.to_email)?;
    let message_id_domain = from_email.split('@').nth(1).unwrap_or("localhost");

    Ok(format!(
        "From: {}\r\nTo: <{}>\r\nSubject: {}\r\nDate: {}\r\nMessage-ID: <{}@{}>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}",
        format_mailbox(from_name, from_email)?,
        to_email,
        subject,
        Utc::now().to_rfc2822(),
        Uuid::new_v4(),
        sanitize_header_value(message_id_domain)?,
        normalize_body_newlines(&message.text_body)
    ))
}

fn format_mailbox(name: &str, email: &str) -> Result<String, MailError> {
    validate_email_address(email)?;
    let email = sanitize_header_value(email)?;
    let name = sanitize_header_value(name)?;

    if name.trim().is_empty() {
        Ok(format!("<{email}>"))
    } else {
        Ok(format!("\"{}\" <{}>", escape_quoted_header(&name), email))
    }
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

fn escape_quoted_header(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_body_newlines(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

fn dot_stuff(message: &str) -> String {
    message
        .lines()
        .map(|line| {
            if line.starts_with('.') {
                format!(".{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

async fn send_line(
    reader: &mut BufReader<TcpStream>,
    duration: Duration,
    line: &str,
) -> Result<(), MailError> {
    let line = format!("{line}\r\n");
    write_all(reader, duration, line.as_bytes()).await
}

async fn write_all(
    reader: &mut BufReader<TcpStream>,
    duration: Duration,
    bytes: &[u8],
) -> Result<(), MailError> {
    timeout(duration, reader.get_mut().write_all(bytes))
        .await
        .map_err(|_| MailError::new("SMTP write timed out."))?
        .map_err(|err| MailError::new(format!("SMTP write failed: {err}")))
}

async fn expect_response(
    reader: &mut BufReader<TcpStream>,
    duration: Duration,
    accepted_codes: &[u16],
) -> Result<SmtpResponse, MailError> {
    let response = read_response(reader, duration).await?;
    if accepted_codes.contains(&response.code) {
        Ok(response)
    } else {
        Err(MailError::new(format!(
            "SMTP server returned {}: {}",
            response.code,
            response.lines.join(" | ")
        )))
    }
}

async fn read_response(
    reader: &mut BufReader<TcpStream>,
    duration: Duration,
) -> Result<SmtpResponse, MailError> {
    let mut lines = Vec::new();

    loop {
        let mut line = String::new();
        let bytes_read = timeout(duration, reader.read_line(&mut line))
            .await
            .map_err(|_| MailError::new("SMTP read timed out."))?
            .map_err(|err| MailError::new(format!("SMTP read failed: {err}")))?;

        if bytes_read == 0 {
            return Err(MailError::new("SMTP server closed the connection."));
        }

        let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
        if trimmed.len() < 3 {
            return Err(MailError::new("SMTP server returned a malformed response."));
        }

        let parsed_code = trimmed[0..3]
            .parse::<u16>()
            .map_err(|_| MailError::new("SMTP server returned a malformed status code."))?;
        let is_last_line = trimmed.as_bytes().get(3).copied() != Some(b'-');

        lines.push(trimmed);

        if is_last_line {
            return Ok(SmtpResponse {
                code: parsed_code,
                lines,
            });
        }
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }

        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }

    encoded
}
