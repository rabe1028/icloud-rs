use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
};

use icloud_client::{BlockingICloudClient, BlockingSignInResult, ReqwestBlockingTransport};
use icloud_types::{ClientContext, Credentials};
use secrecy::SecretString;

fn main() -> Result<(), Box<dyn Error>> {
    let apple_id = env::var("ICLOUD_APPLE_ID")?;
    let password = env::var("ICLOUD_PASSWORD")?;
    let widget_key = env::var("ICLOUD_WIDGET_KEY").unwrap_or_else(|_| {
        "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d".to_owned()
    });

    let context = ClientContext::production(widget_key)?;
    let transport = ReqwestBlockingTransport::new(reqwest::blocking::Client::new());
    let client = BlockingICloudClient::random_identity(context, transport);
    let credentials = Credentials::new(apple_id, SecretString::from(password));

    let prebootstrap = match client.sign_in(&credentials)? {
        BlockingSignInResult::Authenticated(client) => client,
        BlockingSignInResult::PendingTwoFactor(mut pending) => {
            println!("auth status: {:?}", pending.auth_status());
            if let Some(phone_number) = pending.trusted_phone_numbers().first().cloned() {
                pending.request_sms_verification_code(&phone_number)?;
                println!(
                    "sent SMS code to {}",
                    phone_number
                        .display_label()
                        .unwrap_or("trusted phone number")
                );
                let code = prompt("verification code")?;
                pending.submit_sms_verification_code(&phone_number, code.trim())?
            } else {
                println!("enter the code shown on a trusted device");
                let code = prompt("verification code")?;
                pending.submit_trusted_device_code(code.trim())?
            }
        }
    };

    let authenticated = prebootstrap.bootstrap()?;
    let snapshot = authenticated.snapshot();

    // The snapshot contains cookies and auth tokens. Encrypt this file in real apps.
    fs::write(
        "icloud-session.snapshot.json",
        serde_json::to_vec_pretty(&snapshot)?,
    )?;
    println!("wrote encrypted-at-rest caller-owned snapshot material");

    Ok(())
}

fn prompt(label: &str) -> Result<String, io::Error> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value)
}
