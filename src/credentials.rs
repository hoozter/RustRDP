use keyring::Entry;

const SERVICE_NAME: &str = "com.hoozter.RustRDP";

#[derive(Clone, Copy, Debug, Default)]
pub struct CredentialStore;

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("The desktop wallet is not available: {0}")]
    Unavailable(String),
    #[error("The desktop wallet could not store this password: {0}")]
    Store(String),
    #[error("The password could not be retrieved from the desktop wallet: {0}")]
    Retrieve(String),
    #[error("The password could not be removed from the desktop wallet: {0}")]
    Delete(String),
}

impl CredentialStore {
    pub fn availability() -> Result<(), CredentialError> {
        Entry::store_status()
            .as_ref()
            .map(|_| ())
            .map_err(|error| CredentialError::Unavailable(error.to_string()))
    }

    pub fn store(credential_id: &str, password: &str) -> Result<(), CredentialError> {
        entry(credential_id)?
            .set_password(password)
            .map_err(|error| CredentialError::Store(error.to_string()))
    }

    pub fn retrieve(credential_id: &str) -> Result<String, CredentialError> {
        entry(credential_id)?
            .get_password()
            .map_err(|error| CredentialError::Retrieve(error.to_string()))
    }

    pub fn delete(credential_id: &str) -> Result<(), CredentialError> {
        entry(credential_id)?
            .delete_credential()
            .map_err(|error| CredentialError::Delete(error.to_string()))
    }
}

fn entry(credential_id: &str) -> Result<Entry, CredentialError> {
    Entry::new(SERVICE_NAME, credential_id)
        .map_err(|error| CredentialError::Unavailable(error.to_string()))
}
