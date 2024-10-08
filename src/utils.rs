use std::env;

pub fn make_err(err: Box<dyn std::error::Error>, process: &str) -> String {
    format!("Failed {}: {:?}", process, err)
}

pub fn get_env(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("{} must be set", key))
}

pub fn get_env_feature_turned_on(key: &str) -> bool {
    if let Ok(var) = get_env(key) {
        return vec!["ok", "1", "yes", "y", "true"]
            .contains(&var.to_lowercase().as_str())
    }

    false
}