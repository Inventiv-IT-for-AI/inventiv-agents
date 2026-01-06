use lettre::{
    message::{header::ContentType, Mailbox, Message, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: Option<String>,
    pub use_tls: bool,
}

impl SmtpConfig {
    /// Load SMTP configuration from environment variables
    pub fn from_env() -> Option<Self> {
        let server = std::env::var("SMTP_SERVER").ok()?;
        let port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(465);
        let username = std::env::var("SMTP_USERNAME").ok()?;
        let password = std::env::var("SMTP_PASSWORD")
            .ok()
            .or_else(|| {
                // Try reading from file (for secrets)
                std::env::var("SMTP_PASSWORD_FILE")
                    .ok()
                    .and_then(|path| std::fs::read_to_string(path).ok())
            })?;
        let from_email = std::env::var("SMTP_FROM_EMAIL")
            .ok()
            .unwrap_or_else(|| username.clone());
        let from_name = std::env::var("SMTP_FROM_NAME").ok();
        let use_tls = std::env::var("SMTP_USE_TLS")
            .ok()
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);

        Some(Self {
            server,
            port,
            username,
            password,
            from_email,
            from_name,
            use_tls,
        })
    }

    /// Create SMTP transport
    pub fn create_transport(&self) -> AsyncSmtpTransport<Tokio1Executor> {
        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&self.server)
            .expect("Failed to create SMTP relay")
            .port(self.port)
            .timeout(Some(Duration::from_secs(30)));

        // Add authentication
        builder = builder.credentials(Credentials::new(
            self.username.clone(),
            self.password.clone(),
        ));

        // TLS is handled automatically by lettre based on port:
        // - Port 465: implicit TLS (SSL)
        // - Port 587: STARTTLS
        // - Port 25: no encryption (not recommended)

        builder.build()
    }
}

#[derive(Debug)]
pub struct EmailService {
    config: SmtpConfig,
}

impl EmailService {
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    pub fn from_env() -> Option<Self> {
        SmtpConfig::from_env().map(Self::new)
    }

    /// Send a plain text email
    pub async fn send_text(
        &self,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), EmailError> {
        let from_mailbox: Mailbox = if let Some(from_name) = &self.config.from_name {
            format!("{} <{}>", from_name, self.config.from_email)
                .parse()
                .map_err(|_| EmailError::InvalidAddress("from".to_string()))?
        } else {
            self.config
                .from_email
                .parse()
                .map_err(|_| EmailError::InvalidAddress("from".to_string()))?
        };

        let to_mailbox: Mailbox = to
            .parse()
            .map_err(|_| EmailError::InvalidAddress("to".to_string()))?;

        let email = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(body.to_string()),
            )
            .map_err(|e| EmailError::BuildError(e.to_string()))?;

        self.send(email).await
    }

    /// Send an HTML email
    pub async fn send_html(
        &self,
        to: &str,
        subject: &str,
        html_body: &str,
        text_body: Option<&str>,
    ) -> Result<(), EmailError> {
        let from_mailbox: Mailbox = if let Some(from_name) = &self.config.from_name {
            format!("{} <{}>", from_name, self.config.from_email)
                .parse()
                .map_err(|_| EmailError::InvalidAddress("from".to_string()))?
        } else {
            self.config
                .from_email
                .parse()
                .map_err(|_| EmailError::InvalidAddress("from".to_string()))?
        };

        let to_mailbox: Mailbox = to
            .parse()
            .map_err(|_| EmailError::InvalidAddress("to".to_string()))?;

        let mut builder = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject);

        let email = if let Some(text) = text_body {
            builder
                .multipart(
                    MultiPart::alternative()
                        .singlepart(
                            SinglePart::builder()
                                .header(ContentType::TEXT_PLAIN)
                                .body(text.to_string()),
                        )
                        .singlepart(
                            SinglePart::builder()
                                .header(ContentType::TEXT_HTML)
                                .body(html_body.to_string()),
                        ),
                )
                .map_err(|e| EmailError::BuildError(e.to_string()))?
        } else {
            builder
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html_body.to_string()),
                )
                .map_err(|e| EmailError::BuildError(e.to_string()))?
        };

        self.send(email).await
    }

    /// Send password reset email
    pub async fn send_password_reset(
        &self,
        to: &str,
        reset_token: &str,
        reset_url: Option<&str>,
    ) -> Result<(), EmailError> {
        // URL-encode the token to handle base64 special characters (+, /, =)
        let encoded_token = urlencoding::encode(reset_token);
        let default_url = format!("/reset-password?token={}", encoded_token);
        let reset_link = reset_url
            .map(|u| format!("{}{}", u, default_url))
            .unwrap_or_else(|| default_url);

        let subject = "Réinitialisation de votre mot de passe";
        let html_body = format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{ font-family: Arial, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .button {{ display: inline-block; padding: 12px 24px; background-color: #007bff; color: white; text-decoration: none; border-radius: 4px; margin: 20px 0; }}
        .button:hover {{ background-color: #0056b3; }}
        .footer {{ margin-top: 30px; font-size: 12px; color: #666; }}
    </style>
</head>
<body>
    <div class="container">
        <h2>Réinitialisation de votre mot de passe</h2>
        <p>Bonjour,</p>
        <p>Vous avez demandé à réinitialiser votre mot de passe. Cliquez sur le bouton ci-dessous pour continuer :</p>
        <p><a href="{}" class="button">Réinitialiser mon mot de passe</a></p>
        <p>Ou copiez-collez ce lien dans votre navigateur :</p>
        <p style="word-break: break-all; color: #666;">{}</p>
        <p>Ce lien est valide pendant 1 heure.</p>
        <p>Si vous n'avez pas demandé cette réinitialisation, ignorez cet email.</p>
        <div class="footer">
            <p>Cordialement,<br>L'équipe Inventiv Agents</p>
        </div>
    </div>
</body>
</html>
"#,
            reset_link, reset_link
        );

        let text_body = format!(
            r#"
Réinitialisation de votre mot de passe

Bonjour,

Vous avez demandé à réinitialiser votre mot de passe. Utilisez le lien suivant pour continuer :

{}

Ce lien est valide pendant 1 heure.

Si vous n'avez pas demandé cette réinitialisation, ignorez cet email.

Cordialement,
L'équipe Inventiv Agents
"#,
            reset_link
        );

        self.send_html(to, subject, &html_body, Some(&text_body))
            .await
    }

    /// Send MFA code email
    pub async fn send_mfa_code(&self, to: &str, code: &str) -> Result<(), EmailError> {
        let subject = "Code de vérification (MFA)";
        let html_body = format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{ font-family: Arial, sans-serif; line-height: 1.6; color: #333; }}
        .container {{ max-width: 600px; margin: 0 auto; padding: 20px; }}
        .code {{ font-size: 32px; font-weight: bold; text-align: center; padding: 20px; background-color: #f5f5f5; border-radius: 4px; margin: 20px 0; letter-spacing: 8px; }}
        .footer {{ margin-top: 30px; font-size: 12px; color: #666; }}
    </style>
</head>
<body>
    <div class="container">
        <h2>Code de vérification</h2>
        <p>Bonjour,</p>
        <p>Voici votre code de vérification pour l'authentification à deux facteurs :</p>
        <div class="code">{}</div>
        <p>Ce code est valide pendant 10 minutes.</p>
        <p>Si vous n'avez pas demandé ce code, ignorez cet email.</p>
        <div class="footer">
            <p>Cordialement,<br>L'équipe Inventiv Agents</p>
        </div>
    </div>
</body>
</html>
"#,
            code
        );

        let text_body = format!(
            r#"
Code de vérification

Bonjour,

Voici votre code de vérification pour l'authentification à deux facteurs :

{}

Ce code est valide pendant 10 minutes.

Si vous n'avez pas demandé ce code, ignorez cet email.

Cordialement,
L'équipe Inventiv Agents
"#,
            code
        );

        self.send_html(to, subject, &html_body, Some(&text_body))
            .await
    }

    /// Internal method to send email
    async fn send(&self, email: Message) -> Result<(), EmailError> {
        let transport = self.config.create_transport();

        // With tokio1 feature, lettre provides async send() method
        transport
            .send(email)
            .await
            .map_err(|e| EmailError::SendError(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum EmailError {
    InvalidAddress(String),
    BuildError(String),
    SendError(String),
}

impl std::fmt::Display for EmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmailError::InvalidAddress(addr) => write!(f, "Invalid email address: {}", addr),
            EmailError::BuildError(msg) => write!(f, "Failed to build email: {}", msg),
            EmailError::SendError(msg) => write!(f, "Failed to send email: {}", msg),
        }
    }
}

impl std::error::Error for EmailError {}

