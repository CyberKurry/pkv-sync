use crate::api::error::ApiError;
use crate::auth::password;

pub(crate) async fn hash_admin_password(plaintext: &str) -> Result<String, ApiError> {
    let plaintext = plaintext.to_owned();
    tokio::task::spawn_blocking(move || password::hash_strong(&plaintext))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .map_err(password_error_to_api)
}

fn password_error_to_api(e: password::PasswordError) -> ApiError {
    match e {
        password::PasswordError::TooShort { .. }
        | password::PasswordError::TooLong { .. }
        | password::PasswordError::TooWeak => ApiError::bad_request("weak_password", e.to_string()),
        _ => ApiError::internal(e.to_string()),
    }
}
