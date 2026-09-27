//! Shared application identity used by the daemon and desktop client.

#[cfg(debug_assertions)]
pub const APP_NAME: &str = "Anastasia Debug";
#[cfg(not(debug_assertions))]
pub const APP_NAME: &str = "Anastasia";

#[cfg(debug_assertions)]
pub const APP_ID: &str = "app.anastasia.debug";
#[cfg(not(debug_assertions))]
pub const APP_ID: &str = "app.anastasia";

#[cfg(debug_assertions)]
pub const DATA_DIRECTORY_NAME: &str = "Anastasia Debug";
#[cfg(not(debug_assertions))]
pub const DATA_DIRECTORY_NAME: &str = "Anastasia";

/// Desktop preferences live beside engine data, never in the prototype GUI home.
pub fn desktop_data_dir() -> std::path::PathBuf {
    let home = std::env::var_os("ANASTASIA_CLI_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join(".anastasia-cli")
        });
    home.join("desktop")
}
