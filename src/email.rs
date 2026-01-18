use anyhow::Result;
use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone)]
pub struct EmailService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from_email: String,
    from_name: String,
    frontend_url: String,
    templates_dir: String,
}

impl EmailService {
    pub fn new(
        smtp_host: &str,
        smtp_port: u16,
        smtp_username: &str,
        smtp_password: &str,
        from_email: &str,
        from_name: &str,
        frontend_url: &str,
    ) -> Result<Self> {
        let creds = Credentials::new(smtp_username.to_string(), smtp_password.to_string());

        // Use STARTTLS for port 587, implicit TLS for port 465
        let mailer: AsyncSmtpTransport<Tokio1Executor> = if smtp_port == 465 {
            // Port 465: Implicit TLS (connection starts encrypted)
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
                .port(smtp_port)
                .credentials(creds)
                .build()
        } else {
            // Port 587 or others: STARTTLS (starts unencrypted, upgrades to TLS)
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)?
                .port(smtp_port)
                .credentials(creds)
                .build()
        };

        Ok(Self {
            mailer,
            from_email: from_email.to_string(),
            from_name: from_name.to_string(),
            frontend_url: frontend_url.to_string(),
            templates_dir: "templates/emails".to_string(),
        })
    }

    /// Load and parse an HTML template with variable substitution
    fn load_template(
        &self,
        template_name: &str,
        variables: &HashMap<&str, String>,
    ) -> Result<String> {
        let template_path = Path::new(&self.templates_dir).join(template_name);
        let mut html = fs::read_to_string(&template_path)
            .map_err(|e| anyhow::anyhow!("Failed to read template {}: {}", template_name, e))?;

        // Replace all {{variable}} placeholders
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key); // {{key}}
            html = html.replace(&placeholder, value);
        }

        Ok(html)
    }

    /// Generate plain text fallback from template purpose
    fn generate_plain_text(&self, purpose: &str, link: &str, username: Option<&str>) -> String {
        match purpose {
            "verification" => format!(
                r#"Welcome to BlogVerse!

Please verify your email address by clicking the link below:

{}

This link will expire in 24 hours.

If you didn't create an account, you can safely ignore this email.

Best regards,
The BlogVerse Team"#,
                link
            ),
            "password_reset" => format!(
                r#"Hi there,

You requested to reset your password. Click the link below to set a new password:

{}

This link will expire in 1 hour.

If you didn't request a password reset, you can safely ignore this email.

Best regards,
The BlogVerse Team"#,
                link
            ),
            "welcome" => format!(
                r#"Hi {},

Your email has been verified! Welcome to BlogVerse.

Start exploring: {}

Happy writing!

Best regards,
The BlogVerse Team"#,
                username.unwrap_or("there"),
                link
            ),
            _ => String::new(),
        }
    }

    /// Send email verification link
    pub async fn send_verification_email(&self, to_email: &str, token: &str) -> Result<()> {
        let verification_link = format!("{}/verify-email?token={}", self.frontend_url, token);

        let mut variables = HashMap::new();
        variables.insert("verification_link", verification_link.clone());

        let html_body = self.load_template("verification.html", &variables)?;
        let plain_body = self.generate_plain_text("verification", &verification_link, None);

        self.send_email(
            to_email,
            "Verify Your Email - BlogVerse",
            &plain_body,
            &html_body,
        )
        .await
    }

    /// Send password reset link
    pub async fn send_password_reset_email(&self, to_email: &str, token: &str) -> Result<()> {
        let reset_link = format!("{}/reset-password?token={}", self.frontend_url, token);

        let mut variables = HashMap::new();
        variables.insert("reset_link", reset_link.clone());

        let html_body = self.load_template("password_reset.html", &variables)?;
        let plain_body = self.generate_plain_text("password_reset", &reset_link, None);

        self.send_email(
            to_email,
            "Reset Your Password - BlogVerse",
            &plain_body,
            &html_body,
        )
        .await
    }

    /// Send welcome email after verification
    pub async fn send_welcome_email(&self, to_email: &str, username: &str) -> Result<()> {
        let dashboard_link = format!("{}/dashboard", self.frontend_url);

        let mut variables = HashMap::new();
        variables.insert("username", username.to_string());
        variables.insert("dashboard_link", dashboard_link.clone());

        let html_body = self.load_template("welcome.html", &variables)?;
        let plain_body = self.generate_plain_text("welcome", &dashboard_link, Some(username));

        self.send_email(to_email, "Welcome to BlogVerse!", &plain_body, &html_body)
            .await
    }

    /// Send multipart email (HTML + plain text fallback)
    async fn send_email(
        &self,
        to_email: &str,
        subject: &str,
        plain_body: &str,
        html_body: &str,
    ) -> Result<()> {
        let from = format!("{} <{}>", self.from_name, self.from_email);

        let email = Message::builder()
            .from(from.parse()?)
            .to(to_email.parse()?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(plain_body.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body.to_string()),
                    ),
            )?;

        self.mailer.send(email).await?;

        tracing::info!("Email sent to {}", to_email);
        Ok(())
    }
}
