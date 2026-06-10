use crate::auth::AuthenticatedUser;
use crate::db::repos::{TokenRepo, UserRepo};
use crate::service::AppState;

pub(crate) async fn mcp_token_still_valid(
    state: &AppState,
    token_hash: &str,
    user: &AuthenticatedUser,
) -> bool {
    let Ok(Some((row, user_id))) = state.tokens.find_by_hash(token_hash).await else {
        return false;
    };
    if row.id != user.token_id || user_id != user.user_id {
        return false;
    }
    let Ok(Some(db_user)) = state.users.find_by_id(&user.user_id).await else {
        return false;
    };
    db_user.is_active
}
