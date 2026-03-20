use anyhow::Result;
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

pub fn store_token(service: &str, account: &str, token: &str) -> Result<()> {
    // Delete first to avoid duplicate item errors; ignore not-found
    let _ = delete_generic_password(service, account);
    set_generic_password(service, account, token.as_bytes())?;
    Ok(())
}

pub fn get_token(service: &str, account: &str) -> Result<Option<String>> {
    match get_generic_password(service, account) {
        Ok(bytes) => Ok(Some(String::from_utf8(bytes)?)),
        Err(e) if e.code() == -25300 => Ok(None), // errSecItemNotFound
        Err(e) => Err(e.into()),
    }
}

pub fn delete_token(service: &str, account: &str) -> Result<()> {
    match delete_generic_password(service, account) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()), // already gone
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve_token() {
        let service = "com.homerun.test.keychain";
        let account = "github-token";
        let token = "ghp_test_token_12345";

        store_token(service, account, token).unwrap();
        let retrieved = get_token(service, account).unwrap();
        assert_eq!(retrieved, Some(token.to_string()));

        delete_token(service, account).unwrap();
        let deleted = get_token(service, account).unwrap();
        assert_eq!(deleted, None);
    }
}
